#![doc(hidden)]

use std::future::Future;

use anyhow::Error;
use teloxide::prelude::*;

use crate::types::TeloxideHandler;

pub struct Command {
    pub command: String,
    pub description: String,
    pub handler: TeloxideHandler,
    pub is_hidden: bool,
}

impl Command {
    pub fn new(command: &str, description: &str, handler: TeloxideHandler) -> Self {
        Self {
            command: command.to_owned(),
            description: description.to_owned(),
            handler,
            is_hidden: false,
        }
    }

    pub fn hidden(mut self) -> Self {
        self.is_hidden = true;
        self
    }
}

#[async_trait]
pub trait Module {
    async fn register_dependency(&mut self, dep_map: &mut DependencyMap) -> Result<(), Error>;

    fn filter_handler(&self) -> TeloxideHandler {
        dptree::entry()
    }

    fn commands(&self) -> Vec<Command> {
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
