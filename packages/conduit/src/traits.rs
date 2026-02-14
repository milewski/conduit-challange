use crate::node::{NodeError, SharedValue};
use crate::registry::Payload;
use async_trait::async_trait;
use std::any::Any;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::Mutex;

// -- Conversion traits for Input/Output types --

/// Trait for types that can be constructed from an engine Payload.
pub trait NodeInput: Send + Sync + 'static {
    fn from_payload(payload: &Payload) -> Result<Self, NodeError>
    where
        Self: Sized;
    fn field_names() -> Vec<&'static str>
    where
        Self: Sized;
}

/// Trait for types that can be converted into engine output entries.
pub trait NodeOutput: Send + Sync + 'static {
    fn into_outputs(self) -> Vec<(&'static str, SharedValue)>;
    fn field_names() -> Vec<&'static str>
    where
        Self: Sized;
}

// -- Blanket impls: NodeInput for common single-value types --

macro_rules! impl_node_input_single {
    ($t:ty) => {
        impl NodeInput for $t {
            fn from_payload(payload: &Payload) -> Result<Self, NodeError> {
                payload
                    .get("input")
                    .and_then(|v| v.downcast_ref::<$t>())
                    .cloned()
                    .ok_or(NodeError::MissingInput("input".to_string()))
            }
            fn field_names() -> Vec<&'static str> {
                vec!["input"]
            }
        }
    };
}

impl_node_input_single!(String);
impl_node_input_single!(Vec<u8>);
impl_node_input_single!(u32);
impl_node_input_single!(bool);
impl_node_input_single!(f64);

// -- Blanket impls: NodeOutput for common single-value types --

impl NodeOutput for () {
    fn into_outputs(self) -> Vec<(&'static str, SharedValue)> {
        vec![]
    }
    fn field_names() -> Vec<&'static str> {
        vec![]
    }
}

macro_rules! impl_node_output_single {
    ($t:ty) => {
        impl NodeOutput for $t {
            fn into_outputs(self) -> Vec<(&'static str, SharedValue)> {
                vec![("output", Arc::new(self) as SharedValue)]
            }
            fn field_names() -> Vec<&'static str> {
                vec!["output"]
            }
        }
    };
}

impl_node_output_single!(String);
impl_node_output_single!(Vec<u8>);
impl_node_output_single!(u32);
impl_node_output_single!(bool);
impl_node_output_single!(f64);

// -- Dynamic Input Helper --

pub struct DynamicInput(pub Vec<(&'static str, SharedValue)>);

impl NodeOutput for DynamicInput {
    fn into_outputs(self) -> Vec<(&'static str, SharedValue)> {
        self.0
    }

    fn field_names() -> Vec<&'static str> {
        Vec::new() // Not used by engine currently
    }
}

pub trait AsInput {
    type Output: std::any::Any + Send + Sync;

    fn as_input(self) -> Self::Output;
}

impl AsInput for &str {
    type Output = String;

    fn as_input(self) -> String {
        self.to_string()
    }
}

macro_rules! impl_as_input_identity {
    ($kind:ty) => {
        impl AsInput for $kind {
            type Output = $kind;
            fn as_input(self) -> $kind {
                self
            }
        }
    };
}

impl_as_input_identity!(u8);
impl_as_input_identity!(u16);
impl_as_input_identity!(u32);
impl_as_input_identity!(u64);
impl_as_input_identity!(u128);
impl_as_input_identity!(usize);
impl_as_input_identity!(i8);
impl_as_input_identity!(i16);
impl_as_input_identity!(i32);
impl_as_input_identity!(i64);
impl_as_input_identity!(i128);
impl_as_input_identity!(isize);
impl_as_input_identity!(f32);
impl_as_input_identity!(f64);
impl_as_input_identity!(bool);
impl_as_input_identity!(String);
impl_as_input_identity!(Vec<u8>);
impl_as_input_identity!(Vec<String>);
impl_as_input_identity!(Vec<&'static str>);

#[derive(Clone)]
pub struct EventData {
    pub name: String,
    pub value: Option<SharedValue>,
}

impl EventData {
    pub fn with_value(name: impl Into<String>, value: impl Any + Send + Sync + 'static) -> Self {
        Self {
            name: name.into(),
            value: Some(Arc::new(value)),
        }
    }

    pub fn without_value(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: None,
        }
    }
}

pub trait NodeEvent: Send + Sync + 'static {
    fn into_parts(self) -> EventData;
}

impl NodeEvent for () {
    fn into_parts(self) -> EventData {
        EventData::without_value(String::new())
    }
}

pub struct Emitter<Event>
where
    Event: NodeEvent,
{
    events: Arc<Mutex<Vec<EventData>>>,
    marker: PhantomData<Event>,
}

impl<Event> Clone for Emitter<Event>
where
    Event: NodeEvent,
{
    fn clone(&self) -> Self {
        Self {
            events: self.events.clone(),
            marker: PhantomData,
        }
    }
}

impl<Event> Default for Emitter<Event>
where
    Event: NodeEvent,
{
    fn default() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            marker: PhantomData,
        }
    }
}

impl<Event> Emitter<Event>
where
    Event: NodeEvent,
{
    pub async fn emit(&self, event: Event) {
        let mut events = self.events.lock().expect("event emitter mutex poisoned");
        events.push(event.into_parts());
    }

    pub fn into_events(self) -> Vec<EventData> {
        let mut events = self.events.lock().expect("event emitter mutex poisoned");
        std::mem::take(&mut *events)
    }
}

pub struct NodeExecutionResult {
    pub outputs: Vec<(&'static str, SharedValue)>,
    pub events: Vec<EventData>,
}

// -- User-facing trait (not object-safe) --

/// Trait that node authors implement. Associated types define input/output shape;
/// `run` contains the node logic.
#[async_trait]
pub trait ExecutableNode: Send + Sync + 'static {
    type Input: NodeInput;
    type Output: NodeOutput;
    type Event: NodeEvent;

    async fn run(&self, input: Self::Input, emitter: Emitter<Self::Event>) -> Result<Self::Output, NodeError>;
}

// -- Object-safe trait used by the engine --

/// Engine-internal trait. `#[derive(Node)]` generates this automatically.
#[async_trait]
pub trait DynNode: Send + Sync {
    fn name(&self) -> &'static str;

    async fn run_with_payload(&self, payload: Payload) -> Result<NodeExecutionResult, NodeError>;

    fn input_fields(&self) -> Vec<&'static str>;
    fn output_fields(&self) -> Vec<&'static str>;
}
