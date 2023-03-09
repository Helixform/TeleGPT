#![doc(hidden)]

use teloxide::prelude::*;
use teloxide::types::{Me, MediaKind, MessageCommon, MessageEntityKind, MessageKind, User};

use crate::{
    module_mgr::ModuleManager,
    types::{HandlerResult, TeloxideDispatcher},
    utils::HandlerExt,
};

fn can_respond_group_message(me: &User, msg: &Message) -> bool {
    match msg.kind {
        MessageKind::Common(MessageCommon {
            media_kind: MediaKind::Text(ref media_text),
            ..
        }) => {
            let text = media_text.text.as_str();
            // Command message:
            if text.starts_with("/") {
                return true;
            }
            // Mention message:
            if media_text.entities.iter().any(|ent| match &ent.kind {
                MessageEntityKind::Mention => {
                    let mention_username = &text[ent.offset..(ent.offset + ent.length)];
                    if mention_username.len() < 1 {
                        return false; // Just in case.
                    }
                    me.username
                        .as_ref()
                        .map(|n| n == &mention_username[1..])
                        .unwrap_or(false)
                }
                _ => false,
            }) {
                return true;
            }
        }
        _ => {}
    };

    return false;
}

async fn message_filter(me: Me, msg: Message) -> bool {
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

    if !msg.chat.is_private() && !can_respond_group_message(&me.user, &msg) {
        return true;
    }

    if let Some(text) = msg.text() {
        debug!("{} sent a message: {}", from, text);
    } else {
        debug!("{} sent a message: {:#?}", from, msg.kind);
    }

    false
}

async fn default_handler(upd: Update) -> HandlerResult {
    warn!("Update ({}) is not handled!", upd.id);
    Ok(())
}

pub(crate) async fn noop_handler() -> HandlerResult {
    Ok(())
}

pub(crate) fn build_dispatcher(bot: Bot, mut module_mgr: ModuleManager) -> TeloxideDispatcher {
    // Load dependencies.
    let mut dep_map = DependencyMap::new();
    module_mgr.with_all_modules(|m| m.register_dependency(&mut dep_map));

    // Build handler chain.
    let mut biz_handler = Some(dptree::entry());
    module_mgr.with_all_modules(|m| {
        let new_biz_handler = biz_handler.take().unwrap().branch(m.handler_chain());
        biz_handler.replace(new_biz_handler);
    });
    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .filter_async(message_filter)
                .endpoint(noop_handler),
        ) // Pre-handler and filter for message updates.
        .branch(biz_handler.unwrap()) // Core business handlers.
        .branch(dptree::endpoint(default_handler)) // Fallback handler.
        .post_chain(dptree::endpoint(noop_handler)); // For future extensions.

    Dispatcher::builder(bot, handler)
        .dependencies(dep_map)
        .enable_ctrlc_handler()
        .build()
}
