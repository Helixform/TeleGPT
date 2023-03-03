#[macro_use]
extern crate log;

mod chat;
mod module_mgr;

use pretty_env_logger;
use teloxide::{prelude::*, Bot};

use chat::Chat;
use module_mgr::ModuleManager;

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

async fn default_handler(_bot: Bot, msg: Message) -> HandlerResult {
    warn!("Message ({}) is not handled!", msg.id);
    Ok(())
}

pub(crate) async fn noop_handler() -> HandlerResult {
    Ok(())
}

async fn start_dispatcher(bot: Bot, module_mgr: ModuleManager) {
    // Load dependencies.
    let mut dep_map = DependencyMap::new();
    module_mgr.with_all_modules(|m| m.register_dependency(&mut dep_map));

    // Build handler chain.
    let mut handler = Some(Update::filter_message().chain(dptree::filter_async(message_filter)));
    module_mgr.with_all_modules(|m| {
        let new_handler = handler.take().unwrap().branch(m.handler_chain());
        handler.replace(new_handler);
    });
    handler = handler.map(|h| h.branch(dptree::endpoint(default_handler)));

    let mut dispatcher = Dispatcher::builder(bot, handler.unwrap())
        .dependencies(dep_map)
        .enable_ctrlc_handler()
        .build();

    info!("Bot is started!");
    dispatcher.dispatch().await;
}

fn init_bot() -> Bot {
    Bot::from_env()
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

    let bot = init_bot();
    start_dispatcher(bot, module_mgr).await;
}
