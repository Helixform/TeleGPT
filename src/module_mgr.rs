#![doc(hidden)]

use std::future::Future;

use anyhow::Error;
use teloxide::prelude::*;
use teloxide::types::BotCommand;

use crate::types::TeloxideHandler;

#[async_trait]
pub trait Module {
    async fn register_dependency(&mut self, dep_map: &mut DependencyMap) -> Result<(), Error>;

    fn handler_chain(&self) -> TeloxideHandler;

    fn commands(&self) -> Vec<BotCommand> {
        vec![]
    }
}

pub struct ModuleManager {
    modules: Vec<Box<dyn Module + 'static>>,
}

impl ModuleManager {
    pub fn new() -> Self {
        Self { modules: vec![] }
    }

    pub fn register_module<C>(&mut self, module: C)
    where
        C: Module + 'static,
    {
        self.modules.push(Box::new(module));
    }

    pub fn with_all_modules<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut dyn Module),
    {
        for module in self.modules.iter_mut() {
            f(module.as_mut());
        }
    }

    pub async fn with_all_modules_async<'a, F, Fut>(&'a mut self, mut f: F) -> Result<(), Error>
    where
        F: FnMut(&'a mut dyn Module) -> Fut,
        Fut: Future<Output = Result<(), Error>> + 'a,
    {
        for module in self.modules.iter_mut() {
            f(module.as_mut()).await?;
        }
        Ok(())
    }
}
