use crate::node::SharedValue;
use crate::traits::DynNode;
use heck::ToSnakeCase;
use std::collections::HashMap;

pub struct NodeRegistry {
    factories: HashMap<String, fn(value: Payload) -> Box<dyn DynNode>>,
}

pub type Payload = HashMap<String, SharedValue>;

pub trait RegisterableNode {
    fn register_type(registry: &mut NodeRegistry);
    fn type_name() -> &'static str;
}

pub struct NodeRegistration {
    register_fn: fn(&mut NodeRegistry),
}

impl NodeRegistration {
    pub const fn new<T: RegisterableNode>() -> Self {
        Self {
            register_fn: |registry| T::register_type(registry),
        }
    }

    pub fn register(&self, registry: &mut NodeRegistry) {
        (self.register_fn)(registry);
    }
}

inventory::collect!(NodeRegistration);

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    pub fn register<F: DynNode + Default + 'static>(&mut self, name: &str) {
        self.factories
            .insert(name.to_snake_case(), |_inputs| Box::<F>::new(F::default()));
    }

    pub fn create(&self, name: &str, _settings: Payload) -> Option<Box<dyn DynNode>> {
        self.factories.get(name).map(|factory| factory(Payload::new()))
    }

    pub fn has(&self, name: &str) -> bool {
        self.factories.contains_key(name)
    }

    pub fn register_all(&mut self) {
        for registration in inventory::iter::<NodeRegistration> {
            registration.register(self);
        }
    }
}
