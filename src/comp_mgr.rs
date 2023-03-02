use std::any::Any;
use std::collections::HashMap;

pub trait Component {
    fn key() -> &'static str;
}

pub struct ComponentManager {
    map: HashMap<String, Box<dyn Any + Sync + Send + 'static>>,
}

impl ComponentManager {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn register_component_with_key<K, C>(&mut self, key: K, comp: C)
    where
        K: AsRef<str>,
        C: Sync + Send + 'static,
    {
        self.map.insert(key.as_ref().to_owned(), Box::new(comp));
    }

    pub fn register_component<C>(&mut self, comp: C)
    where
        C: Component + Sync + Send + 'static,
    {
        let key = C::key();
        self.register_component_with_key(key, comp)
    }

    pub fn get_component_with_key<'m, K, C>(&'m self, key: K) -> Option<&'m C>
    where
        K: AsRef<str>,
        C: Sync + Send + 'static,
    {
        if let Some(any_comp) = self.map.get(key.as_ref()) {
            return any_comp.downcast_ref();
        }

        None
    }

    pub fn get_component<'m, C>(&'m self) -> Option<&'m C>
    where
        C: Component + Sync + Send + 'static,
    {
        let key = C::key();
        self.get_component_with_key(key)
    }
}
