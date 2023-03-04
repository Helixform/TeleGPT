mod braille;
mod session;
mod session_mgr;

use std::error::Error;
use std::time::Duration;

use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestMessageArgs,
    CreateChatCompletionRequestArgs, Role,
};
use async_openai::Client as OpenAIClient;
use teloxide::dispatching::DpHandlerDescription;
use teloxide::prelude::*;
use teloxide::types::{BotCommand, InlineKeyboardButton, InlineKeyboardMarkup, Me};

use crate::module_mgr::Module;
use crate::{noop_handler, HandlerResult};
pub(crate) use session::Session;
pub(crate) use session_mgr::SessionManager;

#[derive(Debug, Clone, PartialEq, Eq)]
struct MessageText(String);

async fn handle_chat_message(
    bot: Bot,
    msg: Message,
    text: MessageText,
    chat_id: ChatId,
    session_mgr: SessionManager,
    openai_client: OpenAIClient,
) -> bool {
    let text = text.0;
    let chat_id = chat_id.to_string();

    if text.starts_with("/") {
        // Let other modules to process the command.
        return false;
    }

    match actually_handle_chat_message(bot, Some(msg), text, chat_id, session_mgr, openai_client)
        .await
    {
        Err(err) => {
            error!("Failed to handle chat message: {}", err);
        }
        _ => {}
    }

    true
}

async fn handle_retry_action(
    bot: Bot,
    query: CallbackQuery,
    session_mgr: SessionManager,
    openai_client: OpenAIClient,
) -> bool {
    if !query.data.map(|data| data == "/retry").unwrap_or(false) {
        return false;
    }

    let message = query.message;
    if message.is_none() {
        return false;
    }
    let message = message.unwrap();

    match bot.delete_message(message.chat.id, message.id).await {
        Err(err) => {
            error!("Failed to revoke the retry message: {}", err);
            return false;
        }
        _ => {}
    }

    let chat_id = message.chat.id.to_string();
    let last_message = session_mgr.swap_session_pending_message(chat_id.clone(), None);
    if last_message.is_none() {
        error!("Last message not found");
        return true;
    }
    let last_message = last_message.unwrap();

    match actually_handle_chat_message(
        bot,
        None,
        last_message.content,
        chat_id,
        session_mgr,
        openai_client,
    )
    .await
    {
        Err(err) => {
            error!("Failed to retry handling chat message: {}", err);
        }
        _ => {}
    }

    true
}

async fn actually_handle_chat_message(
    bot: Bot,
    reply_to_msg: Option<Message>,
    content: String,
    chat_id: String,
    session_mgr: SessionManager,
    openai_client: OpenAIClient,
) -> HandlerResult {
    // Send a progress indicator message first.
    let progress_bar = braille::BrailleProgress::new(1, 1, 3, Some("Thinking... 🤔".to_owned()));
    let mut send_progress_msg = bot.send_message(chat_id.clone(), progress_bar.current_string());
    send_progress_msg.reply_to_message_id = reply_to_msg.map(|m| m.id);
    let sent_progress_msg = send_progress_msg.await?;

    // Construct the request messages.
    let mut msgs = session_mgr.get_history_messages(&chat_id);
    let user_msg = ChatCompletionRequestMessageArgs::default()
        .role(Role::User)
        .content(content)
        .build()
        .unwrap();
    msgs.push(user_msg.clone());

    // Send request to OpenAI while playing a progress animation.
    let req_result = tokio::select! {
        _ = async {
            let mut progress_bar = progress_bar;
            loop {
                tokio::time::sleep(Duration::from_millis(200)).await;
                progress_bar.advance_progress();
                let _ = bot.edit_message_text(
                    chat_id.clone(),
                    sent_progress_msg.id,
                    &progress_bar.current_string()
                ).await;
            }
        } => { unreachable!() },
        reply_result = request_chat_model(&openai_client, msgs) => {
            reply_result.map_err(|err| anyhow!("API error: {}", err))
        },
        _ = tokio::time::sleep(Duration::from_secs(30)) => {
            Err(anyhow!("API timeout"))
        },
    };

    // Reply to the user and add to history.
    let reply_result = match req_result {
        Ok(text) => {
            session_mgr.add_message_to_session(chat_id.clone(), user_msg);
            session_mgr.add_message_to_session(
                chat_id.clone(),
                ChatCompletionRequestMessageArgs::default()
                    .role(Role::Assistant)
                    .content(text.clone())
                    .build()
                    .unwrap(),
            );
            // TODO: add retry for edit failures.
            bot.edit_message_text(chat_id, sent_progress_msg.id, text)
                .await
        }
        Err(err) => {
            error!("Failed to request the model: {}", err);
            session_mgr.swap_session_pending_message(chat_id.clone(), Some(user_msg));
            let retry_button = InlineKeyboardButton::callback("Retry", "/retry");
            let reply_markup = InlineKeyboardMarkup::default().append_row([retry_button]);
            bot.edit_message_text(
                chat_id,
                sent_progress_msg.id,
                "Hmm, something went wrong...",
            )
            .reply_markup(reply_markup)
            .await
        }
    };

    if let Err(err) = reply_result {
        error!("Failed to edit the final message: {}", err);
    }

    Ok(())
}

async fn reset_session(bot: Bot, chat_id: ChatId, session_mgr: SessionManager) -> HandlerResult {
    session_mgr.reset_session(chat_id.to_string());
    let _ = bot.send_message(chat_id, "⚠️ Session is reset!").await;
    Ok(())
}

async fn request_chat_model(
    client: &OpenAIClient,
    msgs: Vec<ChatCompletionRequestMessage>,
) -> Result<String, Box<dyn Error>> {
    let req = CreateChatCompletionRequestArgs::default()
        .model("gpt-3.5-turbo")
        .temperature(0.6)
        .messages(msgs)
        .build()?;

    let resp = client.chat().create(req).await?;
    let mut choices = resp.choices;

    if choices.is_empty() {
        // TODO: use `Err()` to indicate a server error.
        return Ok("".to_owned());
    }

    Ok(choices.remove(0).message.content)
}

fn filter_command(cmd: &str) -> impl Fn(Me, MessageText) -> bool {
    let pat = format!("/{}", cmd);
    move |me, text| {
        if !text.0.starts_with(&pat) {
            return false;
        }

        // When sending commands in a group, a mention suffix may be attached to
        // the text. For example: "/reset@xxxx_bot".
        let rest = &text.0[pat.len()..];
        if rest.len() > 1 {
            return me
                .username
                .as_ref()
                .map(|n| n == &rest[1..])
                .unwrap_or(false);
        }

        true
    }
}

pub(crate) struct Chat;

impl Module for Chat {
    fn register_dependency(&self, dep_map: &mut DependencyMap) {
        dep_map.insert(SessionManager::new());
        dep_map.insert(OpenAIClient::new());
    }

    fn handler_chain(
        &self,
    ) -> Handler<'static, DependencyMap, HandlerResult, DpHandlerDescription> {
        dptree::entry()
            .branch(
                Update::filter_message()
                    .filter_map(|msg: Message| msg.text().map(|text| MessageText(text.to_owned())))
                    .map(|msg: Message| msg.chat.id)
                    .branch(dptree::filter(filter_command("reset")).endpoint(reset_session))
                    .branch(dptree::filter_async(handle_chat_message).endpoint(noop_handler)),
            )
            .branch(
                Update::filter_callback_query()
                    .filter_async(handle_retry_action)
                    .endpoint(noop_handler),
            )
    }

    fn commands(&self) -> Vec<BotCommand> {
        return vec![BotCommand::new("reset", "Reset the current session")];
    }
}
