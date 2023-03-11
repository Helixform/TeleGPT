//! An out-of-box ChatGPT bot for Telegram.
//!
//! TeleGPT is a Telegram bot based on [`teloxide`](https://docs.rs/teloxide/latest/teloxide/)
//! framework and [`async_openai`](https://docs.rs/async-openai/latest/async_openai/). It
//! provides an easy way to interact with the latest ChatGPT models utilizing your own API key.
//!
//! ## Getting Started
//!
//! ### Using via CLI
//!
//! TeleGPT features a single-binary executable, you can serve the bot by simply running the
//! command below:
//!
//! ```shell
//! $ /path/to/telegpt -c your_config.json
//! ```
//!
//! The configuration is described in [`config`] module.
//!
//! ### Using via library
//!
//! TeleGPT can also be used as a library, therefore you can run it along with your code in
//! the same process. Checkout the [`app`] module to learn more about it.
//!
//! *Note: Currently there are no APIs to let you interact with the bot, and it's a planned
//! feature to implement soon.*
//!
//! ## Further Readings
//!
//! For more information, see the [GitHub repository](https://github.com/IcyStudio/TeleGPT/).

#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate async_trait;

pub mod app;
pub mod config;
mod database;
mod dispatcher;
mod module_mgr;
mod modules;
mod types;
mod utils;
