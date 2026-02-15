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

    assert!(matches!(output, Err(NodeError::ParseError(message)) if message.contains("Append To Non-Array")));
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

// --- Diagnostic error message tests ---
// These tests verify that error messages contain:
// 1. A clear error title
// 2. A code snippet with line/column pointer
// 3. An AI-friendly explanation

#[test]
fn test_syntax_error_shows_line_and_column_with_code_snippet() {
    let output: Result<(), _> = try_pipeline! {r#"
        store::path <<-
    "#};

    let error_message = match output {
        Err(NodeError::ParseError(message)) => message,
        other => panic!("Expected ParseError, got: {:?}", other),
    };

    assert!(
        error_message.contains("Syntax Error"),
        "Should contain error title, got: {}",
        error_message
    );

    assert!(
        error_message.contains("-->"),
        "Should contain location pointer, got: {}",
        error_message
    );

    assert!(
        error_message.contains("line"),
        "Should contain line number reference, got: {}",
        error_message
    );

    assert!(
        error_message.contains("column"),
        "Should contain column reference, got: {}",
        error_message
    );

    assert!(
        error_message.contains("|"),
        "Should contain code snippet with pipe separator, got: {}",
        error_message
    );

    assert!(
        error_message.contains("^"),
        "Should contain caret pointing to error, got: {}",
        error_message
    );

    assert!(
        error_message.contains("Help:"),
        "Should contain AI-friendly help text, got: {}",
        error_message
    );
}

#[test]
fn test_syntax_error_contains_expected_tokens() {
    let output: Result<(), _> = try_pipeline! {r#"
        store::path <<-
    "#};

    let error_message = match output {
        Err(NodeError::ParseError(message)) => message,
        other => panic!("Expected ParseError, got: {:?}", other),
    };

    assert!(
        error_message.contains("missing value after assignment operator"),
        "Should diagnose the root cause as a missing value, got: {}",
        error_message
    );

    assert!(
        error_message.contains("line 2"),
        "Should point to the correct line where the operator is, got: {}",
        error_message
    );
}

#[test]
fn test_duplicated_node_error_names_the_conflicting_identifier() {
    let output: Result<(), _> = try_pipeline! {r#"
        my_node _ { value <- 1 }
        my_node _ { value <- 2 }
    "#};

    let error_message = match output {
        Err(NodeError::ParseError(message)) => message,
        other => panic!("Expected ParseError, got: {:?}", other),
    };

    assert!(
        error_message.contains("Duplicated Node"),
        "Should contain error title, got: {}",
        error_message
    );

    assert!(
        error_message.contains("my_node"),
        "Should name the duplicated identifier, got: {}",
        error_message
    );

    assert!(
        error_message.contains("Help:"),
        "Should contain AI-friendly help, got: {}",
        error_message
    );

    assert!(
        error_message.contains("unique name"),
        "Should suggest making names unique, got: {}",
        error_message
    );
}

#[test]
fn test_append_to_non_array_error_names_module_and_property() {
    let output: Result<(), _> = try_pipeline! {r#"
        store _ {
            path <- "cover.128.png"
        }

        store::path <<- "cover.256.png"
    "#};

    let error_message = match output {
        Err(NodeError::ParseError(message)) => message,
        other => panic!("Expected ParseError, got: {:?}", other),
    };

    assert!(
        error_message.contains("Append To Non-Array"),
        "Should contain error title, got: {}",
        error_message
    );

    assert!(
        error_message.contains("store"),
        "Should name the module, got: {}",
        error_message
    );

    assert!(
        error_message.contains("path"),
        "Should name the property, got: {}",
        error_message
    );

    assert!(
        error_message.contains("<<-"),
        "Should reference the append operator, got: {}",
        error_message
    );

    assert!(
        error_message.contains("Help:"),
        "Should contain help text, got: {}",
        error_message
    );
}

#[test]
fn test_module_not_found_error_names_the_module() {
    let output: Result<(), _> = try_pipeline! {r#"
        unknown {
            a <- 1
        }
    "#};

    let error_message = match output {
        Err(NodeError::ModuleValidationError(message)) => message,
        other => panic!("Expected ModuleValidationError, got: {:?}", other),
    };

    assert!(
        error_message.contains("unknown"),
        "Should name the unknown module, got: {}",
        error_message
    );
}

#[test]
fn test_mixed_types_in_array_error_has_help_text() {
    use crate::dsl::parser::NodeParser;

    let result = NodeParser::parse(r#"config _ { items <- [1 "hello"] }"#);

    let error_message = match result {
        Err(error) => format!("{}", error),
        Ok(_) => panic!("Expected MixedTypesInArray error"),
    };

    assert!(
        error_message.contains("Mixed Types In Array"),
        "Should contain error title, got: {}",
        error_message
    );

    assert!(
        error_message.contains("Help:"),
        "Should contain help text, got: {}",
        error_message
    );

    assert!(
        error_message.contains("same type"),
        "Should explain type consistency requirement, got: {}",
        error_message
    );
}

#[test]
fn test_syntax_error_message_shows_source_line_content() {
    let output: Result<(), _> = try_pipeline! {r#"
        store _ { path <- }
    "#};

    let error_message = match output {
        Err(NodeError::ParseError(message)) => message,
        other => panic!("Expected ParseError, got: {:?}", other),
    };

    assert!(
        error_message.contains("store"),
        "Should show the source line containing the error, got: {}",
        error_message
    );

    assert!(
        error_message.contains("path"),
        "Should show the source line containing the error, got: {}",
        error_message
    );
}

#[test]
fn test_error_message_provides_common_causes_for_syntax_errors() {
    let output: Result<(), _> = try_pipeline! {r#"
        <- {
    "#};

    let error_message = match output {
        Err(NodeError::ParseError(message)) => message,
        other => panic!("Expected ParseError, got: {:?}", other),
    };

    assert!(
        error_message.contains("Help:"),
        "Should contain help section, got: {}",
        error_message
    );
}

#[test]
fn test_reference_type_not_supported_names_identifier_and_property() {
    let output: Result<u32, _> = try_pipeline! {r#"
        config _ { value <- 1 }
        <- config::unknown_prop
    "#};

    let error_message = match output {
        Err(NodeError::ReferenceTypeNotSupported { identifier, property }) => {
            format!("identifier={}, property={}", identifier, property)
        }
        other => panic!("Expected ReferenceTypeNotSupported, got: {:?}", other),
    };

    assert!(
        error_message.contains("config"),
        "Should reference the module, got: {}",
        error_message
    );

    assert!(
        error_message.contains("unknown_prop"),
        "Should reference the property, got: {}",
        error_message
    );
}

#[test]
fn test_parser_error_display_format_is_not_debug_format() {
    let output: Result<(), _> = try_pipeline! {r#"
        INVALID SYNTAX
    "#};

    let error_message = match output {
        Err(NodeError::ParseError(message)) => message,
        other => panic!("Expected ParseError, got: {:?}", other),
    };

    assert!(
        !error_message.contains("ParserError("),
        "Error should use Display format, not Debug format, got: {}",
        error_message
    );

    assert!(
        !error_message.contains("variant:"),
        "Error should use Display format, not Debug format, got: {}",
        error_message
    );
}

#[test]
fn test_unclosed_interpolation_brace_points_to_correct_line() {
    use crate::dsl::parser::NodeParser;

    let source = r#"
store _ {
    processed <- 0
    paths <- []
}

for size in [ 128 256 512 ] {

    path _ {
       <- "{ prefix }.{ size.png"
    }

    resizer {
        width <- size
        height <- size
    }
}

<- store::processed
"#;

    let result = NodeParser::parse(source);

    let error_message = match result {
        Err(error) => format!("{}", error),
        Ok(_) => panic!("Expected syntax error for unclosed interpolation"),
    };

    assert!(
        error_message.contains("line 10"),
        "Should point to line 10 where the unclosed brace is, got: {}",
        error_message
    );

    assert!(
        error_message.contains("unclosed"),
        "Should mention 'unclosed' in the description, got: {}",
        error_message
    );

    assert!(
        !error_message.contains("line 20") && !error_message.contains("line 19"),
        "Should NOT point to EOF, got: {}",
        error_message
    );

    assert!(
        error_message.contains("Help:"),
        "Should contain AI-friendly help, got: {}",
        error_message
    );
}

#[test]
fn test_unclosed_brace_points_to_opening_brace() {
    use crate::dsl::parser::NodeParser;

    let source = r#"
config _ {
    value <- 1

resizer {
    width <- 42
}
"#;

    let result = NodeParser::parse(source);

    let error_message = match result {
        Err(error) => format!("{}", error),
        Ok(_) => panic!("Expected syntax error for unclosed brace"),
    };

    assert!(
        error_message.contains("unclosed brace"),
        "Should diagnose unclosed brace, got: {}",
        error_message
    );

    assert!(
        error_message.contains("line 2"),
        "Should point to the opening brace line, got: {}",
        error_message
    );

    assert!(
        error_message.contains("Help:"),
        "Should contain AI-friendly help, got: {}",
        error_message
    );
}

#[test]
fn test_missing_value_after_arrow_points_to_correct_line() {
    let output: Result<(), _> = try_pipeline! {r#"
        config _ {
            value <- 1
        }

        store::path <<-
    "#};

    let error_message = match output {
        Err(NodeError::ParseError(message)) => message,
        other => panic!("Expected ParseError, got: {:?}", other),
    };

    assert!(
        error_message.contains("missing value after assignment operator"),
        "Should diagnose missing value, got: {}",
        error_message
    );

    assert!(
        error_message.contains("<<-"),
        "Should show the operator in context, got: {}",
        error_message
    );
}

#[test]
fn test_unclosed_string_literal_points_to_opening_quote() {
    use crate::dsl::parser::NodeParser;

    let source = "path _ {\n   <- \"{ prefix }.{ size + 1 }.png\n}\n";
    let error_message = format!("{}", NodeParser::parse(source).unwrap_err());

    assert!(
        error_message.contains("unclosed string literal"),
        "Should diagnose unclosed string, got: {}",
        error_message
    );

    assert!(
        error_message.contains("missing closing `\"`"),
        "Should mention missing closing quote, got: {}",
        error_message
    );

    assert!(
        error_message.contains("line 2"),
        "Should point to line 2 where the string opens, got: {}",
        error_message
    );
}

#[test]
fn test_unclosed_paren_in_interpolation_points_to_opening_paren() {
    use crate::dsl::parser::NodeParser;

    let source = "path _ {\n   <- \"{ prefix }.{ (size + 1 }.png\"\n}\n";
    let error_message = format!("{}", NodeParser::parse(source).unwrap_err());

    assert!(
        error_message.contains("unclosed `(`"),
        "Should diagnose unclosed parenthesis, got: {}",
        error_message
    );

    assert!(
        error_message.contains("interpolation"),
        "Should mention it's inside interpolation, got: {}",
        error_message
    );

    assert!(
        error_message.contains("line 2"),
        "Should point to line 2 where the paren is, got: {}",
        error_message
    );
}

#[test]
fn test_for_loop_with_wrong_keyword_suggests_in() {
    use crate::dsl::parser::NodeParser;

    let source = "for size of [ 128 256 512 ] {\n    path _ {\n       <- \"{ size }.png\"\n    }\n}\n";
    let error_message = format!("{}", NodeParser::parse(source).unwrap_err());

    assert!(
        error_message.contains("did you mean `in`"),
        "Should suggest using `in` keyword, got: {}",
        error_message
    );

    assert!(
        error_message.contains("`of`"),
        "Should mention the wrong keyword `of`, got: {}",
        error_message
    );

    assert!(
        error_message.contains("for size in"),
        "Should show the corrected syntax, got: {}",
        error_message
    );
}

#[test]
fn test_for_loop_with_from_keyword_suggests_in() {
    use crate::dsl::parser::NodeParser;

    let source = "for item from [ 1 2 3 ] {\n    node _ {\n       <- \"{ item }\"\n    }\n}\n";
    let error_message = format!("{}", NodeParser::parse(source).unwrap_err());

    assert!(
        error_message.contains("did you mean `in`"),
        "Should suggest using `in` keyword for `from`, got: {}",
        error_message
    );

    assert!(
        error_message.contains("`from`"),
        "Should mention the wrong keyword `from`, got: {}",
        error_message
    );
}
