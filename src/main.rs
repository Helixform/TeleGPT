#![feature(fn_traits)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;

mod database;
mod dispatcher;
mod module_mgr;
mod modules;
mod utils;
mod config;

use std::error::Error;

use pretty_env_logger;
use teloxide::{prelude::*, types::MenuButton, Bot};

use crate::database::InMemDatabaseProvider;
use module_mgr::ModuleManager;
use modules::chat::Chat;
use modules::stats::{Stats, StatsManager};

pub(crate) type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

pub(crate) async fn noop_handler() -> HandlerResult {
    Ok(())
}

async fn update_menu(bot: Bot, module_mgr: &mut ModuleManager) -> HandlerResult {
    let mut commands = vec![];
    module_mgr.with_all_modules(|m| commands.extend(m.commands().into_iter()));
    Ok(bot.set_my_commands(commands).await.and(Ok(()))?)
}

async fn init_bot(module_mgr: &mut ModuleManager) -> Result<Bot, Box<dyn Error + Send + Sync>> {
    let bot = Bot::from_env();
    bot.set_chat_menu_button()
        .menu_button(MenuButton::Commands)
        .await?;
    update_menu(bot.clone(), module_mgr).await?;
    Ok(bot)
}

#[tokio::main]
async fn main() {
    pretty_env_logger::formatted_timed_builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    info!("Bot is starting...");

    info!("Initializing DatabaseManager...");
    let db_mgr = database::DatabaseManager::with_db_provider(InMemDatabaseProvider).unwrap();

    let stats_mgr = match StatsManager::with_db_manager(db_mgr).await {
        Ok(stats_mgr) => stats_mgr,
        Err(err) => {
            error!("Failed to init StatsManager: {}", err);
            return;
        }
    };

    let mut module_mgr = ModuleManager::new();
    module_mgr.register_module(Chat);
    module_mgr.register_module(Stats::new(stats_mgr));
    info!("Modules are registered!");

    let bot = match init_bot(&mut module_mgr).await {
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
