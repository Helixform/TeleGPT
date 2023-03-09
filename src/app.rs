use anyhow::Error;
use teloxide::{prelude::*, types::MenuButton};

use crate::{
    config::{Config, SharedConfig},
    database::{DatabaseManager, FileDatabaseProvider, InMemDatabaseProvider},
    dispatcher::build_dispatcher,
    module_mgr::ModuleManager,
    modules::chat::Chat,
    modules::stats::{Stats, StatsManager},
    types::HandlerResult,
};

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

    let mut built_dispatcher = build_dispatcher(bot, module_mgr);
    info!("Bot is started!");
    built_dispatcher.dispatch().await;
}
