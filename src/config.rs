use std::{ops::Deref, sync::Arc};

use paste::paste;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct SharedConfig {
    config: Arc<Config>,
}

impl SharedConfig {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}

impl Deref for SharedConfig {
    type Target = Config;

    fn deref(&self) -> &Self::Target {
        return self.config.as_ref();
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(rename = "openaiAPIKey")]
    pub openai_api_key: String,
    #[serde(rename = "botToken")]
    pub telegram_bot_token: String,

    #[serde(default = "default_openai_api_timeout", rename = "openaiAPITimeout")]
    pub openai_api_timeout: u64,

    #[serde(default = "default_conversation_limit", rename = "conversationLimit")]
    pub conversation_limit: u64,

    #[serde(default)]
    pub i18n: I18nStrings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct I18nStrings {
    #[serde(default = "default_api_error_prompt", rename = "apiErrorPrompt")]
    pub api_error_prompt: String,
    #[serde(default = "default_reset_prompt", rename = "resetPrompt")]
    pub reset_prompt: String,
}

macro_rules! define_defaults {
    ($ty_name:ident { $($name:ident: $ty:ty = $default:expr,)* }) => {
        define_defaults! { $($name: $ty = $default,)* }
        paste! {
            impl Default for $ty_name {
                fn default() -> Self {
                    Self {
                        $($name: [<default_ $name>](),)*
                    }
                }
            }
        }
    };
    ($($name:ident: $ty:ty = $default:expr,)*) => {
        paste! {
            $(
                fn [<default_ $name>]() -> $ty {
                    $default
                }
            )*
        }
    };
}

define_defaults! {
    openai_api_timeout: u64 = 30,
    conversation_limit: u64 = 20,
}

define_defaults!(I18nStrings {
    api_error_prompt: String = "Hmm, something went wrong...".to_owned(),
    reset_prompt: String = "\u{26A0} Session is reset!".to_owned(),
});
