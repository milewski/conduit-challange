use async_trait::async_trait;
use conduit::node::NodeError;
use conduit::traits::ExecutableNode;
use conduit_derive::Node;
use std::fs;

#[derive(Node, Default)]
pub struct ReadFile;

#[async_trait]
impl ExecutableNode for ReadFile {
    type Input = String;
    type Output = Vec<u8>;

    async fn run(&self, input: Self::Input) -> Result<Self::Output, NodeError> {
        fs::read(&input).map_err(NodeError::from)
    }
}
