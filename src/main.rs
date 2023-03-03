#[macro_use]
extern crate log;

mod chat;
mod dptree_util;
mod module_mgr;

use pretty_env_logger;
use teloxide::{prelude::*, types::MenuButton, Bot};

use chat::Chat;
use module_mgr::ModuleManager;

use crate::dptree_util::HandlerExt;

pub(crate) type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

async fn message_filter(_bot: Bot, msg: Message) -> bool {
    let from = msg
        .from()
        .map(|u| {
            let full_name = u.full_name();
            if full_name.is_empty() {
                u.id.to_string()
            } else {
                full_name
            }
        })
        .unwrap_or("<unknown>".to_owned());

    if let Some(text) = msg.text() {
        info!("{} sent a message: {}", from, text);
    } else {
        info!("{} sent a message: {:#?}", from, msg.kind);
    }

    true
}

async fn default_handler(msg: Message) -> HandlerResult {
    warn!("Message ({}) is not handled!", msg.id);
    Ok(())
}

async fn update_menu(bot: Bot, module_mgr: &ModuleManager) -> HandlerResult {
    let mut commands = vec![];
    module_mgr.with_all_modules(|m| commands.extend(m.commands().into_iter()));
    Ok(bot.set_my_commands(commands).await.and(Ok(()))?)
}

pub(crate) async fn noop_handler() -> HandlerResult {
    Ok(())
}

async fn start_dispatcher(bot: Bot, module_mgr: ModuleManager) {
    // Load dependencies.
    let mut dep_map = DependencyMap::new();
    module_mgr.with_all_modules(|m| m.register_dependency(&mut dep_map));

    // Build handler chain.
    let mut biz_handler = Some(dptree::entry());
    module_mgr.with_all_modules(|m| {
        let new_biz_handler = biz_handler.take().unwrap().branch(m.handler_chain());
        biz_handler.replace(new_biz_handler);
    });
    biz_handler = biz_handler.map(|h| h.branch(dptree::endpoint(default_handler)));
    let handler = Update::filter_message()
        .chain(dptree::filter_async(message_filter))
        .chain(biz_handler.unwrap())
        .post_chain(dptree::endpoint(noop_handler)); // For future extensions.

    let mut dispatcher = Dispatcher::builder(bot, handler)
        .dependencies(dep_map)
        .enable_ctrlc_handler()
        .build();

    info!("Bot is started!");
    dispatcher.dispatch().await;
}

async fn init_bot(module_mgr: &ModuleManager) -> Bot {
    let bot = Bot::from_env();
    bot.set_chat_menu_button()
        .menu_button(MenuButton::Commands)
        .await
        .expect("Failed to set menu button");
    update_menu(bot.clone(), module_mgr)
        .await
        .expect("Failed to update commands");
    bot
}

#[tokio::main]
async fn main() {
    pretty_env_logger::formatted_timed_builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    info!("Bot is starting...");

    let mut module_mgr = ModuleManager::new();
    module_mgr.register_module(Chat);
    info!("Modules are registered!");

    let bot = init_bot(&module_mgr).await;
    start_dispatcher(bot, module_mgr).await;
}
