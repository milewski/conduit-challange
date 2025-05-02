use std::any::Any;
use crate::node::{Input, InputsType, Output, SharedValue};
use crate::traits::{Descriptor, FieldType, Node};
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use crate::registry::Payload;

#[derive(Debug)]
pub struct Save {
    pub destination: Input<String>,
    pub input: Input<Vec<u8>>,
}

impl Descriptor for Save {
    fn name(&self) -> &'static str {
        "save"
    }

    fn fields(&self) -> Vec<FieldType> {
        vec![
            FieldType::Input("destination"),
            FieldType::Input("input"),
        ]
    }

    fn take_outputs(self: Box<Self>) -> Vec<(&'static str, SharedValue)> {
        vec![]
    }
}

impl From<Payload> for Save {
    fn from(value: Payload) -> Self {
        Save {
            destination: Input::new(value.get("destination").unwrap().to_owned()),
            input: Input::new(value.get("input").unwrap().to_owned()),
        }
    }
}

impl Node for Save {
    fn run(&self) {
        fs::write(self.destination.read(), self.input.read().as_slice()).unwrap();
    }
}
