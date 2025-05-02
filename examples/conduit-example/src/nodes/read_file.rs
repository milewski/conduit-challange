use conduit::node::{Input, Output};
use conduit::traits::ExecutableNode;
use conduit_derive::Node;
use std::fs;

#[derive(Node)]
pub struct ReadFile {
    pub input: Input<String>,
    pub output: Output<Vec<u8>>,
}

impl ExecutableNode for ReadFile {
    fn run(&self) {
        if let Ok(data) = fs::read(self.input.read()) {
            self.output.write(data)
        }
    }
}
