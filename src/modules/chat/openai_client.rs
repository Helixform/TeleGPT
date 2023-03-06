use anyhow::Error;
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionResponseMessage, CreateChatCompletionRequestArgs,
};
use async_openai::Client as OpenAIClient;

pub(crate) struct ChatModelResult {
    pub message: ChatCompletionResponseMessage,
    pub token_usage: u32,
}

pub(crate) fn new_client(api_key: &str) -> OpenAIClient {
    OpenAIClient::new().with_api_key(api_key)
}

pub(crate) async fn request_chat_model(
    client: &OpenAIClient,
    msgs: Vec<ChatCompletionRequestMessage>,
) -> Result<ChatModelResult, Error> {
    let req = CreateChatCompletionRequestArgs::default()
        .model("gpt-3.5-turbo")
        .temperature(0.6)
        .messages(msgs)
        .build()?;

    let resp = client.chat().create(req).await?;
    let mut choices = resp.choices;

    if choices.is_empty() {
        return Err(anyhow!("Server responds with empty data"));
    }

    Ok(ChatModelResult {
        message: choices.remove(0).message,
        token_usage: resp.usage.map(|u| u.total_tokens).unwrap_or(0),
    })
}
