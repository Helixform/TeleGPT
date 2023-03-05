use paste::paste;
use serde::Deserialize;

macro_rules! define_defaults {
    ($ty_name:ident { $($name:ident: $ty:ty = $default:expr,)* }) => {
        paste! {
            $(
                fn [<default_ $name>]() -> $ty {
                    $default
                }
            )*

            impl Default for $ty_name {
                fn default() -> Self {
                    Self {
                        $($name: [<default_ $name>](),)*
                    }
                }
            }
        }
    };
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub openai_api_timeout: u64,
    pub openai_api_key: String,
    pub telegram_bot_token: String,

    #[serde(default = "default_conversation_limit")]
    pub conversation_limit: u64,

    #[serde(default)]
    pub i18n: I18nStrings,
}

fn default_conversation_limit() -> u64 {
    20
}

#[derive(Debug, Clone, Deserialize)]
pub struct I18nStrings {
    #[serde(default = "default_api_error_prompt")]
    pub api_error_prompt: String,
    #[serde(default = "default_reset_prompt")]
    pub reset_prompt: String,
}

define_defaults!(I18nStrings {
    api_error_prompt: String = "Hmm, something went wrong...".to_owned(),
    reset_prompt: String = "\u{26A0} Session is reset!".to_owned(),
});
