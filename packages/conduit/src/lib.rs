use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

pub mod dsl;
pub mod ecs;
pub mod node;
pub mod registry;
pub mod traits;

pub use crate::ecs::Engine;
pub use crate::node::{FromSharedValue, SharedValue};

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
#[unsafe(no_mangle)]
pub extern "C" fn conduit_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            drop(CString::from_raw(s));
        }
    }
}
