use crate::dsl::parser::{
    CallbackAssignment, CallbackAssignmentOperation, EVENT_PAYLOAD_IDENTIFIER, EventCallback, NodeParser,
    PIPELINE_RESULT_ID, Value,
};

fn assert_parser_snapshot_named(snapshot_name: &str, input: &str) {
    insta::with_settings!({
        snapshot_path => "../dsl/snapshots",
        prepend_module_to_snapshot => false,
        filters => vec![(r"[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}", "[UUID]")]
    }, {
        let nodes = NodeParser::parse(input).map(|result| result.nodes);
        insta::assert_debug_snapshot!(snapshot_name, nodes);
    });
}

#[test]
fn test_basic_nodes() {
    assert_parser_snapshot_named("conduit__dsl__parser__tests__basic_nodes", "named module {}");
    assert_parser_snapshot_named("conduit__dsl__parser__tests__basic_nodes-2", "anonymous_module {}");
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__basic_nodes-3",
        "anonymous_module { a <- true }",
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__basic_nodes-4",
        "anonymous_module { a <- false }",
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__basic_nodes-5",
        "anonymous_module { a <- 123 }",
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__basic_nodes-6",
        "anonymous_module { a <- -123 }",
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__basic_nodes-7",
        "anonymous_module { a <- 123.123 }",
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__basic_nodes-8",
        "anonymous_module { a <- -123.123 }",
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__basic_nodes-9",
        r#"anonymous_module { a <- "string" }"#,
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__basic_nodes-10",
        r#"anonymous_module { a <- "string with space" }"#,
    );
}

#[test]
fn test_inline_nodes() {
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__inline_nodes",
        "name_a module_a { property_a <- anonymous_module_b::custom {} }",
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__inline_nodes-2",
        "name_a module_a { property_a <- anonymous_module_b {} }",
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__inline_nodes-3",
        "name_a module_a { property_a <- name_b module_b::custom {} }",
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__inline_nodes-4",
        "name_a module_a { property_a <- name_b module_b {} }",
    );
}

#[test]
fn test_relations_and_chaining() {
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__relations_and_chaining",
        r#"
            name_a module_a { property_a <- 123 }
            name_b module_b { property_b <- name_a::property_a }
            name_c module_c { property_c <- name_b::property_b }
            name_d module_d { property_d -> name_a }
            name_e module_e { property_e -> name_a::custom_e }
            name_f module_f {
                property_f_1 -> name_g module_g { property_g -> name_a::custom_g }
                property_f_2 -> name_h module_h { property_h -> name_a::custom_h }
            }
        "#,
    );
}

#[test]
fn test_expression_basic() {
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__expression_basic",
        "name module { property <- (123 + 123) }",
    );
}

#[test]
fn test_expression_arithmetic() {
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__expression_arithmetic",
        "name module { property <- (10 - 3) }",
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__expression_arithmetic-2",
        "name module { property <- (6 * 7) }",
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__expression_arithmetic-3",
        "name module { property <- (100 / 4) }",
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__expression_arithmetic-4",
        "name module { property <- (2 ^ 10) }",
    );
}

#[test]
fn test_expression_precedence() {
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__expression_precedence",
        "name module { property <- (2 + 3 * 4) }",
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__expression_precedence-2",
        "name module { property <- ((2 + 3) * 4) }",
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__expression_precedence-3",
        "name module { property <- (10 - 2 * 3) }",
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__expression_precedence-4",
        "name module { property <- (10 / 2 + 3) }",
    );
}

#[test]
fn test_expression_nested() {
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__expression_nested",
        "name module { property <- ((1 + 2) * (3 + 4)) }",
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__expression_nested-2",
        "name module { property <- (((10))) }",
    );
}

#[test]
fn test_duplicated_node_error() {
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__duplicated_node_error",
        r#"
            name module_a {}
            name module_b {}
        "#,
    );
}

#[test]
fn test_expression_with_node_ref() {
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__expression_with_node_ref",
        r#"
            config constants { multiplier <- 4 }
            name module { width <- (32 * config::multiplier) }
        "#,
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__expression_with_node_ref-2",
        r#"
            a module_a { x <- 10 }
            b module_b { y <- (a::x + 5) }
        "#,
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__expression_with_node_ref-3",
        r#"
            a module_a { x <- 10 }
            b module_b { y <- 20 }
            c module_c { z <- (a::x * b::y + 1) }
        "#,
    );
}

#[test]
fn test_shorthand_basic() {
    assert_parser_snapshot_named("conduit__dsl__parser__tests__shorthand_basic", r#"module_a <- "hello""#);
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__shorthand_basic-2",
        r#"name module_a <- "hello""#,
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__shorthand_basic-3",
        r#"name module_a <- 42"#,
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__shorthand_basic-4",
        r#"name module_a <- true"#,
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__shorthand_basic-5",
        r#"
            name_a module_a <- 123
            name_b module_b <- name_a
        "#,
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__shorthand_basic-6",
        r#"
            name_a module_a <- 123
            name_b module_b -> name_a
        "#,
    );
}

#[test]
fn test_shorthand_chaining() {
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__shorthand_chaining",
        r#"name_a module_a <- name_b module_b <- "hello""#,
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__shorthand_chaining-2",
        r#"name_a module_a <- module_b <- "hello""#,
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__shorthand_chaining-3",
        r#"name_a module_a <- name_b module_b <- name_c module_c <- 42"#,
    );
}

