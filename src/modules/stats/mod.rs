mod stats_mgr;

use std::fmt::Write;

use anyhow::Error;
use teloxide::prelude::*;

use crate::{
    database::DatabaseManager,
    module_mgr::{Command, Module},
    types::HandlerResult,
};
pub(crate) use stats_mgr::StatsManager;

pub(crate) struct Stats {
    db_mgr: DatabaseManager,
}

impl Stats {
    pub(crate) fn new(db_mgr: DatabaseManager) -> Self {
        Self { db_mgr }
    }
}

async fn handle_show_stats(bot: Bot, msg: Message, stats_mgr: StatsManager) -> HandlerResult {
    let mut reply_text = String::new();
    if let Some(from_username) = msg.from().and_then(|u| u.username.as_ref()) {
        let user_usage = stats_mgr
            .query_usage(Some(from_username.to_owned()))
            .await?;
        writeln!(&mut reply_text, "Your token usage: {}", user_usage)?;
    }
    let total_usage = stats_mgr.query_usage(None).await?;
    write!(&mut reply_text, "Total token usage: {}", total_usage)?;

    bot.send_message(msg.chat.id, reply_text)
        .reply_to_message_id(msg.id)
        .send()
        .await?;

    Ok(())
}

#[async_trait]
impl Module for Stats {
    async fn register_dependency(&mut self, dep_map: &mut DependencyMap) -> Result<(), Error> {
        let stats_mgr = StatsManager::with_db_manager(self.db_mgr.clone()).await?;
        dep_map.insert(stats_mgr);
        Ok(())
    }

    fn commands(&self) -> Vec<Command> {
        vec![Command::new(
            "stats",
            "Show the token usage and other stats",
            dptree::endpoint(handle_show_stats),
        )]
    }
}
