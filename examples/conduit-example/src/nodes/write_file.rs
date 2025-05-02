use conduit::node::Input;
use conduit::traits::ExecutableNode;
use conduit_derive::Node;
use std::fs;

#[derive(Node)]
pub struct WriteFile {
    pub destination: Input<String>,
    pub input: Input<Vec<u8>>,
}

impl ExecutableNode for WriteFile {
    fn run(&self) {
        if let Err(error) = fs::write(self.destination.read(), self.input.read().as_slice()) {
            println!("Error writing file: {}", error)
        }
    }
}
