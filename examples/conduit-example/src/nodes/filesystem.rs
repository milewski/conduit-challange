use conduit::node::NodeError;
use conduit_derive::node;

#[node]
async fn read_file(#[input] path: String) -> Result<Vec<u8>, NodeError> {
    tokio::fs::read(path).await.map_err(NodeError::from)
}

#[node]
async fn write_file(#[input] content: Vec<u8>, destination: String) -> Result<(), NodeError> {
    tokio::fs::write(destination, content).await.map_err(NodeError::from)
}
