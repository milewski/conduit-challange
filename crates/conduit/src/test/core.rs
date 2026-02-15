use crate::{input, node, pipeline};
use conduit::node::NodeError;
use conduit::try_pipeline;
use std::ops::{Range, RangeInclusive};

#[test]
fn test_input_output() {
    let input = input! { name: "rafael" };
    let output: String = pipeline! {input, r#"
        -> name
        <- name
    "#};

    assert_eq!(output, "rafael");
}

#[test]
fn test_tuple_mixed_types() {
    let (a, b, c): (u32, String, i8) = pipeline! {r#"
        <- (42, "hello", -50)
    "#};

    assert_eq!(a, 42);
    assert_eq!(b, "hello");
    assert_eq!(c, -50i8);
}

#[test]
fn test_tuple_all_numeric_types() {
    let signed: (i8, i32, i16, i128, isize) = pipeline! { "<- (1, 2, 3, 4, 5)" };
    let unsigned: (u8, u32, u16, u128, usize) = pipeline! { "<- (1, 2, 3, 4, 5)" };
    let float: (f32, f64) = pipeline! { "<- (1.0, 2.0)" };

    assert_eq!(signed, (1_i8, 2_i32, 3_i16, 4_i128, 5_isize));
    assert_eq!(unsigned, (1_u8, 2_u32, 3_u16, 4_u128, 5_usize));
    assert_eq!(float, (1_f32, 2_f64));
}

#[test]
fn test_tuple_nested() {
    let (a, (b, c)): (u32, (u32, u32)) = pipeline! { r#"
        <- (1, (2, 3))
    "# };

    assert_eq!(a, 1);
    assert_eq!(b, 2);
    assert_eq!(c, 3);
}

#[test]
fn test_output_is_optional() {
    let output_1: () = pipeline! {r#"-> name <- "example""#};
    let output_2: () = pipeline! {r#""#};

    assert_eq!(output_1, ());
    assert_eq!(output_2, ());
}

#[test]
fn test_tuple_with_expressions() {
    let (a, b): (u8, String) = pipeline! {r#"
        <- ((1 + (2 * 3)), "result is: { (1 + 2 * 3) }")
    "#};

    assert_eq!(a, 7);
    assert_eq!(b, "result is: 7");
}

#[test]
fn test_interpolation_string_ref() {
    let input = input! { name: "world" };
    let output: String = pipeline! {input, r#"
        -> name
        <- "hello { name }"
    "#};

    assert_eq!(output, "hello world");
}

#[test]
fn test_interpolation_expression() {
    let input = input! { width: 10 };
    let output: String = pipeline! {input, r#"
        -> width
        <- "width is { (width + 5) }"
    "#};

    assert_eq!(output, "width is 15");
}

#[test]
fn test_pipeline_multiple_input_identifiers_definition() {
    let input = input! {
        source: "cover.png",
        destination: "cover.resized.png",
        width: 512,
        height: 256,
    };

    let output: (String, String, u32, u32) = pipeline! {input, r#"
        -> source, destination, width, height
        <- (source, destination, width, height)
    "#};

    assert_eq!(
        output,
        ("cover.png".to_string(), "cover.resized.png".to_string(), 512, 256,)
    );
}

#[test]
fn test_string_literal_value() {
    let output: String = pipeline! {r#"
        <- "just a string"
    "#};

    assert_eq!(output, "just a string");
}

#[test]
fn test_output_using_module_output() {
    let (a, b, c): (String, String, String) = pipeline! {r#"
        explicity _ { a <- "a" }
        implicity _ { output <- "b" }
        <- ( explicity::a, implicity, "{ explicity::a }__{ implicity }" )
    "#};

    assert_eq!(a, "a");
    assert_eq!(b, "b");
    assert_eq!(c, "a__b");
}

#[test]
fn test_data_node_body_implicit_input_maps_to_output() {
    let (path, destination): (String, String) = pipeline! {r#"
        prefix _ { output <- "cover" }
        size _ { output <- "128" }

        path _ {
            <- "{ prefix }.{ size }.png"
        }

        write_file _ {
            destination <- path
        }

        <- path
        <- write_file::destination
    "#};

    assert_eq!(path, "cover.128.png");
    assert_eq!(destination, "cover.128.png");
}

#[test]
fn test_multiple_returns_are_supported_and_is_equivalent_as_returning_tuples() {
    let (a, b, c, d): (String, String, String, (u32, u32)) = pipeline! {r#"
        <- a _::a {      a <- "a" } # module a returning the `a` property
        <- b _    { output <- "b" } # module b returning the `::output` implicity
        <-   _    { output <- "c" } # anonymous module returning `::output` implicity
        <- (2, 3)                   # returning a tuple directly without a module
    "#};

    assert_eq!(a, "a");
    assert_eq!(b, "b");
    assert_eq!(c, "c");
    assert_eq!(d, (2, 3));
}

#[test]
fn test_multiple_returns_can_be_declared_in_one_line() {
    let (name, age): (String, u32) = pipeline! {r#"
        output _ {
            name <- "Rafael"
            age <- 24
        }

        <- output::name, output::age
    "#};

    assert_eq!(name, "Rafael");
    assert_eq!(age, 24);
}

#[test]
fn test_custom_module_can_be_processed() {
    #[node]
    fn simple_multiplier(a: u32, b: u32) -> u32 {
        a * b
    }

    let output: u32 = pipeline! {r#"
        <- simple_multiplier {
            a <- 2
            b <- 2
        }
    "#};

    assert_eq!(output, 4);
}

#[test]
fn test_node_receives_exclusive_range_value() {
    #[node]
    fn range_length(between: Range<u32>) -> u32 {
        between.end - between.start
    }

    let output: u32 = pipeline! {r#"
        random _ {
            between <- 0..10
        }

        <- range_length {
            between <- random::between
        }
    "#};

    assert_eq!(output, 10);
}

#[test]
fn test_node_receives_inclusive_range_value() {
    #[node]
    fn inclusive_range_length(between: RangeInclusive<u32>) -> u32 {
        between.end() - between.start() + 1
    }

    let output: u32 = pipeline! {r#"
        random _ {
            between <- 0..=10
        }

        <- inclusive_range_length {
            between <- random::between
        }
    "#};

    assert_eq!(output, 11);
}

#[test]
fn test_node_macro_supports_optional_input_fields() {
    #[conduit_derive::node]
    async fn optional_prefix(#[input] input: String, prefix: Option<String>) -> String {
        match prefix {
            Some(prefix) => format!("{}{}", prefix, input),
            None => input,
        }
    }

    let output_without_prefix: String = pipeline! {r#"
        <- optional_prefix {
            <- "world"
        }
    "#};

    let output_with_prefix: String = pipeline! {r#"
        <- optional_prefix {
            <- "world"
            prefix <- "hello "
        }
    "#};

    assert_eq!(output_without_prefix, "world");
    assert_eq!(output_with_prefix, "hello world");
}

#[test]
fn test_node_macro_result_ok_infers_error_type_without_turbofish() {
    #[conduit_derive::node]
    async fn parse_number(#[input] input: String) -> Result<u32, NodeError> {
        Ok(input
            .parse::<u32>()
            .map_err(|error| NodeError::Custom(error.to_string()))?)
    }

    let output: u32 = pipeline! {r#"
        <- parse_number <- "7"
    "#};

    assert_eq!(output, 7);
}

#[test]
fn test_nested_modules() {
    let output: u32 = pipeline! {r#"
        <- multiplier {               # 4 * 2 = 8
            a <- m multiplier {       # 2 * 2 = 4
                a <- 2
                b <- 2
            }
            b <- subtract {           # 4 - 2 = 2
                a <- m
                b <- 2
            }
        }
    "#};

    assert_eq!(output, 8);
}

#[test]
fn test_async_module() {
    #[node]
    async fn async_checked_divide(dividend: u32, divisor: u32) -> Result<u32, String> {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        if divisor == 0 {
            Err("division by zero".to_string())
        } else {
            Ok(dividend / divisor)
        }
    }

    let output: u32 = pipeline! {r#"
        <- async_checked_divide {
            dividend <- 10
            divisor <- 2
        }
    "#};

    assert_eq!(output, 5);
}

#[test]
fn test_multiple_nodes_on_a_single_output() {
    #[node]
    async fn number() -> u32 {
        5
    }

    let ((a, b), (c, d), (e, f), (g, h)): ((u32, u32), (u32, u32), (u32, u32), (u32, u32)) = pipeline! {r#"
        number -> [
            a multiplier { b <- 2 }
            b multiplier { b <- 4 }
        ]

        number -> [
            c multiplier::a { b <- 2 }
            d multiplier::a { b <- 4 }
        ]

        number::output -> [
            e multiplier::a { b <- 2 }
            f multiplier::a { b <- 4 }
        ]

        number {
            output -> [
                g multiplier::a { b <- 2 }
                h multiplier::a { b <- 4 }
            ]
        }

        <- ((a, b), (c, d), (e, f), (g, h))
    "#};

    assert_eq!((a, b), (10, 20));
    assert_eq!((c, d), (10, 20));
    assert_eq!((e, f), (10, 20));
    assert_eq!((g, h), (10, 20));
}

#[test]
fn test_implicit_input_output_syntax() {
    #[node]
    fn simple_math(#[input] value: u32, operand: u32) -> u32 {
        value + operand
    }

    let output: (u32, u32) = pipeline! {r#"
        <- simple_math {
            <- 10
            operand <- 5
        }

        <- simple_math {
            value <- 5
            operand <- 5
        }
    "#};

    assert_eq!(output, (15, 10));
}

#[test]
fn test_implicit_output_chaining() {
    #[node]
    fn producer() -> u32 {
        10
    }

    #[node]
    fn consumer(#[input] value: u32) -> u32 {
        value * 2
    }

    let output: u32 = pipeline! {r#"
        p producer { -> c consumer {} }
        <- c
    "#};

    assert_eq!(output, 20);
}

#[test]
fn test_error_is_thrown_if_property_does_not_exist() {
    #[node]
    fn throw_error_on_invalid_property(a: u32) -> u32 {
        a
    }

    let output: Result<u32, _> = try_pipeline! {r#"
        <- throw_error_on_invalid_property {
            b <- 1
        }
    "#};

    assert!(matches!(output, Err(NodeError::ModuleValidationError(_))));
}

#[test]
fn test_properties_can_be_mass_assigned() {
    let output: (u32, u32) = pipeline! {r#"
        <- multiplier {
            a <- 5
            b <- 5
        }

        <- multiplier {
            [a, b] <- 2
        }
    "#};

    assert_eq!(output, (25, 4));
}

#[test]
fn test_default_values_can_be_mass_assigned_to_inputs() {
    let a: (u32, u32, u32) = pipeline! { input! { x: 1, y: 2, z: 3 }, r#"
        -> x, y, z
        <- x, y, z
    "#};

    let b: (u32, u32, u32) = pipeline! { input! { x: 1, z: 3 }, r#"
        -> x, y, z <- 5
        <- x, y, z
    "#};

    assert_eq!(a, (1, 2, 3));
    assert_eq!(b, (1, 5, 3));
}

#[test]
fn test_all_numeric_types_inputs() {
    let inputs = input! {
        u8: 1u8,
        u16: 1u16,
        u32: 1u32,
        u64: 1u64,
        u128: 1u128,
        usize: 1usize,
        i8: 1i8,
        i16: 1i16,
        i32: 1i32,
        i64: 1i64,
        i128: 1i128,
        isize: 1isize,
        f32: 1.0f32,
        f64: 1.0f64
    };

    let output: (u8, String) = pipeline! {inputs, r#"
        -> u8 -> u16 -> u32 -> u64 -> u128 -> usize
        -> i8 -> i16 -> i32 -> i64 -> i128 -> isize
        -> f32 -> f64

        store _ {
            value <- (u8 + u16 + u32 + u64 + u128 + usize + i8 + i16 + i32 + i64 + i128 + isize + f32 + f64)
        }

        <- store::value
        <- "{ store::value }"
    "#};

    assert_eq!(output, (14u8, "14".to_string()));
}
