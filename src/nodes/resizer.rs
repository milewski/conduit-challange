use crate::node::{Input, Output, SharedValue};
use crate::registry::Payload;
use crate::traits::{Descriptor, ExecutableNode, FieldType};
use conduit_derive::Node;
use image::imageops;
use std::io::Cursor;

#[derive(Node, Debug)]
pub struct Resizer {
    pub source: Input<Vec<u8>>,
    pub width: Input<u32>,
    pub height: Input<u32>,
    pub output: Output<Vec<u8>>,
}

impl ExecutableNode for Resizer {
    fn run(&self) {
        println!("resize");
        let image = image::load_from_memory(self.source.read().as_slice()).unwrap();
        let image = image.resize(*self.width.read(), *self.height.read(), imageops::FilterType::Lanczos3);
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);

        image.write_to(&mut cursor, image::ImageFormat::Png).unwrap();

        self.output.write(buffer)
    }
}
