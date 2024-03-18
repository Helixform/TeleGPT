//! Configuration-related types.
//!
//! The configuration can be represented in and deserialized from JSON,
//! here is an example:
//!
//! ```json
//! {
//!   "openaiAPIKey": "sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
//!   "botToken": "8888888888:XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
//!   "adminUsernames": ["cyandev"],
//!   "conversationLimit": 30,
//!   "databasePath": "./path/to/telegpt.sqlite",
//!   "i18n": {
//!     "resetPrompt": "I’m ready for a new challenge. What can I do for you now?"
//!   }
//! }
//! ```
//!
//! See [`Config`] for more detailed descriptions.

use std::collections::HashSet;
use std::ops::Deref;
use std::sync::Arc;

use paste::paste;
use serde::Deserialize;

/// A thread-safe reference-counting object that represents
/// a [`Config`] instance.
#[derive(Debug, Clone)]
pub struct SharedConfig {
    config: Arc<Config>,
}

impl SharedConfig {
    /// Constructs a new `SharedConfig`.
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

/// Top-level config type fot the bot.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// The API key of your OpenAI account.
    /// JSON key: `openaiAPIKey`
    #[serde(rename = "openaiAPIKey")]
    pub openai_api_key: String,
    /// The token of your Telegram bot.
    /// JSON key: `botToken`
    #[serde(rename = "botToken")]
    pub telegram_bot_token: String,

    /// The openai model your want to use in chat.
    /// Value is default to "gpt-3.5-turbo".
    /// JSON key: `openaiGptModel`
    #[serde(default = "default_openai_gpt_model", rename = "openaiGptModel")]
    pub openai_gpt_model: String,

    /// A timeout in seconds for waiting for the OpenAI server response.
    /// JSON key: `openaiAPITimeout`
    #[serde(default = "default_openai_api_timeout", rename = "openaiAPITimeout")]
    pub openai_api_timeout: u64,

    /// A set of usernames that represents the admin users, who can use
    /// admin commands. You must specify this field to use admin features.
    /// JSON key: `adminUsernames`
    #[serde(default, rename = "adminUsernames")]
    pub admin_usernames: HashSet<String>,

    /// The throttle interval (in milliseconds) for sending streamed
    /// chunks back to Telegram.
    /// JSON key: `streamThrottleInterval`
    #[serde(
        default = "default_stream_throttle_interval",
        rename = "streamThrottleInterval"
    )]
    pub stream_throttle_interval: u64,

    /// Maximum number of messages in a single conversation.
    /// JSON key: `conversationLimit`
    #[serde(default = "default_conversation_limit", rename = "conversationLimit")]
    pub conversation_limit: u64,

    /// The maximum number of tokens allowed for the generated answer.
    /// JSON key: `maxTokens`
    #[serde(default, rename = "maxTokens")]
    pub max_tokens: Option<u16>,

    /// A boolean value that indicates whether to parse and render the
    /// markdown contents. When set to `false`, the raw contents returned
    /// from OpenAI will be displayed. This is default to `false`.
    /// JSON key: `rendersMarkdown`
    #[serde(default = "default_renders_markdown", rename = "rendersMarkdown")]
    pub renders_markdown: bool,

    /// A path for storing the database, [`None`] for in-memory database.
    /// JSON key: `databasePath`
    #[serde(rename = "databasePath")]
    pub database_path: Option<String>,

    /// Strings for I18N.
    /// JSON key: `i18n`
    #[serde(default)]
    pub i18n: I18nStrings,
}

/// Strings for I18N.
#[derive(Debug, Clone, Deserialize)]
pub struct I18nStrings {
    /// A text to display when there are something wrong with the OpenAI service.
    /// JSON key: `apiErrorPrompt`
    #[serde(default = "default_api_error_prompt", rename = "apiErrorPrompt")]
    pub api_error_prompt: String,
    /// A text to display when the session is reset.
    /// JSON key: `resetPrompt`
    #[serde(default = "default_reset_prompt", rename = "resetPrompt")]
    pub reset_prompt: String,
    /// A text to display when the current user is not allowed to use the bot.
    /// JSON key: `notAllowedPrompt`
    #[serde(default = "default_not_allowed_prompt", rename = "notAllowedPrompt")]
    pub not_allowed_prompt: String,
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
    openai_api_timeout: u64 = 10,
    stream_throttle_interval: u64 = 500,
    conversation_limit: u64 = 20,
    renders_markdown: bool = false,
    openai_gpt_model: String = "gpt-3.5-turbo".to_owned(),
}

define_defaults!(I18nStrings {
    api_error_prompt: String = "Hmm, something went wrong...".to_owned(),
    reset_prompt: String = "\u{26A0} Session is reset!".to_owned(),
    not_allowed_prompt: String = "Sadly, you are not allowed to use this bot currently.".to_owned(),
});
