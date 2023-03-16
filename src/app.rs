//! The entry of the bot application.
//!
//! You normally don't use this crate directly. Instead, you run the binary
//! to use the bot. When integrating the bot into other programs, invoke
//! [`run`] function to start the bot server.

use anyhow::Error;
use teloxide::{
    prelude::*,
    types::{BotCommand, MenuButton},
};

use crate::{
    config::{Config, SharedConfig},
    database::{DatabaseManager, FileDatabaseProvider, InMemDatabaseProvider},
    dispatcher::build_dispatcher,
    module_mgr::ModuleManager,
    modules::{admin::Admin, chat::Chat, openai::OpenAI, prefs::Prefs, stats::Stats},
    types::HandlerResult,
};

async fn update_menu(bot: Bot, module_mgr: &mut ModuleManager) -> HandlerResult {
    let mut commands = vec![];
    module_mgr.with_all_modules(|m| {
        commands.extend(
            m.commands()
                .into_iter()
                .filter(|command| !command.is_hidden)
                .map(|command| BotCommand::new(command.command, command.description)),
        )
    });
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

/// Starts bot server and blocks the caller until the bot is requested
/// to shutdown.
pub async fn run(config: SharedConfig) {
    debug!("Initializing database...");
    let db_mgr = if let Some(database_path) = &config.database_path {
        DatabaseManager::with_db_provider(FileDatabaseProvider::new(database_path))
    } else {
        DatabaseManager::with_db_provider(InMemDatabaseProvider)
    }
    .unwrap();

    debug!("Initializing modules...");
    let mut module_mgr = ModuleManager::new();
    module_mgr.register_module(crate::modules::config::Config::new(config.clone()));
    module_mgr.register_module(OpenAI);
    module_mgr.register_module(Prefs::new(db_mgr.clone()));
    module_mgr.register_module(Admin::new(db_mgr.clone()));
    module_mgr.register_module(Stats::new(db_mgr.clone()));
    module_mgr.register_module(Chat);

    info!("Initializing bot...");
    let bot = match init_bot(&config, &mut module_mgr).await {
        Ok(bot) => bot,
        Err(err) => {
            error!("Failed to init bot: {}", err);
            return;
        }
    };

    let mut built_dispatcher = match build_dispatcher(bot, module_mgr).await {
        Ok(dispatcher) => dispatcher,
        Err(err) => {
            error!("Failed to init dispatcher: {}", err);
            return;
        }
    };
    info!("Bot is started!");
    built_dispatcher.dispatch().await;
}
