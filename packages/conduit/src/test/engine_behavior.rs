use crate::node::NodeError;
use crate::{Engine, input, pipeline, try_pipeline};

#[test]
fn test_engine_generates_dot_graph_for_pipeline() {
    let engine = Engine::new();
    let dot_graph = engine
        .generate_dot_graph(
            r#"
            source _ { output <- 1 }
            sink _ { input <- source }
        "#,
        )
        .expect("dot graph should be generated");

    assert!(dot_graph.contains("digraph {"));
    assert!(dot_graph.contains("source"));
    assert!(dot_graph.contains("sink"));
}

#[test]
fn test_pipeline_result_scalar_values() {
    let string_output: String = pipeline! { r#"<- "hello world""# };
    let number_output: u32 = pipeline! { r#"<- 42"# };
    let boolean_output: bool = pipeline! { r#"<- true"# };

    assert_eq!(string_output, "hello world");
    assert_eq!(number_output, 42);
    assert!(boolean_output);
}

#[test]
fn test_pipeline_result_from_node_reference() {
    let output: String = pipeline! {r#"
        config _ { output <- "from config" }
        <- config
    "#};

    assert_eq!(output, "from config");
}

#[test]
fn test_pipeline_errors_for_missing_result_and_type_mismatch() {
    let missing_result: Result<String, _> = try_pipeline! { r#"config _ { value <- 42 }"# };
    let type_mismatch: Result<u32, _> = try_pipeline! { r#"<- "hello""# };

    assert!(matches!(missing_result, Err(NodeError::NoPipelineResultDefined)));
    assert!(matches!(type_mismatch, Err(NodeError::TypeMismatch { .. })));
}

#[test]
fn test_expression_resolution_with_references_and_precedence() {
    let output: (u32, u32) = pipeline! {r#"
        config _ {
            size <- 545
            x <- 10
            y <- 3
        }
        <- (config::size / 2)
        <- (config::x + config::y * 2)
    "#};

    assert_eq!(output, (272, 16));
}

#[test]
fn test_external_input_overrides_default_value() {
    let inputs = input! { external_value: 99 };
    let output: u32 = pipeline! {inputs, r#"
        -> external_value <- 10
        <- external_value
    "#};

    assert_eq!(output, 99);
}

#[test]
fn test_string_interpolation_with_expression_and_reference() {
    let output: (String, String) = pipeline! {r#"
        config _ {
            width <- 100
            name <- "world"
        }
        <- "cover.{ (config::width + 1) }.png"
        <- "hello { config::name }"
    "#};

    assert_eq!(output.0, "cover.101.png");
    assert_eq!(output.1, "hello world");
}
