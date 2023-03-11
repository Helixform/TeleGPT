mod prefs_mgr;

use anyhow::Error;
use teloxide::prelude::*;
use teloxide::types::BotCommand;

use crate::{database::DatabaseManager, module_mgr::Module, types::TeloxideHandler};
pub(crate) use prefs_mgr::PreferencesManager;

pub(crate) struct Prefs {
    db_mgr: DatabaseManager,
}

impl Prefs {
    pub(crate) fn new(db_mgr: DatabaseManager) -> Self {
        Self { db_mgr }
    }
}

#[async_trait]
impl Module for Prefs {
    async fn register_dependency(&mut self, dep_map: &mut DependencyMap) -> Result<(), Error> {
        let prefs_mgr = PreferencesManager::with_db_manager(self.db_mgr.clone()).await?;
        dep_map.insert(prefs_mgr);
        Ok(())
    }

    fn handler_chain(&self) -> TeloxideHandler {
        dptree::entry()
    }

    fn commands(&self) -> Vec<BotCommand> {
        vec![]
    }
}
