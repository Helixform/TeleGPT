pub(crate) mod db_provider;
mod stats_mgr;

use std::fmt::Write;

use teloxide::dispatching::DpHandlerDescription;
use teloxide::prelude::*;
use teloxide::types::BotCommand;

use crate::module_mgr::Module;
use crate::utils::dptree_ext;
use crate::HandlerResult;
pub(crate) use db_provider::DatabaseProvider;
pub(crate) use stats_mgr::StatsManager;

pub(crate) struct Stats {
    stats_mgr: Option<StatsManager>,
}

impl Stats {
    pub(crate) fn new(stats_mgr: StatsManager) -> Self {
        Self {
            stats_mgr: Some(stats_mgr),
        }
    }
}

async fn handle_show_stats(bot: Bot, msg: Message, stats_mgr: StatsManager) -> HandlerResult {
    let mut reply_text = String::new();
    if let Some(from_username) = msg.from().and_then(|u| u.username.as_ref()) {
        let user_usage = stats_mgr.query_usage(Some(from_username.to_owned())).await;
        write!(&mut reply_text, "Your token usage: {}\n", user_usage)?;
    }
    let total_usage = stats_mgr.query_usage(None).await;
    write!(&mut reply_text, "Total token usage: {}", total_usage)?;

    bot.send_message(msg.chat.id, reply_text)
        .reply_to_message_id(msg.id)
        .send()
        .await?;

    Ok(())
}

impl Module for Stats {
    fn register_dependency(&mut self, dep_map: &mut DependencyMap) {
        dep_map.insert(self.stats_mgr.take().unwrap());
    }

    fn handler_chain(
        &self,
    ) -> Handler<'static, DependencyMap, HandlerResult, DpHandlerDescription> {
        dptree::entry().branch(
            Update::filter_message()
                .filter(dptree_ext::command_filter("stats"))
                .endpoint(handle_show_stats),
        )
    }

    fn commands(&self) -> Vec<BotCommand> {
        return vec![BotCommand::new(
            "stats",
            "Show the token usage and other stats",
        )];
    }
}
