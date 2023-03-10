use std::pin::Pin;

use anyhow::Error;
use async_openai::types::{ChatCompletionRequestMessage, CreateChatCompletionRequestArgs};
use async_openai::Client as OpenAIClient;
use futures::{future, Stream, StreamExt};

pub(crate) type ChatModelStream = Pin<Box<dyn Stream<Item = ChatModelResult> + Send>>;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ChatModelResult {
    pub content: String,
    pub token_usage: u32,
}

pub(crate) fn new_client(api_key: &str) -> OpenAIClient {
    OpenAIClient::new().with_api_key(api_key)
}

pub(crate) async fn request_chat_model(
    client: &OpenAIClient,
    msgs: Vec<ChatCompletionRequestMessage>,
) -> Result<ChatModelStream, Error> {
    let req = CreateChatCompletionRequestArgs::default()
        .model("gpt-3.5-turbo")
        .temperature(0.6)
        .messages(msgs)
        .build()?;

    let stream = client.chat().create_stream(req).await?;
    Ok(stream
        .scan(ChatModelResult::default(), |acc, cur| {
            let content = cur
                .as_ref()
                .ok()
                .and_then(|resp| resp.choices.first())
                .and_then(|choice| choice.delta.content.as_ref());
            if let Some(content) = content {
                acc.content.push_str(content);
            }
            future::ready(Some(acc.clone()))
        })
        .boxed())
}

pub(crate) fn estimate_prompt_tokens(msgs: &Vec<ChatCompletionRequestMessage>) -> u32 {
    let mut text_len = 0;
    for msg in msgs {
        text_len += msg.content.len();
    }
    ((text_len as f64) * 1.4) as _
}

pub(crate) fn estimate_tokens(text: &str) -> u32 {
    let text_len = text.len();
    ((text_len as f64) * 1.4) as _
}
