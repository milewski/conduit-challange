use crate::node::{Input, Output, SharedValue};
use crate::registry::Payload;
use crate::traits::{Descriptor, ExecutableNode, FieldType};
use conduit_derive::Node;
use std::fs;

#[derive(Node, Debug)]
pub struct Loader {
    pub input: Input<String>,
    pub output: Output<Vec<u8>>,
}

impl ExecutableNode for Loader {
    fn run(&self) {
        if let Ok(data) = fs::read(self.input.read()) {
            self.output.write(data)
        }
    }
}
