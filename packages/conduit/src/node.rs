use std::any::{Any, type_name};
use std::sync::Arc;

pub type SharedValue = Arc<dyn Any + Send + Sync + 'static>;

/// Trait for types that can be extracted from a SharedValue.
pub trait FromSharedValue: Sized {
    fn from_shared_value(value: &SharedValue) -> Result<Self, NodeError>;
}

macro_rules! impl_from_shared_value_primitive {
    ($($t:ty),*) => {
        $(
            impl FromSharedValue for $t {
                fn from_shared_value(value: &SharedValue) -> Result<Self, NodeError> {
                     value
                        .downcast_ref::<$t>()
                        .cloned()
                        .ok_or_else(|| NodeError::TypeMismatch {
                            field: "result".to_string(),
                            expected: type_name::<$t>().to_string(),
                        })
                }
            }
        )*
    };
}

impl_from_shared_value_primitive!(String, bool, Vec<u8>);

macro_rules! impl_from_shared_value_numeric {
    ($($t:ty),*) => {
        $(
            impl FromSharedValue for $t {
                fn from_shared_value(value: &SharedValue) -> Result<Self, NodeError> {
                    if let Some(v) = value.downcast_ref::<i128>() {
                        return <$t>::try_from(*v).map_err(|_| NodeError::TypeMismatch {
                             field: "result".to_string(),
                             expected: concat!("castable from i128 to ", stringify!($t)).to_string(),
                        });
                    }
                    if let Some(v) = value.downcast_ref::<f64>() {
                         return Ok(*v as $t);
                    }
                    // Legacy fallback
                    if let Some(v) = value.downcast_ref::<u32>() {
                        return Ok(*v as $t);
                    }
                    if let Some(v) = value.downcast_ref::<i32>() {
                        return Ok(*v as $t);
                    }

                    Err(NodeError::TypeMismatch {
                        field: "result".to_string(),
                        expected: concat!("numeric value for ", stringify!($t)).to_string(),
                    })
                }
            }
        )*
    };
}

impl_from_shared_value_numeric!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);

macro_rules! impl_from_shared_value_float {
    ($($t:ty),*) => {
        $(
            impl FromSharedValue for $t {
                fn from_shared_value(value: &SharedValue) -> Result<Self, NodeError> {
                    if let Some(v) = value.downcast_ref::<f64>() {
                        return Ok(*v as $t);
                    }
                    if let Some(v) = value.downcast_ref::<i128>() {
                        return Ok(*v as $t);
                    }
                    // Legacy fallback
                    if let Some(v) = value.downcast_ref::<u32>() {
                        return Ok(*v as $t);
                    }

                    Err(NodeError::TypeMismatch {
                        field: "result".to_string(),
                        expected: concat!("numeric value for ", stringify!($t)).to_string(),
                    })
                }
            }
        )*
    };
}

impl_from_shared_value_float!(f32, f64);

impl FromSharedValue for () {
    fn from_shared_value(_: &SharedValue) -> Result<Self, NodeError> {
        Ok(())
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum NodeError {
    #[error("missing input field: {0}")]
    MissingInput(String),

    #[error("type mismatch for field '{field}': expected {expected}")]
    TypeMismatch { field: String, expected: String },

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Module validation error: {0}")]
    ModuleValidationError(String),

    #[error("Module '{0}' not found in registry")]
    ModuleNotFound(String),

    #[error("Task execution error: {0}")]
    TaskExecutionError(String),

    #[error("Pipeline result not resolved")]
    PipelineResultNotResolved,

    #[error("No pipeline result defined (no `<- value` statement)")]
    NoPipelineResultDefined,

    #[error("Reference '{identifier}::{property}' type not supported")]
    ReferenceTypeNotSupported { identifier: String, property: String },

    #[error("Cannot resolve '{identifier}::{property}'")]
    ReferenceResolutionError { identifier: String, property: String },

    #[error("Expression reference value is not a numeric type")]
    NotANumericType,

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

macro_rules! impl_from_shared_value_tuple {
    ($($T:ident),+) => {
        impl<$($T: FromSharedValue + Any),+> FromSharedValue for ($($T,)+) {
            fn from_shared_value(value: &SharedValue) -> Result<Self, NodeError> {
                if let Some(vec) = value.downcast_ref::<Vec<SharedValue>>() {
                    let len = vec.len();
                    let expected_len = count_idents!($($T)+);
                    if len != expected_len {
                         return Err(NodeError::Custom(format!("Expected tuple of size {}, got {}", expected_len, len)));
                    }

                    let mut iter = vec.iter();
                    Ok(($(
                        $T::from_shared_value(iter.next().unwrap())?,
                    )+))
                } else {
                    Err(NodeError::TypeMismatch {
                        field: "result".to_string(),
                        expected: "Vec<SharedValue> (Tuple)".to_string(),
                    })
                }
            }
        }
    }
}

macro_rules! count_idents {
    ($i:ident) => { 1 };
    ($i:ident $($rest:ident)+) => { 1 + count_idents!($($rest)+) };
}

impl_from_shared_value_tuple!(A, B);
impl_from_shared_value_tuple!(A, B, C);
impl_from_shared_value_tuple!(A, B, C, D);
impl_from_shared_value_tuple!(A, B, C, D, E);
