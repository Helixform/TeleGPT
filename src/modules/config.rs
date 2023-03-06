use teloxide::dispatching::DpHandlerDescription;
use teloxide::prelude::*;

use crate::{config::SharedConfig, module_mgr::Module, HandlerResult};

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

impl Module for Config {
    fn register_dependency(&mut self, dep_map: &mut DependencyMap) {
        dep_map.insert(self.config.take().unwrap());
    }

    fn handler_chain(
        &self,
    ) -> Handler<'static, DependencyMap, HandlerResult, DpHandlerDescription> {
        dptree::entry()
    }
}
