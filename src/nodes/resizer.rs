use std::io::Cursor;
use crate::traits::{Descriptor, FieldType, Node};
use image::imageops;
use crate::node::{Input, Output, SharedValue};
use crate::nodes::loader::Loader;
use crate::registry::Payload;

#[derive(Debug)]
pub struct Resizer {
    pub source: Input<Vec<u8>>,
    pub width: Input<u32>,
    pub height: Input<u32>,
    pub output: Output<Vec<u8>>,
}

impl From<Payload> for Resizer {
    fn from(value: Payload) -> Self {
        Resizer {
            source: Input::new(value.get("source").unwrap().to_owned()),
            width: Input::new(value.get("width").unwrap().to_owned()),
            height: Input::new(value.get("height").unwrap().to_owned()),
            output: Default::default(),
        }
    }
}

impl Descriptor for Resizer {
    fn name(&self) -> &'static str {
        "resizer"
    }

    fn fields(&self) -> Vec<FieldType> {
        vec![
            FieldType::Input("source"),
            FieldType::Input("width"),
            FieldType::Input("height"),
            FieldType::Output("output"),
        ]
    }

    fn take_outputs(self: Box<Self>) -> Vec<(&'static str, SharedValue)> {
        vec![
            ("output", self.output.into_shared_value())
        ]
    }
}

impl Node for Resizer {
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
