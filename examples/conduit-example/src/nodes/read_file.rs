use async_trait::async_trait;
use conduit::node::NodeError;
use conduit_derive::{Node, node};
use std::fs;

// extern crate self as conduit;

// #[derive(Node, Default)]
// pub struct ReadFile;
//
// #[async_trait]
// impl ExecutableNode for ReadFile {
//     type Input = String;
//     type Output = Vec<u8>;
//
//     async fn run(&self, input: Self::Input) -> Result<Self::Output, NodeError> {
//         fs::read(&input).map_err(NodeError::from)
//     }
// }

// #[node]
// async fn read_file(#[input] path: String) -> Result<Vec<u8>, NodeError> {
//     tokio::fs::read(path).await.map_err(NodeError::from)
// }
