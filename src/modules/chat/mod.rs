#![allow(clippy::too_many_arguments)]

mod braille;
mod openai_client;
mod session;
mod session_mgr;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Error;
use async_openai::types::{ChatCompletionRequestMessage, ChatCompletionRequestMessageArgs, Role};
use async_openai::Client as OpenAIClient;
use futures::StreamExt as FuturesStreamExt;
use teloxide::dispatching::DpHandlerDescription;
use teloxide::dptree::di::DependencySupplier;
use teloxide::prelude::*;
use teloxide::types::{BotCommand, InlineKeyboardButton, InlineKeyboardMarkup, Me};

use crate::{
    config::SharedConfig,
    dispatcher::noop_handler,
    module_mgr::Module,
    modules::stats::StatsManager,
    types::HandlerResult,
    utils::{dptree_ext, StreamExt},
};
use braille::BrailleProgress;
use openai_client::ChatModelResult;
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
    config: SharedConfig,
) -> bool {
    let mut text = text.0;
    let chat_id = chat_id.to_string();

    if text.starts_with('/') {
        // Let other modules to process the command.
        return false;
    }

    let trimmed_text = text.trim_start();
    if let Some(text_without_at) = trimmed_text.strip_prefix('@') {
        // Remove the leading mention to prevent the model from
        // being affected by it.
        let username = me.username();
        if let Some(text_without_mention) = text_without_at.strip_prefix(username) {
            text = text_without_mention.to_owned();
        }
    }
    text = text.trim().to_owned();

    if let Err(err) = actually_handle_chat_message(
        bot,
        Some(msg),
        text,
        chat_id,
        session_mgr,
        stats_mgr,
        openai_client,
        config,
    )
    .await
    {
        error!("Failed to handle chat message: {}", err);
    }

    true
}

async fn handle_retry_action(
    bot: Bot,
    query: CallbackQuery,
    session_mgr: SessionManager,
    stats_mgr: StatsManager,
    openai_client: OpenAIClient,
    config: SharedConfig,
) -> bool {
    if !query.data.map(|data| data == "/retry").unwrap_or(false) {
        return false;
    }

    let message = query.message;
    if message.is_none() {
        return false;
    }
    let message = message.unwrap();

    if let Err(err) = bot.delete_message(message.chat.id, message.id).await {
        error!("Failed to revoke the retry message: {}", err);
        return false;
    }

    let chat_id = message.chat.id.to_string();
    let last_message = session_mgr.swap_session_pending_message(chat_id.clone(), None);
    if last_message.is_none() {
        error!("Last message not found");
        return true;
    }
    let last_message = last_message.unwrap();

    if let Err(err) = actually_handle_chat_message(
        bot,
        None,
        last_message.content,
        chat_id,
        session_mgr,
        stats_mgr,
        openai_client,
        config,
    )
    .await
    {
        error!("Failed to retry handling chat message: {}", err);
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
    config: SharedConfig,
) -> HandlerResult {
    // Send a progress indicator message first.
    let progress_bar = BrailleProgress::new(1, 1, 3, Some("Thinking... 🤔".to_owned()));
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

    let result = stream_model_result(
        &bot,
        &chat_id,
        &sent_progress_msg,
        progress_bar,
        msgs,
        openai_client,
        &config,
    )
    .await;

    // Record stats and add the reply to history.
    let reply_result = match result {
        Ok(res) => {
            session_mgr.add_message_to_session(chat_id.clone(), user_msg);
            session_mgr.add_message_to_session(
                chat_id.clone(),
                ChatCompletionRequestMessageArgs::default()
                    .role(Role::Assistant)
                    .content(res.content)
                    .build()
                    .unwrap(),
            );
            // TODO: maybe we need to handle the case that `reply_to_msg` is `None`.
            if let Some(from_username) = reply_to_msg
                .as_ref()
                .and_then(|m| m.from())
                .and_then(|u| u.username.as_ref())
            {
                let res = stats_mgr
                    .add_usage(from_username.to_owned(), res.token_usage as _)
                    .await;
                if let Err(err) = res {
                    error!("Failed to update stats: {}", err);
                }
            }
            Ok(())
        }
        Err(err) => {
            error!("Failed to request the model: {}", err);
            session_mgr.swap_session_pending_message(chat_id.clone(), Some(user_msg));
            let retry_button = InlineKeyboardButton::callback("Retry", "/retry");
            let reply_markup = InlineKeyboardMarkup::default().append_row([retry_button]);
            bot.edit_message_text(chat_id, sent_progress_msg.id, &config.i18n.api_error_prompt)
                .reply_markup(reply_markup)
                .await
                .map(|_| ())
        }
    };

    if let Err(err) = reply_result {
        error!("Failed to edit the final message: {}", err);
    }

    Ok(())
}

async fn stream_model_result(
    bot: &Bot,
    chat_id: &str,
    editing_msg: &Message,
    mut progress_bar: BrailleProgress,
    msgs: Vec<ChatCompletionRequestMessage>,
    openai_client: OpenAIClient,
    config: &SharedConfig,
) -> Result<ChatModelResult, Error> {
    let estimated_prompt_tokens = openai_client::estimate_prompt_tokens(&msgs);

    let stream = openai_client::request_chat_model(&openai_client, msgs).await?;
    let mut throttled_stream =
        stream.throttle_buffer::<Vec<_>>(Duration::from_millis(config.stream_throttle_interval));

    let mut timeout_times = 0;
    let mut last_response = None;
    loop {
        tokio::select! {
            res = throttled_stream.next() => {
                if res.is_none() {
                    break;
                }

                // Since the stream item is scanned (accumulated), we only
                // need to get the last item in the buffer and use it as
                // the latest message content.
                last_response = res.as_ref().unwrap().last().cloned();

                // Reset the timeout once the stream is resumed.
                timeout_times = 0;
            },
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                timeout_times += 1;
                if timeout_times >= config.openai_api_timeout {
                    return Err(anyhow!("Stream is timeout"));
                }
            }
        }

        progress_bar.advance_progress();
        let updated_text = if let Some(last_response) = &last_response {
            format!(
                "{}\n{}",
                last_response.content,
                progress_bar.current_string()
            )
        } else {
            progress_bar.current_string()
        };

        let _ = bot
            .edit_message_text(chat_id.to_owned(), editing_msg.id, updated_text)
            .await;
    }

    if let Some(mut last_response) = last_response {
        // TODO: OpenAI currently doesn't support to give the token usage
        // in stream mode. Therefore we need to estimate it locally.
        last_response.token_usage =
            openai_client::estimate_tokens(&last_response.content) + estimated_prompt_tokens;
        bot.edit_message_text(chat_id.to_owned(), editing_msg.id, &last_response.content)
            .await?;
        return Ok(last_response);
    }

    Err(anyhow!("Server returned empty response"))
}

async fn reset_session(
    bot: Bot,
    chat_id: ChatId,
    session_mgr: SessionManager,
    config: SharedConfig,
) -> HandlerResult {
    session_mgr.reset_session(chat_id.to_string());
    let _ = bot.send_message(chat_id, &config.i18n.reset_prompt).await;
    Ok(())
}

pub(crate) struct Chat;

impl Module for Chat {
    fn register_dependency(&mut self, dep_map: &mut DependencyMap) {
        let config: Arc<SharedConfig> = dep_map.get();

        dep_map.insert(SessionManager::new(config.as_ref().clone()));
        dep_map.insert(openai_client::new_client(&config.openai_api_key));
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
        vec![BotCommand::new("reset", "Reset the current session")]
    }
}
