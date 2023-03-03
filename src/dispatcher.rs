use std::error::Error;

use crate::noop_handler;
use crate::utils::HandlerExt;
use crate::{HandlerResult, ModuleManager};
use teloxide::{dispatching::DefaultKey, prelude::*, Bot};

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

pub(crate) fn build_dispatcher(
    bot: Bot,
    module_mgr: ModuleManager,
) -> Dispatcher<Bot, Box<dyn Error + Send + Sync + 'static>, DefaultKey> {
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

    Dispatcher::builder(bot, handler)
        .dependencies(dep_map)
        .enable_ctrlc_handler()
        .build()
}
