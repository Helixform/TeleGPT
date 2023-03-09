#![doc(hidden)]

use teloxide::prelude::*;
use teloxide::types::BotCommand;

use crate::types::TeloxideHandler;

pub trait Module {
    fn register_dependency(&mut self, dep_map: &mut DependencyMap);

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
}
