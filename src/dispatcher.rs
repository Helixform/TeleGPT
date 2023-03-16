#![doc(hidden)]

use std::sync::Arc;

use anyhow::Error;
use teloxide::prelude::*;
use teloxide::types::{Me, MediaKind, MessageCommon, MessageEntityKind, MessageKind, User};
use tokio::sync::Mutex;

use crate::{
    conversation::ConversationManager,
    module_mgr::ModuleManager,
    types::{HandlerResult, TeloxideDispatcher},
    utils::{dptree_ext::command_filter, HandlerExt},
};

fn can_respond_group_message(me: &User, msg: &Message) -> bool {
    if let MessageKind::Common(MessageCommon {
        media_kind: MediaKind::Text(ref media_text),
        ..
    }) = msg.kind
    {
        let text = media_text.text.as_str();
        // Command message:
        if text.starts_with('/') {
            return true;
        }
        // Mention message:
        if media_text.entities.iter().any(|ent| match &ent.kind {
            MessageEntityKind::Mention => {
                let mention_username = &text[ent.offset..(ent.offset + ent.length)];
                if mention_username.is_empty() {
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

    false
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

pub(crate) async fn build_dispatcher(
    bot: Bot,
    mut module_mgr: ModuleManager,
) -> Result<TeloxideDispatcher, Error> {
    // Load dependencies.
    struct DependencyMapHolder {
        dep_map: Option<DependencyMap>,
    }
    let dep_map_holder = Arc::new(Mutex::new(DependencyMapHolder {
        dep_map: Some(DependencyMap::new()),
    }));
    module_mgr
        .with_all_modules_async(|m| {
            let dep_map_holder = Arc::clone(&dep_map_holder);
            async move {
                let mut locked_dep_map_holder = dep_map_holder.lock().await;
                let dep_map = locked_dep_map_holder.dep_map.as_mut().unwrap();
                m.register_dependency(dep_map).await?;
                Ok(())
            }
        })
        .await?;
    let mut dep_map = dep_map_holder.lock().await.dep_map.take().unwrap();

    // Build conversation manager and handler chain.
    let conversation_mgr = ConversationManager::new();
    let conversation_handler = conversation_mgr.make_handler();
    dep_map.insert(conversation_mgr);

    // Build command handler chain.
    let mut command_handler = Some(Update::filter_message());
    module_mgr.with_all_modules(|m| {
        let mut new_command_handler = command_handler.take().unwrap();
        for command in m.commands() {
            new_command_handler = new_command_handler
                .branch(dptree::filter_map(command_filter(command.command)).chain(command.handler));
        }
        command_handler.replace(new_command_handler);
    });

    // Build handler chain.
    let mut biz_handler = Some(dptree::entry());
    module_mgr.with_all_modules(|m| {
        let new_biz_handler = biz_handler.take().unwrap().branch(m.filter_handler());
        biz_handler.replace(new_biz_handler);
    });
    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .filter_async(message_filter)
                .endpoint(noop_handler),
        ) // Pre-handler and filter for message updates.
        .branch(conversation_handler) // Conversation handlers.
        .branch(command_handler.unwrap()) // Command handlers.
        .branch(biz_handler.unwrap()) // Core business handlers.
        .branch(dptree::endpoint(default_handler)) // Fallback handler.
        .post_chain(dptree::endpoint(noop_handler)); // For future extensions.

    let dispatcher = Dispatcher::builder(bot, handler)
        .dependencies(dep_map)
        .enable_ctrlc_handler()
        .build();
    Ok(dispatcher)
}
