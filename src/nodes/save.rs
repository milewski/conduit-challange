use crate::node::Input;
use crate::traits::ExecutableNode;
use conduit_derive::Node;
use std::fs;

#[derive(Node, Debug)]
pub struct Save {
    pub destination: Input<String>,
    pub input: Input<Vec<u8>>,
}

impl ExecutableNode for Save {
    fn run(&self) {
        fs::write(self.destination.read(), self.input.read().as_slice()).unwrap();
    }
}
