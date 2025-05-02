use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use crate::node::{Output, SharedValue};
use crate::registry::Payload;

pub trait Node: Debug + Descriptor {
    fn run(&self);
}

pub trait Descriptor {
    fn name(&self) -> &'static str;
    fn fields(&self) -> Vec<FieldType>;
    fn take_outputs(self: Box<Self>) -> Vec<(&'static str, SharedValue)>;
}

#[derive(Clone, Debug)]
pub enum FieldType {
    Input(&'static str),
    Output(&'static str),
}