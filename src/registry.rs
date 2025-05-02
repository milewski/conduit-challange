use crate::node::SharedValue;
use crate::traits::Node;
use bevy_ecs::prelude::Resource;
use std::collections::HashMap;

#[derive(Resource)]
pub struct NodeRegistry {
    factories: HashMap<String, fn(value: Payload) -> Box<dyn Node>>,
}

pub type Payload = HashMap<String, SharedValue>;

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    pub fn register<F: Node + for<'a> From<Payload> + 'static>(&mut self, name: &str) {
        self.factories.insert(name.to_string(), |inputs| Box::<F>::new(inputs.into()));
    }

    pub fn create(&self, name: &str, settings: Payload) -> Option<Box<dyn Node>> {
        self.factories.get(name).map(|factory| factory(settings))
    }
}
