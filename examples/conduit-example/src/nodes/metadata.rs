use async_trait::async_trait;
use conduit::node::NodeError;
use conduit::traits::ExecutableNode;
use conduit_derive::{Node, NodeOutput};

#[derive(Node, Default)]
pub struct Metadata;

#[derive(NodeOutput)]
pub struct MetadataOutput {
    pub name: String,
    pub width: u32,
    pub height: u32,
}

#[async_trait]
impl ExecutableNode for Metadata {
    type Input = Vec<u8>;
    type Output = MetadataOutput;

    async fn run(&self, input: Self::Input) -> Result<Self::Output, NodeError> {
        let image = image::load_from_memory(&input).map_err(|error| NodeError::Custom(error.to_string()))?;

        Ok(MetadataOutput {
            name: "example".to_string(),
            width: image.width(),
            height: image.height(),
        })
    }
}
