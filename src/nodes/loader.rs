use std::any::Any;
use crate::node::{Input, InputsType, Output, SharedValue};
use crate::traits::{Descriptor, FieldType, Node};
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use crate::registry::Payload;

#[derive(Debug)]
pub struct Loader {
    pub input: Input<String>,
    pub output: Output<Vec<u8>>,
}

impl Descriptor for Loader {
    fn name(&self) -> &'static str {
        "loader"
    }

    fn fields(&self) -> Vec<FieldType> {
        vec![
            FieldType::Input("input"),
            FieldType::Output("output"),
        ]
    }

    fn take_outputs(self: Box<Self>) -> Vec<(&'static str, SharedValue)> {
        vec![
            ("output", self.output.into_shared_value())
        ]
    }
}

impl From<Payload> for Loader {
    fn from(value: Payload) -> Self {
        Loader {
            input: Input::new(value.get("input").unwrap().to_owned()),
            output: Output::default(),
        }
    }
}

impl Node for Loader {
    fn run(&self) {
        if let Ok(data) = fs::read(self.input.read()) {
            self.output.write(data)
        }
    }
}
