mod braille;
mod openai_client;
mod session;
mod session_mgr;

use std::time::Duration;

use async_openai::types::{ChatCompletionRequestMessageArgs, Role};
use async_openai::Client as OpenAIClient;
use teloxide::dispatching::DpHandlerDescription;
use teloxide::prelude::*;
use teloxide::types::{BotCommand, InlineKeyboardButton, InlineKeyboardMarkup, Me};

use crate::module_mgr::Module;
use crate::utils::dptree_ext;
use crate::StatsManager;
use crate::{noop_handler, HandlerResult};
pub(crate) use session::Session;
pub(crate) use session_mgr::SessionManager;

#[derive(Debug, Clone, PartialEq, Eq)]
struct MessageText(String);

async fn handle_chat_message(
    bot: Bot,
    me: Me,
    msg: Message,
    text: MessageText,
    chat_id: ChatId,
    session_mgr: SessionManager,
    stats_mgr: StatsManager,
    openai_client: OpenAIClient,
) -> bool {
    let mut text = text.0;
    let chat_id = chat_id.to_string();

    if text.starts_with("/") {
        // Let other modules to process the command.
        return false;
    }

    let trimmed_text = text.trim_start();
    if trimmed_text.starts_with("@") {
        // Remove the leading mention to prevent the model from
        // being affected by it.
        let username = me.username();
        if trimmed_text[1..].starts_with(username) {
            text = trimmed_text[(1 + username.len())..].to_owned();
        }
    }
    text = text.trim().to_owned();

    match actually_handle_chat_message(
        bot,
        Some(msg),
        text,
        chat_id,
        session_mgr,
        stats_mgr,
        openai_client,
    )
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
    stats_mgr: StatsManager,
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
        stats_mgr,
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
    stats_mgr: StatsManager,
    openai_client: OpenAIClient,
) -> HandlerResult {
    // Send a progress indicator message first.
    let progress_bar = braille::BrailleProgress::new(1, 1, 3, Some("Thinking... 🤔".to_owned()));
    let mut send_progress_msg = bot.send_message(chat_id.clone(), progress_bar.current_string());
    send_progress_msg.reply_to_message_id = reply_to_msg.as_ref().map(|m| m.id);
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
        reply_result = openai_client::request_chat_model(&openai_client, msgs) => {
            reply_result.map_err(|err| anyhow!("API error: {}", err))
        },
        _ = tokio::time::sleep(Duration::from_secs(30)) => {
            Err(anyhow!("API timeout"))
        },
    };

    // Reply to the user and add to history.
    let reply_result = match req_result {
        Ok(res) => {
            let reply_text = res.message.content;
            session_mgr.add_message_to_session(chat_id.clone(), user_msg);
            session_mgr.add_message_to_session(
                chat_id.clone(),
                ChatCompletionRequestMessageArgs::default()
                    .role(Role::Assistant)
                    .content(reply_text.clone())
                    .build()
                    .unwrap(),
            );
            // TODO: maybe we need to handle the case that `reply_to_msg` is `None`.
            if let Some(from_username) = reply_to_msg
                .as_ref()
                .and_then(|m| m.from())
                .and_then(|u| u.username.as_ref())
            {
                stats_mgr
                    .add_usage(from_username.to_owned(), res.token_usage as _)
                    .await;
            }
            // TODO: add retry for edit failures.
            bot.edit_message_text(chat_id, sent_progress_msg.id, reply_text)
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

pub(crate) struct Chat;

impl Module for Chat {
    fn register_dependency(&mut self, dep_map: &mut DependencyMap) {
        dep_map.insert(SessionManager::new());
        dep_map.insert(openai_client::new_client());
    }

    fn handler_chain(
        &self,
    ) -> Handler<'static, DependencyMap, HandlerResult, DpHandlerDescription> {
        dptree::entry()
            .branch(
                Update::filter_message()
                    .filter_map(|msg: Message| msg.text().map(|text| MessageText(text.to_owned())))
                    .map(|msg: Message| msg.chat.id)
                    .branch(
                        dptree::filter(dptree_ext::command_filter("reset")).endpoint(reset_session),
                    )
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
