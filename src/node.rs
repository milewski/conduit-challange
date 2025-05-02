use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ptr::replace;
use std::sync::Arc;
use bevy_ecs::change_detection::Mut;
use bevy_ecs::event::Events;
use bevy_ecs::prelude::{Component, Event, EventWriter, Ref};

pub type Settings = HashMap<String, Box<dyn Any + Send + Sync + 'static>>;

pub type SharedValue = Arc<dyn Any + Send + Sync + 'static>;

// #[derive(Event)]
// pub struct Payload(pub SharedValue);

#[derive(Component, Clone, Debug)]
pub enum InputsType {
    Input(Option<SharedValue>),
    Output(Option<SharedValue>),
}

// pub struct Channel<'a> {
//     reader: EventWriter<'a, Payload>,
//     writer: EventWriter<'a, Payload>,
// }
//
// pub struct TestInput<T> {
//     channel: Channel<'static>,
//     value: T,
// }
//
// pub struct TestOutput<T> {
//     channel: Channel<'static>,
//     value: T,
// }


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
pub struct Output<T: Send + Sync + 'static > {
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

// impl<T: Sync + Send + 'static> Input<T> {
//     pub fn new(inner: T) -> Self {
//         Self { value: Arc::new(inner), inner: PhantomData::default() }
//     }
//
//     pub fn get(&self) -> &T {
//         self.value.downcast_ref::<T>().unwrap()
//     }
// }
//
// #[derive(Debug, Default)]
// pub struct Output<T> {
//     inner: RefCell<Option<T>>,
//     // _marker: PhantomData<T>
// }
//
// impl<T> Output<T> {
//     pub fn set(&self, value: T) {
//         self.inner.replace(Some(value));
//     }
// }
