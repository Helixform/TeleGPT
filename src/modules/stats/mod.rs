pub(crate) mod db_provider;
mod stats_mgr;

use teloxide::dispatching::DpHandlerDescription;
use teloxide::prelude::*;
use teloxide::types::BotCommand;

use crate::module_mgr::Module;
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

impl Module for Stats {
    fn register_dependency(&mut self, dep_map: &mut DependencyMap) {
        dep_map.insert(self.stats_mgr.take().unwrap());
    }

    fn handler_chain(
        &self,
    ) -> Handler<'static, DependencyMap, HandlerResult, DpHandlerDescription> {
        dptree::entry()
    }

    fn commands(&self) -> Vec<BotCommand> {
        return vec![BotCommand::new(
            "stats",
            "Show the token usage and other stats",
        )];
    }
}
