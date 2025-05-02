use crate::node::SharedValue;
use std::fmt::Debug;

pub trait ExecutableNode: Debug + Descriptor {
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