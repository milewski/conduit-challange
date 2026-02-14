use crate::functional_node;
use crate::node::NodeError;
use crate::traits::{Emitter, EventData, ExecutableNode, NodeEvent};
use async_trait::async_trait;
use conduit_derive::{Node, NodeInput};

#[functional_node]
fn multiplier(#[input] a: u32, b: u32) -> u32 {
    a * b
}

#[functional_node]
fn subtract(a: u32, b: u32) -> u32 {
    a - b
}

#[functional_node]
fn adder(#[input] a: u32, b: u32) -> u32 {
    a + b
}

#[functional_node]
async fn sleep(#[input] duration: u64) {
    tokio::time::sleep(tokio::time::Duration::from_millis(duration)).await;
}

#[derive(Debug)]
enum Events {
    Done { current_count: u32 },
    Complete { value: u32 },
    Error { value: u32 },
    Message { value: u32 },
}

impl NodeEvent for Events {
    fn into_parts(self) -> EventData {
        match self {
            Events::Done { current_count } => EventData::with_value("done", current_count),
            Events::Complete { value } => EventData::with_value("complete", value),
            Events::Error { value } => EventData::with_value("error", value),
            Events::Message { value } => EventData::with_value("message", value),
        }
    }
}

#[derive(NodeInput)]
struct TaskInput {
    count: u32,
}

#[derive(Node, Default)]
struct Task;

#[async_trait]
impl ExecutableNode for Task {
    type Input = TaskInput;
    type Output = ();
    type Event = Events;

    async fn run(&self, input: Self::Input, emitter: Emitter<Self::Event>) -> Result<Self::Output, NodeError> {
        for current_count in 1..=input.count {
            emitter.emit(Events::Done { current_count }).await;
        }
        emitter.emit(Events::Complete { value: 10 }).await;
        emitter.emit(Events::Error { value: 20 }).await;
        emitter.emit(Events::Message { value: 30 }).await;
        Ok(())
    }
}

#[functional_node]
fn echo(#[input] input: u32) -> u32 {
    input
}
