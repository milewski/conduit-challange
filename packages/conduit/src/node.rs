use std::any::Any;
use std::sync::Arc;

pub type SharedValue = Arc<dyn Any + Send + Sync + 'static>;

/// Trait for types that can be extracted from a SharedValue.
pub trait FromSharedValue: Sized {
    fn from_shared_value(value: &SharedValue) -> Result<Self, NodeError>;
}

// Implement for unit type (no return value)
impl FromSharedValue for () {
    fn from_shared_value(_: &SharedValue) -> Result<Self, NodeError> {
        Ok(())
    }
}

// Implement for common types
impl FromSharedValue for String {
    fn from_shared_value(value: &SharedValue) -> Result<Self, NodeError> {
        value
            .downcast_ref::<String>()
            .cloned()
            .ok_or_else(|| NodeError::TypeMismatch {
                field: "result",
                expected: "String",
            })
    }
}

impl FromSharedValue for Vec<u8> {
    fn from_shared_value(value: &SharedValue) -> Result<Self, NodeError> {
        value
            .downcast_ref::<Vec<u8>>()
            .cloned()
            .ok_or_else(|| NodeError::TypeMismatch {
                field: "result",
                expected: "Vec<u8>",
            })
    }
}

impl FromSharedValue for u32 {
    fn from_shared_value(value: &SharedValue) -> Result<Self, NodeError> {
        value
            .downcast_ref::<u32>()
            .copied()
            .ok_or_else(|| NodeError::TypeMismatch {
                field: "result",
                expected: "u32",
            })
    }
}

impl FromSharedValue for bool {
    fn from_shared_value(value: &SharedValue) -> Result<Self, NodeError> {
        value
            .downcast_ref::<bool>()
            .copied()
            .ok_or_else(|| NodeError::TypeMismatch {
                field: "result",
                expected: "bool",
            })
    }
}

impl FromSharedValue for f64 {
    fn from_shared_value(value: &SharedValue) -> Result<Self, NodeError> {
        value
            .downcast_ref::<f64>()
            .copied()
            .ok_or_else(|| NodeError::TypeMismatch {
                field: "result",
                expected: "f64",
            })
    }
}

impl FromSharedValue for i32 {
    fn from_shared_value(value: &SharedValue) -> Result<Self, NodeError> {
        value
            .downcast_ref::<i32>()
            .copied()
            .ok_or_else(|| NodeError::TypeMismatch {
                field: "result",
                expected: "i32",
            })
    }
}

impl FromSharedValue for i64 {
    fn from_shared_value(value: &SharedValue) -> Result<Self, NodeError> {
        value
            .downcast_ref::<i64>()
            .copied()
            .ok_or_else(|| NodeError::TypeMismatch {
                field: "result",
                expected: "i64",
            })
    }
}

impl FromSharedValue for f32 {
    fn from_shared_value(value: &SharedValue) -> Result<Self, NodeError> {
        value
            .downcast_ref::<f32>()
            .copied()
            .ok_or_else(|| NodeError::TypeMismatch {
                field: "result",
                expected: "f32",
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
