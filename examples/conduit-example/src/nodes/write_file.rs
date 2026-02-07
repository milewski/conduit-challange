use async_trait::async_trait;
use conduit::node::NodeError;
use conduit::traits::ExecutableNode;
use conduit_derive::{Node, NodeInput};
use std::fs;

#[derive(Node, Default)]
pub struct WriteFile;

#[derive(NodeInput)]
pub struct WriteFileInput {
    pub destination: String,
    pub input: Vec<u8>,
}

#[async_trait]
impl ExecutableNode for WriteFile {
    type Input = WriteFileInput;
    type Output = ();

    async fn run(&self, input: Self::Input) -> Result<Self::Output, NodeError> {
        fs::write(&input.destination, &input.input).map_err(NodeError::from)
    }
}
