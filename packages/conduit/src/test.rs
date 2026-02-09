#[cfg(test)]
mod tests {
    use crate::node::NodeError;
    use crate::{functional_node, input, pipeline, pipeline_result};

    #[functional_node]
    fn multiplier(#[input] a: u32, b: u32) -> u32 {
        a * b
    }

    #[functional_node]
    fn subtract(a: u32, b: u32) -> u32 {
        a - b
    }

    #[functional_node]
    fn adder(#[input] a: u32, b: u32) -> u32 {
        a + b
    }

    #[functional_node]
    async fn sleep(#[input] duration: u64) {
        tokio::time::sleep(tokio::time::Duration::from_millis(duration)).await;
    }

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
    fn test_custom_module_can_be_processed() {
        #[functional_node]
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
        #[functional_node]
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
        #[functional_node]
        async fn number() -> u32 {
            5
        }

        let ((a, b), (c, d), (e, f), (g, h)): ((u32, u32), (u32, u32), (u32, u32), (u32, u32)) = pipeline! {r#"
            # simpler version
            number -> [
                a multiplier { b <- 2 }
                b multiplier { b <- 4 }
            ]

            # simpler version with explicity input attribute
            number -> [
                c multiplier::a { b <- 2 }
                d multiplier::a { b <- 4 }
            ]

            # the same but with an explicit output
            number::output -> [
                e multiplier::a { b <- 2 }
                f multiplier::a { b <- 4 }
            ]

            # verbose version
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
        #[functional_node]
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
        #[functional_node]
        fn producer() -> u32 {
            10
        }

        #[functional_node]
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
    fn test_cannot_add_number_with_string_using_module_references() {
        let result: Result<u32, _> = pipeline_result! {r#"
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
        let output: Result<u32, _> = pipeline_result! {r#"
            <- (1 + "1")
        "#};

        assert!(matches!(output, Err(NodeError::NotANumericType)));
    }

    #[test]
    fn test_cannot_add_string_with_string() {
        let output: Result<u32, _> = pipeline_result! {r#"
            <- ("a" + "b")
        "#};

        assert!(matches!(output, Err(NodeError::NotANumericType)));
    }

    #[test]
    fn test_parse_error() {
        let output: Result<(), _> = pipeline_result! {r#"
            INVALID SYNTAX
        "#};

        assert!(matches!(output, Err(NodeError::ParseError(_))));
    }

    #[test]
    fn test_error_when_referencing_properties_that_does_not_exist_on_a_module() {
        let output: Result<u32, _> = pipeline_result! {r#"
            config _ { value <- 1 }
            <- config::unknown_prop
        "#};

        assert!(matches!(output, Err(NodeError::ReferenceTypeNotSupported { .. })));
    }

    #[test]
    fn test_error_when_module_does_not_exist() {
        let output: Result<(), _> = pipeline_result! {r#"
            unknown {
                a <- 1
            }
         "#};

        assert!(matches!(output, Err(NodeError::ModuleValidationError(_))));
    }

    #[test]
    fn test_missing_input() {
        let output: Result<u32, _> = pipeline_result! {r#"
            <- adder {
                a <- 10
                # missing input `b` for the `adder` node
            }
        "#};

        assert!(matches!(output, Err(NodeError::MissingInput(_))));
    }

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

            #  1 * 0 + 1 =  1
            #  1 * 1 + 1 =  2
            #  2 * 2 + 1 =  5
            #  5 * 3 + 1 = 16
            # 16 * 4 + 1 = 65

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

            <- store::value       # as numeric value
            <- "{ store::value }" # as string
        "#};

        // 14 * 1 = 14
        assert_eq!(output, (14u8, "14".to_string()));
    }
}
