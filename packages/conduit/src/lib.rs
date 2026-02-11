use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

pub mod dsl;
pub mod ecs;
pub mod node;
pub mod registry;
mod test;
pub mod traits;

pub use crate::ecs::Engine;
pub use crate::node::{FromSharedValue, SharedValue};
pub use conduit_derive::node as functional_node;

extern crate self as conduit;

// Opaque type to represent the Engine in C
pub struct ConduitEngine(Engine);

#[unsafe(no_mangle)]
pub extern "C" fn conduit_engine_new() -> *mut ConduitEngine {
    let engine = Engine::new();
    let boxed = Box::new(ConduitEngine(engine));
    Box::into_raw(boxed)
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_engine_free(ptr: *mut ConduitEngine) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn conduit_run_pipeline(engine: *mut ConduitEngine, pipeline: *const c_char) -> bool {
    if engine.is_null() || pipeline.is_null() {
        return false;
    }

    let engine = unsafe { &mut *engine };

    let c_str = unsafe { CStr::from_ptr(pipeline) };
    let pipeline_str = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let _: Result<(), _> = engine.0.run_pipeline_blocking(pipeline_str, ());
    true
}

// Helper function to create a C string from a Rust string
fn to_c_string(s: &str) -> *mut c_char {
    match CString::new(s) {
        Ok(c_str) => c_str.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

// Helper function to free a C string
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_macro_simple() {
        let output: String = pipeline! {
            r#"
            config _ { name <- "world" }
            <- config::name
        "#
        };
        assert_eq!(output, "world");
    }

    #[test]
    fn test_pipeline_macro_with_input() {
        let input = input! { name: "example" };
        let output: String = pipeline! {
            input,
            r#"
            -> name
            config _ { name <- name }
            <- config::name
        "#
        };
        assert_eq!(output, "example");
    }
}

/// Macro to create a dynamic input object for the pipeline.
///
/// Usage:
/// ```rust
/// use conduit::input;
/// let input = input! { name: "value", count: 42 };
/// ```
#[macro_export]
macro_rules! input {
    ( $($key:ident : $value:expr),* $(,)? ) => {
        $crate::traits::DynamicInput(vec![
            $( (stringify!($key), std::sync::Arc::new($crate::traits::AsInput::as_input($value)) as $crate::SharedValue) ),*
        ])
    };
}

/// Macro to run a pipeline inline.
///
/// Usage:
/// ```rust,ignore
/// use conduit::{pipeline, input};
///
/// // No input
/// let output: String = pipeline! { r#"<- "hello""# };
///
/// // With input
/// let input = input! { name: "world" };
/// let output: String = pipeline! { input, r#"<- name"# };
/// ```
#[macro_export]
macro_rules! pipeline {
    ($pipeline:expr) => {
        $crate::pipeline!((), $pipeline)
    };
    ($input:expr, $pipeline:expr) => {{
        let mut engine = $crate::Engine::new();
        engine.run_pipeline_blocking($pipeline, $input).unwrap()
    }};
}

/// Macro to run a pipeline inline and return the Result.
///
/// Usage:
/// ```rust,ignore
/// use conduit::{pipeline_result, input};
///
/// let result: Result<String, _> = pipeline_result! { r#"<- "hello""# };
/// ```
#[macro_export]
macro_rules! pipeline_result {
    ($pipeline:expr) => {
        $crate::pipeline_result!((), $pipeline)
    };
    ($input:expr, $pipeline:expr) => {{
        let mut engine = $crate::Engine::new();
        engine.run_pipeline_blocking($pipeline, $input)
    }};
}

/// Public API macro to run a pipeline inline and return a Result (preferred public name).
///
/// Usage:
/// ```rust,ignore
/// use conduit::{try_pipeline, input};
///
/// // Let the compiler infer the output type via assignment:
/// // let result: Result<String, _> = try_pipeline! { r#"<- "hello""# };
/// ```
#[macro_export]
macro_rules! try_pipeline {
    // Untyped variants: infer from context
    ($input:expr, $pipeline:expr) => {{
        let mut engine = $crate::Engine::new();
        engine.run_pipeline_blocking($pipeline, $input)
    }};

    ($pipeline:expr) => {
        $crate::try_pipeline!((), $pipeline)
    };
}
