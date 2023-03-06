#![feature(fn_traits)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;

mod config;
mod database;
mod dispatcher;
mod module_mgr;
mod modules;
mod utils;

use std::fs;

use anyhow::Error;
use clap::Parser;
use pretty_env_logger;
use serde_json;
use teloxide::{prelude::*, types::MenuButton, Bot};

use config::{Config, SharedConfig};
use database::InMemDatabaseProvider;
use module_mgr::ModuleManager;
use modules::chat::Chat;
use modules::stats::{Stats, StatsManager};

pub(crate) type HandlerResult = Result<(), Error>;

pub(crate) async fn noop_handler() -> HandlerResult {
    Ok(())
}

async fn update_menu(bot: Bot, module_mgr: &mut ModuleManager) -> HandlerResult {
    let mut commands = vec![];
    module_mgr.with_all_modules(|m| commands.extend(m.commands().into_iter()));
    Ok(bot.set_my_commands(commands).await.and(Ok(()))?)
}

async fn init_bot(config: &Config, module_mgr: &mut ModuleManager) -> Result<Bot, Error> {
    let bot = Bot::new(&config.telegram_bot_token);
    bot.set_chat_menu_button()
        .menu_button(MenuButton::Commands)
        .await?;
    update_menu(bot.clone(), module_mgr).await?;
    Ok(bot)
}

fn init_config(config_path: &str) -> Result<SharedConfig, Error> {
    let config_buf = fs::read(config_path)?;
    let config_json_str = String::from_utf8(config_buf)?;
    let config = serde_json::from_str(&config_json_str)?;
    Ok(SharedConfig::new(config))
}

#[derive(Parser)]
pub struct Args {
    #[arg(short = 'c', long = "config", default_value = "telegpt.config.json")]
    pub config_path: String,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::formatted_timed_builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let args = Args::parse();
    let config = match init_config(&args.config_path) {
        Ok(config) => config,
        Err(err) => {
            error!("Failed to load config: {}", err);
            return;
        }
    };

    info!("Initializing database...");
    let db_mgr = database::DatabaseManager::with_db_provider(InMemDatabaseProvider).unwrap();

    info!("Initializing modules...");
    let mut module_mgr = ModuleManager::new();
    module_mgr.register_module(modules::config::Config::new(config.clone()));
    module_mgr.register_module(Chat);
    let stats_mgr = match StatsManager::with_db_manager(db_mgr).await {
        Ok(stats_mgr) => stats_mgr,
        Err(err) => {
            error!("Failed to init StatsManager: {}", err);
            return;
        }
    };
    module_mgr.register_module(Stats::new(stats_mgr));

    info!("Initializing bot...");
    let bot = match init_bot(&config, &mut module_mgr).await {
        Ok(bot) => bot,
        Err(err) => {
            error!("Failed to init bot: {}", err);
            return;
        }
    };

    let mut built_dispatcher = dispatcher::build_dispatcher(bot, module_mgr);
    info!("Bot is started!");
    built_dispatcher.dispatch().await;
}
