use crate::{input, pipeline};

#[test]
fn test_for_loop() {
    let output: u32 = pipeline! {r#"
        store _ {
            counter <- 0
        }

        for index in 0..5 {
            store::counter <- (store::counter + index)
        }

        <- store::counter
    "#};

    assert_eq!(output, 10);
}

#[test]
fn test_for_loop_is_async() {
    let output: u32 = pipeline! {r#"
        store _ {
            counter <- 0
        }

        for index in 0..5 {
            store::counter <- index
            sleep <- 10
        }

        <- store::counter
    "#};

    assert_eq!(output, 4);
}

#[test]
fn test_for_loop_range_can_receive_dynamic_inputs() {
    let output: u32 = pipeline! {r#"
        store _ {
            from <- 0
            to <- 5
            counter <- 1
        }

        for index in { store::from }..{ store::to } {
            store::counter <- (store::counter * index + 1)
        }

        <- store::counter
    "#};

    assert_eq!(output, 65);
}

#[test]
fn test_expression_works_on_loop_ranges_and_nested_loops() {
    let output: (u32, u32) = pipeline! {r#"
        store _ {
            counter_a <- 0
            counter_b <- 0
        }

        for index in 0..{ 5 + 1 } {
            store::counter_a <- index
            for inner in 0..{ 5 + 1 } {
                store::counter_b <- (inner + index)
            }
        }

        <- store::counter_a
        <- store::counter_b
    "#};

    assert_eq!(output, (5, 10));
}

#[test]
fn test_passing_array_as_input() {
    let output: String = pipeline! {r#"
        store _ {
            latest <- "default"
            prompts <- [
                "Generate a high-resolution image based on the prompt"
                "Resize the image to the specified dimensions"
            ]
        }

        for prompt in store::prompts {
            store::latest <- prompt
        }

        <- store::latest
    "#};

    assert_eq!(output, "Resize the image to the specified dimensions");
}

#[test]
fn test_for_loop_supports_inclusive_ranges() {
    let output: u32 = pipeline! {r#"
        store _ {
            counter <- 0
        }

        for index in 0..=5 {
            store::counter <- (store::counter + index)
        }

        <- store::counter
    "#};

    assert_eq!(output, 15);
}

#[test]
fn test_for_loop_can_iterate_inline_array_literal() {
    let output: u32 = pipeline! {r#"
        store _ {
            total <- 0
        }

        for size in [128 256 512] {
            store::total <- (store::total + size)
        }

        <- store::total
    "#};

    assert_eq!(output, 896);
}

#[test]
fn test_for_loop_can_append_to_array_store() {
    let output: Vec<String> = pipeline! {r#"
        store _ {
            paths <- []
        }

        for size in [128 256 512] {
            store::paths <<- "cover.{ size }.png"
        }

        <- store::paths
    "#};

    assert_eq!(
        output,
        vec![
            "cover.128.png".to_string(),
            "cover.256.png".to_string(),
            "cover.512.png".to_string()
        ]
    );
}

#[test]
fn test_arrays_can_be_given_from_inputs() {
    let inputs = input! {
        prompts: vec!["Hello", "World"]
    };

    let output: String = pipeline! {inputs, r#"
        store _ { latest <- "" }
        -> prompts
        -> prompts_with_default <- []

        for prompt in prompts {
            store::latest <- prompt
        }

        <- store::latest
    "#};

    assert_eq!(output, "World");
}
