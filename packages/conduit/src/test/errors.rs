use crate::node::NodeError;
use crate::try_pipeline;

#[test]
fn test_cannot_add_number_with_string_using_module_references() {
    let result: Result<u32, _> = try_pipeline! {r#"
        config _ {
            number <- 1
            string <- "1"
        }

        <- (config::number + config::string)
    "#};

    assert!(matches!(result, Err(NodeError::NotANumericType)));
}

#[test]
fn test_cannot_add_number_with_string_using() {
    let output: Result<u32, _> = try_pipeline! {r#"
        <- (1 + "1")
    "#};

    assert!(matches!(output, Err(NodeError::NotANumericType)));
}

#[test]
fn test_cannot_add_string_with_string() {
    let output: Result<u32, _> = try_pipeline! {r#"
        <- ("a" + "b")
    "#};

    assert!(matches!(output, Err(NodeError::NotANumericType)));
}

#[test]
fn test_parse_error() {
    let output: Result<(), _> = try_pipeline! {r#"
        INVALID SYNTAX
    "#};

    assert!(matches!(output, Err(NodeError::ParseError(_))));
}

#[test]
fn test_error_when_referencing_properties_that_does_not_exist_on_a_module() {
    let output: Result<u32, _> = try_pipeline! {r#"
        config _ { value <- 1 }
        <- config::unknown_prop
    "#};

    assert!(matches!(output, Err(NodeError::ReferenceTypeNotSupported { .. })));
}

#[test]
fn test_error_when_module_does_not_exist() {
    let output: Result<(), _> = try_pipeline! {r#"
        unknown {
            a <- 1
        }
     "#};

    assert!(matches!(output, Err(NodeError::ModuleValidationError(_))));
}

#[test]
fn test_missing_input() {
    let output: Result<u32, _> = try_pipeline! {r#"
        <- adder {
            a <- 10
        }
    "#};

    assert!(matches!(output, Err(NodeError::MissingInput(_))));
}

#[test]
fn test_error_when_appending_to_non_array_store_property() {
    let output: Result<(), _> = try_pipeline! {r#"
        store _ {
            path <- "cover.128.png"
        }

        store::path <<- "cover.256.png"
    "#};

    assert!(matches!(output, Err(NodeError::ParseError(message)) if message.contains("AppendToNonArray")));
}

#[test]
fn test_error_when_event_callback_appends_to_non_array_property() {
    let output: Result<u32, _> = try_pipeline! {r#"
        store _ {
            counter <- 0
        }

        task {
            count <- 1
            on done value {
                store::counter <<- value
            }
        }

        <- store::counter
    "#};

    assert!(
        matches!(output, Err(NodeError::Custom(message)) if message.contains("Cannot append to non-array 'counter'"))
    );
}
