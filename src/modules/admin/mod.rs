mod handler_filter;
mod member_mgr;

use std::sync::Arc;

use anyhow::Error;
use teloxide::dptree::di::DependencySupplier;
use teloxide::prelude::*;
use teloxide::types::BotCommand;

use crate::{
    config::SharedConfig,
    database::DatabaseManager,
    module_mgr::Module,
    modules::prefs::PreferencesManager,
    types::{HandlerResult, TeloxideHandler},
    utils::dptree_ext::{command_filter, CommandArgs},
};
pub(crate) use handler_filter::check_user_permission;
pub(crate) use member_mgr::MemberManager;

pub(crate) struct Admin {
    db_mgr: DatabaseManager,
}

impl Admin {
    pub(crate) fn new(db_mgr: DatabaseManager) -> Self {
        Self { db_mgr }
    }
}

fn check_admin(msg: &Message, config: &SharedConfig) -> bool {
    if let Some(user) = msg.from() {
        if let Some(username) = &user.username {
            return config.admin_usernames.contains(username);
        }
    }
    false
}

macro_rules! check_admin {
    ($bot:expr, $msg:expr, $conf:expr) => {
        if !check_admin(&$msg, &$conf) {
            let _ = $bot
                .send_message(
                    $msg.chat.id,
                    "You don't have the right to execute admin commands!",
                )
                .await;
            warn!(
                "Non-admin user \"{}\" tried to execute admin commands",
                $msg.from()
                    .and_then(|u| u.username.clone())
                    .unwrap_or("<unknown>".to_owned())
            );
            return Ok(());
        }
    };
}

async fn set_public(
    bot: Bot,
    msg: Message,
    args: CommandArgs,
    member_mgr: MemberManager,
    config: SharedConfig,
) -> HandlerResult {
    check_admin!(bot, msg, config);

    let value = match args.0.as_str() {
        "yes" | "on" | "true" | "1" => true,
        "no" | "off" | "false" | "0" => false,
        _ => {
            bot.send_message(
                msg.chat.id,
                "Invalid value, possible values are \"yes\", \"no\"",
            )
            .await?;
            return Ok(());
        }
    };

    match member_mgr.set_public_usable(value).await {
        Ok(_) => {
            bot.send_message(msg.chat.id, format!("Success, current status: {}", value))
                .await?;
        }
        Err(err) => {
            error!("Failed to set public usability: {}", err);
            bot.send_message(
                msg.chat.id,
                "Failed to set public usability, internal error occurred",
            )
            .await?;
        }
    }

    Ok(())
}

async fn add_member(
    bot: Bot,
    msg: Message,
    args: CommandArgs,
    member_mgr: MemberManager,
    config: SharedConfig,
) -> HandlerResult {
    check_admin!(bot, msg, config);

    let username = args.0;
    if username.is_empty() || username.contains(' ') {
        bot.send_message(msg.chat.id, "Invalid username").await?;
        return Ok(());
    }

    match member_mgr.add_member(username).await {
        Ok(value) => {
            bot.send_message(
                msg.chat.id,
                if value {
                    "Success"
                } else {
                    "Failed to add member, maybe it's already added"
                },
            )
            .await?;
        }
        Err(err) => {
            error!("Failed to add member: {}", err);
            bot.send_message(msg.chat.id, "Failed to add member, internal error occurred")
                .await?;
        }
    }

    Ok(())
}

async fn delete_member(
    bot: Bot,
    msg: Message,
    args: CommandArgs,
    member_mgr: MemberManager,
    config: SharedConfig,
) -> HandlerResult {
    check_admin!(bot, msg, config);

    let username = args.0;
    if username.is_empty() || username.contains(' ') {
        bot.send_message(msg.chat.id, "Invalid username").await?;
        return Ok(());
    }

    match member_mgr.delete_member(username).await {
        Ok(value) => {
            bot.send_message(
                msg.chat.id,
                if value {
                    "Success"
                } else {
                    "The member is not existed."
                },
            )
            .await?;
        }
        Err(err) => {
            error!("Failed to delete member: {}", err);
            bot.send_message(
                msg.chat.id,
                "Failed to delete member, internal error occurred",
            )
            .await?;
        }
    }

    Ok(())
}

#[async_trait]
impl Module for Admin {
    async fn register_dependency(&mut self, dep_map: &mut DependencyMap) -> Result<(), Error> {
        let prefs_mgr: Arc<PreferencesManager> = dep_map.get();
        let config: Arc<SharedConfig> = dep_map.get();

        let member_mgr = MemberManager::new(
            self.db_mgr.clone(),
            prefs_mgr.as_ref().clone(),
            config.as_ref().clone(),
        )
        .await?;
        dep_map.insert(member_mgr);
        Ok(())
    }

    fn handler_chain(&self) -> TeloxideHandler {
        dptree::entry().branch(
            Update::filter_message()
                .branch(dptree::filter_map(command_filter("set_public")).endpoint(set_public))
                .branch(dptree::filter_map(command_filter("add_member")).endpoint(add_member))
                .branch(dptree::filter_map(command_filter("del_member")).endpoint(delete_member)),
        )
    }

    fn commands(&self) -> Vec<BotCommand> {
        // Don't reveal admin commands to other users.
        vec![]
    }
}
