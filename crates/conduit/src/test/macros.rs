use crate::{graphviz, input, pipeline};

#[test]
fn test_pipeline_macro_simple() {
    let output: String = pipeline! {r#"
        config _ { name <- "world" }
        <- config::name
    "#};

    assert_eq!(output, "world");
}

#[test]
fn test_pipeline_macro_with_input() {
    let input = input! { name: "example" };
    let output: String = pipeline! {input, r#"
        -> name
        config _ { name <- name }
        <- config::name
    "#};

    assert_eq!(output, "example");
}

#[test]
fn test_graphviz_macro_simple() {
    let dot_graph = graphviz! {r#"
        source _ { output <- 1 }
        sink _ { input <- source }
    "#};

    assert!(dot_graph.contains("digraph {"));
    assert!(dot_graph.contains("source"));
    assert!(dot_graph.contains("sink"));
}
