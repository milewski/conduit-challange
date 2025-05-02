use crate::dsl::parser::Rule;

#[derive(thiserror::Error, Debug)]
pub enum ParserError {
    #[error("data store disconnected")]
    ModuleNotDefined { identifier: String },

    #[error("data store disconnected")]
    PropertyConflict { identifier: String, property: String },

    #[error("data store disconnected")]
    DuplicatedNode { identifier: String },

    #[error("data store disconnected")]
    ParserError(#[from] pest::error::Error<Rule>),
}