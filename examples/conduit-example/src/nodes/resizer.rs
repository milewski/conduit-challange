use async_trait::async_trait;
use conduit::node::NodeError;
use conduit::traits::{Emitter, ExecutableNode};
use conduit_derive::{Node, NodeEvent, NodeInput};
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

#[derive(NodeEvent)]
pub enum Event {
    Resized { value: Vec<u8> },
    Started { value: Vec<u8> },
}

#[async_trait]
impl ExecutableNode for Resizer {
    type Input = ResizerInput;
    type Output = Vec<u8>;
    type Event = Event;

    async fn run(&self, input: Self::Input, emitter: Emitter<Self::Event>) -> Result<Self::Output, NodeError> {
        let image = image::load_from_memory(&input.source).map_err(|error| NodeError::Custom(error.to_string()))?;
        let image = image.resize_to_fill(input.width, input.height, imageops::FilterType::Lanczos3);

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);

        image
            .write_to(&mut cursor, image::ImageFormat::Png)
            .map_err(|error| NodeError::Custom(error.to_string()))?;

        emitter.emit(Event::Started { value: buffer.clone() }).await;
        println!("sleeeeping...");
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        emitter.emit(Event::Resized { value: buffer.clone() }).await;

        Ok(buffer)
    }
}
