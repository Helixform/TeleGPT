#[macro_use]
extern crate log;

mod chat;
mod comp_mgr;

use std::sync::Arc;

use pretty_env_logger;
use teloxide::{prelude::*, Bot};

use chat::SessionManager;
use comp_mgr::ComponentManager;

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

async fn noop_handler() -> HandlerResult {
    Ok(())
}

async fn start_dispatcher(bot: Bot, component_mgr: ComponentManager) {
    Dispatcher::builder(
        bot,
        Update::filter_message()
            .chain(dptree::filter_async(message_filter))
            .branch(dptree::filter_async(chat::handle_chat_message).endpoint(noop_handler))
            .branch(dptree::endpoint(default_handler)),
    )
    .dependencies(dptree::deps![Arc::new(component_mgr)])
    .enable_ctrlc_handler()
    .build()
    .dispatch()
    .await;
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

    let mut component_mgr = ComponentManager::new();
    component_mgr.register_component(SessionManager::new());
    component_mgr.register_component(chat::create_openai_client());
    info!("Components are registered!");

    let bot = init_bot();
    start_dispatcher(bot, component_mgr).await;
}
