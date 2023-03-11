use anyhow::Error;
use teloxide::dispatching::DpHandlerDescription;
use teloxide::prelude::*;

use crate::{config::SharedConfig, module_mgr::Module, types::HandlerResult};

pub(crate) struct Config {
    config: Option<SharedConfig>,
}

impl Config {
    pub(crate) fn new(config: SharedConfig) -> Self {
        Self {
            config: Some(config),
        }
    }
}

#[async_trait]
impl Module for Config {
    async fn register_dependency(&mut self, dep_map: &mut DependencyMap) -> Result<(), Error> {
        dep_map.insert(self.config.take().unwrap());
        Ok(())
    }

    fn handler_chain(
        &self,
    ) -> Handler<'static, DependencyMap, HandlerResult, DpHandlerDescription> {
        dptree::entry()
    }
}
