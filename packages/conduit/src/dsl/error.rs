use crate::dsl::parser::Rule;

#[derive(thiserror::Error, Debug)]
pub enum ParserError {
    #[error("data store disconnected")]
    ModuleNotDefined { identifier: String },

    #[error("data store disconnected")]
    PropertyConflict { identifier: String, property: String },

    #[error("data store disconnected")]
    DuplicatedNode { identifier: String },

    #[error("Invalid number format")]
    InvalidNumber,

    #[error("Expression is not a constant")]
    NonConstantExpression,

    #[error("Constant not found: {0}")]
    ConstantNotFound(String),

    #[error("Value is not numeric")]
    NonNumericValue,

    #[error("Division by zero")]
    DivisionByZero,

    #[error("Negative exponent")]
    NegativeExponent,

    #[error("Mixed types in array")]
    MixedTypesInArray,

    #[error("Cannot append to non-array: {identifier}::{property}")]
    AppendToNonArray { identifier: String, property: String },

    #[error("data store disconnected")]
    ParserError(#[from] pest::error::Error<Rule>),
}
