use std::any::Any;
use std::sync::Arc;

pub type SharedValue = Arc<dyn Any + Send + Sync + 'static>;

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
