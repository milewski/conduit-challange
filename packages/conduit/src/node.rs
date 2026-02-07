use std::any::{Any, type_name};
use std::sync::Arc;

pub type SharedValue = Arc<dyn Any + Send + Sync + 'static>;

/// Trait for types that can be extracted from a SharedValue.
pub trait FromSharedValue: Sized {
    fn from_shared_value(value: &SharedValue) -> Result<Self, NodeError>;
}

impl<T: Clone + Any> FromSharedValue for T {
    fn from_shared_value(value: &SharedValue) -> Result<Self, NodeError> {
        value
            .downcast_ref::<T>()
            .cloned()
            .ok_or_else(|| NodeError::TypeMismatch {
                field: "result",
                expected: type_name::<T>(),
            })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("missing input field: {0}")]
    MissingInput(&'static str),

    #[error("type mismatch for field '{field}': expected {expected}")]
    TypeMismatch {
        field: &'static str,
        expected: &'static str,
    },

    #[error("{0}")]
    Custom(String),
}

impl From<String> for NodeError {
    fn from(s: String) -> Self {
        NodeError::Custom(s)
    }
}

impl From<&str> for NodeError {
    fn from(s: &str) -> Self {
        NodeError::Custom(s.to_string())
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for NodeError {
    fn from(e: Box<dyn std::error::Error + Send + Sync>) -> Self {
        NodeError::Custom(e.to_string())
    }
}

impl From<std::io::Error> for NodeError {
    fn from(e: std::io::Error) -> Self {
        NodeError::Custom(e.to_string())
    }
}