#[test]
fn test_shorthand_mixed_with_body() {
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__shorthand_mixed_with_body",
        r#"
            name_a module_a {
                input <- name_b module_b <- "hello"
                extra <- 123
            }
        "#,
    );
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__shorthand_mixed_with_body-2",
        r#"
            name_a module_a {
                input <- name_b module_b <- "hello"
                output -> name_c module_c <- 42
            }
        "#,
    );
}

#[test]
fn test_string_interpolation_parsing_simple() {
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__string_interpolation_parsing_simple",
        r#"node m { s <- "Hello { config::name }!" }"#,
    );
}

#[test]
fn test_implicit_property_assignment() {
    assert_parser_snapshot_named(
        "conduit__dsl__parser__tests__implicit_property_assignment",
        r#"
            name module {
                <- "implicit input"
                -> "implicit output"
                other <- 123
            }
        "#,
    );
}

#[test]
fn test_pipeline_inputs() {
    let input = r#"
        -> width <- 100
        -> height <- 200
        -> source, destination
        node module {
            w <- width
            h <- height
        }
    "#;
    let workflow = NodeParser::parse(input).expect("Failed to parse");
    assert!(workflow.inputs.contains_key("width"));
    assert!(workflow.inputs.contains_key("height"));
    assert!(workflow.inputs.contains_key("source"));
    assert!(workflow.inputs.contains_key("destination"));

    let width_value = workflow.inputs.get("width").unwrap();
    if let Some(Value::Numeric { value, .. }) = width_value {
        assert_eq!(value, "100");
    } else {
        panic!("Expected Numeric value for width");
    }
}

#[test]
fn test_pipeline_result_supports_comma_separated_values() {
    let workflow = NodeParser::parse(
        r#"
        output _ {
            name <- "Rafael"
            age <- 20
        }
        <- output::name, output::age
    "#,
    )
    .expect("comma separated pipeline result should parse");

    let pipeline_result_node = workflow
        .nodes
        .get(PIPELINE_RESULT_ID)
        .expect("pipeline result node should exist");
    let Some(Value::Tuple { values, .. }) = pipeline_result_node.inputs.get("input") else {
        panic!("pipeline result should be stored as tuple for multiple values");
    };

    assert_eq!(values.len(), 2);
}

#[test]
fn test_event_handler_payload_aliases() {
    let implicit_alias_workflow = NodeParser::parse(
        r#"
        store _ { counter <- 0 }
        source task {
            count <- 1
            on done -> {
                store::counter <- done
            }
        }
        "#,
    )
    .expect("implicit payload alias should parse");

    let implicit_callbacks = implicit_alias_workflow
        .event_handlers
        .get("source")
        .and_then(|handlers| handlers.get("done"))
        .expect("done callbacks should exist");
    let EventCallback::Block(implicit_statements) = &implicit_callbacks[0] else {
        panic!("expected callback block")
    };
    let EventCallback::Assignment(implicit_assignment) = &implicit_statements[0] else {
        panic!("expected callback assignment")
    };
    let Value::Relation {
        identifier: implicit_identifier,
        property: implicit_property,
        ..
    } = &implicit_assignment.value
    else {
        panic!("expected relation for implicit payload output")
    };
    assert_eq!(implicit_identifier, EVENT_PAYLOAD_IDENTIFIER);
    assert_eq!(implicit_property, "output");

    let explicit_alias_workflow = NodeParser::parse(
        r#"
        store _ { counter <- 0 }
        source task {
            count <- 1
            on done payload {
                store::counter <- payload
            }
        }
        "#,
    )
    .expect("explicit payload alias should parse");

    let explicit_callbacks = explicit_alias_workflow
        .event_handlers
        .get("source")
        .and_then(|handlers| handlers.get("done"))
        .expect("done callbacks should exist");
    let EventCallback::Block(explicit_statements) = &explicit_callbacks[0] else {
        panic!("expected callback block")
    };
    let EventCallback::Assignment(explicit_assignment) = &explicit_statements[0] else {
        panic!("expected callback assignment")
    };
    let Value::Relation {
        identifier: explicit_identifier,
        property: explicit_property,
        ..
    } = &explicit_assignment.value
    else {
        panic!("expected relation for explicit payload value")
    };
    assert_eq!(explicit_identifier, EVENT_PAYLOAD_IDENTIFIER);
    assert_eq!(explicit_property, "output");
}

#[test]
fn test_event_handler_array_append_assignment() {
    let workflow = NodeParser::parse(
        r#"
        store _ { paths <- [] }
        source task {
            count <- 1
            on done payload {
                store::paths <<- payload
            }
        }
        "#,
    )
    .expect("append assignment should parse");

    let callbacks = workflow
        .event_handlers
        .get("source")
        .and_then(|handlers| handlers.get("done"))
        .expect("done callbacks should exist");
    let EventCallback::Block(statements) = &callbacks[0] else {
        panic!("expected callback block")
    };
    let has_append_assignment = statements.iter().any(|statement| {
        matches!(
            statement,
            EventCallback::Assignment(CallbackAssignment {
                operation: CallbackAssignmentOperation::Append,
                ..
            })
        )
    });
    assert!(has_append_assignment, "expected append assignment in callback block");
}

#[test]
fn test_sequence_group_creates_sequential_edges() {
    let workflow = NodeParser::parse(
        r#"
        (
            first task {}
            second task {}
            third task {}
        )
        "#,
    )
    .expect("sequence group should parse");

    assert_eq!(
        workflow.sequential_edges,
        vec![
            ("first".to_string(), "second".to_string()),
            ("second".to_string(), "third".to_string()),
        ]
    );
}
