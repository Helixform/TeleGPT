mod session;
mod session_mgr;

use std::error::Error;

use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestMessageArgs,
    CreateChatCompletionRequestArgs, Role,
};
use async_openai::Client as OpenAIClient;
use teloxide::dispatching::DpHandlerDescription;
use teloxide::prelude::*;

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

    // Send a progress indicator message first.
    let mut send_progress_msg = bot.send_message(chat_id.clone(), ".");
    send_progress_msg.reply_to_message_id = Some(msg.id);
    let sent_progress_msg = send_progress_msg.await;
    if sent_progress_msg.is_err() {
        error!(
            "Failed to send progress message: {}",
            sent_progress_msg.unwrap_err()
        );
        return true;
    }
    let sent_progress_msg = sent_progress_msg.unwrap();

    // Construct the request messages.
    let mut msgs = session_mgr.get_history_messages(&chat_id);
    let user_msg = ChatCompletionRequestMessageArgs::default()
        .role(Role::User)
        .content(text)
        .build()
        .unwrap();
    msgs.push(user_msg.clone());

    // Send request to OpenAI while playing a progress animation.
    let reply_result = tokio::select! {
        _ = async {
            let mut current_text = ".".to_owned();
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                current_text.push_str(" .");
                let _ = bot.edit_message_text(
                    chat_id.clone(),
                    sent_progress_msg.id,
                    &current_text
                ).await;
            }
        } => { unreachable!() },
        reply_result = request_chat_model(&openai_client, msgs) => {
            // WORKAROUND:
            // I had to use `Option` here to avoid a strange ICE...
            if reply_result.is_err() {
                error!("Failed to request OpenAI: {}", reply_result.unwrap_err());
                None
            } else {
                Some(reply_result.unwrap())
            }
        }
    };

    // Reply to the user and add to history.
    let reply = match reply_result {
        Some(text) => {
            session_mgr.add_message_to_session(chat_id.clone(), user_msg);
            session_mgr.add_message_to_session(
                chat_id.clone(),
                ChatCompletionRequestMessageArgs::default()
                    .role(Role::System)
                    .content(text.clone())
                    .build()
                    .unwrap(),
            );
            text
        }
        None => "Hmm, something went wrong...".to_owned(),
    };

    match bot
        .edit_message_text(chat_id, sent_progress_msg.id, reply)
        .await
    {
        Err(err) => {
            error!("Failed to edit the final message: {}", err);
        }
        _ => {}
    }

    true
}

async fn reset_session(bot: Bot, chat_id: ChatId, session_mgr: SessionManager) -> HandlerResult {
    session_mgr.reset_session(chat_id.to_string());
    let _ = bot.send_message(chat_id, "⚠ 会话已重置").await;
    Ok(())
}

async fn request_chat_model(
    client: &OpenAIClient,
    msgs: Vec<ChatCompletionRequestMessage>,
) -> Result<String, Box<dyn Error>> {
    let req = CreateChatCompletionRequestArgs::default()
        .model("gpt-3.5-turbo")
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

fn filter_command(cmd: &str) -> impl Fn(MessageText) -> bool {
    let pat = format!("/{}", cmd);
    move |text| text.0 == pat
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
        dptree::filter_map(|msg: Message| msg.text().map(|text| MessageText(text.to_owned())))
            .map(|msg: Message| msg.chat.id)
            .branch(dptree::filter(filter_command("reset")).endpoint(reset_session))
            .branch(dptree::filter_async(handle_chat_message).endpoint(noop_handler))
    }
}
