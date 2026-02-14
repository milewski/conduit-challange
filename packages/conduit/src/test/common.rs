use crate::functional_node;
use crate::node::NodeError;
use crate::traits::{Emitter, ExecutableNode};
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
    Done,
    Complete,
    Error,
    Message,
}

impl From<Events> for String {
    fn from(value: Events) -> Self {
        match value {
            Events::Done => "done".to_string(),
            Events::Complete => "complete".to_string(),
            Events::Error => "error".to_string(),
            Events::Message => "message".to_string(),
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
            emitter.emit(Events::Done, Some(current_count)).await;
        }
        emitter.emit(Events::Complete, Some(10u32)).await;
        emitter.emit(Events::Error, Some(20u32)).await;
        emitter.emit(Events::Message, Some(30u32)).await;
        Ok(())
    }
}

#[functional_node]
fn echo(#[input] input: u32) -> u32 {
    input
}
