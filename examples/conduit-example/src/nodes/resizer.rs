use async_trait::async_trait;
use conduit::node::NodeError;
use conduit::traits::ExecutableNode;
use conduit_derive::{Node, NodeInput};
use image::imageops;
use std::io::Cursor;

#[derive(Node, Default)]
pub struct Resizer;

#[derive(NodeInput)]
pub struct ResizerInput {
    #[input]
    pub source: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[async_trait]
impl ExecutableNode for Resizer {
    type Input = ResizerInput;
    type Output = Vec<u8>;

    async fn run(&self, input: Self::Input) -> Result<Self::Output, NodeError> {
        let image = image::load_from_memory(&input.source).map_err(|error| NodeError::Custom(error.to_string()))?;
        let image = image.resize_to_fill(input.width, input.height, imageops::FilterType::Lanczos3);

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);

        image
            .write_to(&mut cursor, image::ImageFormat::Png)
            .map_err(|error| NodeError::Custom(error.to_string()))?;

        Ok(buffer)
    }
}
