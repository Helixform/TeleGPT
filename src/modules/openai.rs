use std::pin::Pin;
use std::sync::Arc;

use anyhow::Error;
use async_openai::types::{ChatCompletionRequestMessage, CreateChatCompletionRequestArgs};
use async_openai::Client;
use futures::{future, Stream, StreamExt};
use teloxide::dptree::di::{DependencyMap, DependencySupplier};

use crate::{config::SharedConfig, module_mgr::Module};

pub(crate) type ChatModelStream = Pin<Box<dyn Stream<Item = ChatModelResult> + Send>>;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ChatModelResult {
    pub content: String,
    pub token_usage: u32,
}

#[derive(Clone)]
pub(crate) struct OpenAIClient {
    client: Client,
    config: SharedConfig,
}

impl OpenAIClient {
    pub(crate) async fn request_chat_model(
        &self,
        msgs: Vec<ChatCompletionRequestMessage>,
    ) -> Result<ChatModelStream, Error> {
        let client = &self.client;
        let req = CreateChatCompletionRequestArgs::default()
            .model("gpt-3.5-turbo")
            .temperature(0.6)
            .max_tokens(self.config.max_tokens.unwrap_or(4096))
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

    pub(crate) fn estimate_prompt_tokens(&self, msgs: &Vec<ChatCompletionRequestMessage>) -> u32 {
        let mut text_len = 0;
        for msg in msgs {
            text_len += msg.content.len();
        }
        ((text_len as f64) * 1.4) as _
    }

    pub(crate) fn estimate_tokens(&self, text: &str) -> u32 {
        let text_len = text.len();
        ((text_len as f64) * 1.4) as _
    }
}

pub(crate) struct OpenAI;

#[async_trait]
impl Module for OpenAI {
    async fn register_dependency(&mut self, dep_map: &mut DependencyMap) -> Result<(), Error> {
        let config: Arc<SharedConfig> = dep_map.get();

        let openai_client = OpenAIClient {
            client: Client::new().with_api_key(&config.openai_api_key),
            config: config.as_ref().clone(),
        };
        dep_map.insert(openai_client);

        Ok(())
    }
}
