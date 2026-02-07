use crate::node::SharedValue;
use crate::traits::ExecutableNode;
use std::collections::HashMap;
use heck::ToSnakeCase;

pub struct NodeRegistry {
    factories: HashMap<String, fn(value: Payload) -> Box<dyn ExecutableNode>>,
}

pub type Payload = HashMap<String, SharedValue>;

// Trait for node types that can be registered
pub trait RegisterableNode {
    fn register_type(registry: &mut NodeRegistry);
    fn type_name() -> &'static str;
}

// A struct to hold node registration information for the inventory pattern
pub struct NodeRegistration {
    register_fn: fn(&mut NodeRegistry),
}

impl NodeRegistration {
    // Make this a const function so it can be used in static contexts
    pub const fn new<T: RegisterableNode>() -> Self {
        Self {
            register_fn: |registry| T::register_type(registry),
        }
    }
    
    pub fn register(&self, registry: &mut NodeRegistry) {
        (self.register_fn)(registry);
    }
}

// Create a global inventory for node registrations
inventory::collect!(NodeRegistration);

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    pub fn register<F: ExecutableNode + for<'a> From<Payload> + 'static>(&mut self, name: &str) {
        self.factories.insert(name.to_snake_case(), |inputs| Box::<F>::new(inputs.into()));
    }

    pub fn create(&self, name: &str, settings: Payload) -> Option<Box<dyn ExecutableNode>> {
        self.factories.get(name).map(|factory| factory(settings))
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
