//! Configuration-related types.
//!
//! The configuration can be represented in and deserialized from JSON,
//! here is an example:
//!
//! ```json
//! {
//!   "openaiAPIKey": "sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
//!   "botToken": "8888888888:XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
//!   "conversationLimit": 30,
//!   "databasePath": "telegpt.sqlite",
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

    /// A timeout in seconds for waiting for the OpenAI server response.
    /// JSON key: `openaiAPITimeout`
    #[serde(default = "default_openai_api_timeout", rename = "openaiAPITimeout")]
    pub openai_api_timeout: u64,

    /// A set of usernames that represents the admin users, who can use
    /// admin commands.
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
}

define_defaults!(I18nStrings {
    api_error_prompt: String = "Hmm, something went wrong...".to_owned(),
    reset_prompt: String = "\u{26A0} Session is reset!".to_owned(),
    not_allowed_prompt: String = "Sadly, you are not allowed to use this bot currently.".to_owned(),
});
