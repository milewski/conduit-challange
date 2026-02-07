use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

pub type Settings = HashMap<String, Box<dyn Any + Send + Sync + 'static>>;

pub type SharedValue = Arc<dyn Any + Send + Sync + 'static>;

#[derive(Clone, Debug)]
pub enum InputsType {
    Input(Option<SharedValue>),
    Output(Option<SharedValue>),
}

#[derive(Debug)]
pub struct Input<T: 'static> {
    inner: SharedValue,
    value: PhantomData<T>,
}

impl<T> Input<T> {
    pub fn new(inner: SharedValue) -> Self {
        Self {
            inner,
            value: PhantomData::default(),
        }
    }

    pub fn read(&self) -> &T {
        self.inner.downcast_ref::<T>().unwrap()
    }
}

#[derive(Debug, Default)]
pub struct Output<T: Send + Sync + 'static> {
    inner: RefCell<T>,
    value: PhantomData<T>,
}

impl<T: Sync + Send> Output<T> {
    pub fn write(&self, value: T) {
        self.inner.replace(value);
    }

    pub fn into_shared_value(self) -> SharedValue {
        Arc::new(self.inner.into_inner())
    }
}
