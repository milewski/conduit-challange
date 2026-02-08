#[cfg(test)]
mod tests {
    use crate::{functional_node, input, pipeline};

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
        fn multiplier(a: u32, b: u32) -> u32 {
            a * b
        }

        let output: u32 = pipeline! {r#"
            <- multiplier {
                a <- 2
                b <- 2
            }
        "#};

        assert_eq!(output, 4);
    }

    #[test]
    fn test_nested_modules() {
        #[functional_node]
        async fn multiplier(a: u32, b: u32) -> u32 {
            a * b
        }

        #[functional_node]
        async fn subtract(a: u32, b: u32) -> u32 {
            a - b
        }

        let output: u32 = pipeline! {r#"
            <- multiplier {               # 4 * 2 = 8
                a <- mul multiplier {     # 2 * 2 = 4
                    a <- 2
                    b <- 2
                }
                b <- sub subtract {       # 4 - 2 = 2
                    a <- mul
                    b <- 2
                }
            }
        "#};

        assert_eq!(output, 8);
    }

    #[test]
    fn test_async_and_result_module() {
        #[functional_node]
        async fn async_checked_divide(dividend: u32, divisor: u32) -> Result<u32, String> {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

            if divisor == 0 {
                Err("division by zero".to_string())
            } else {
                Ok(dividend / divisor)
            }
        }

        let output: u32 = pipeline!(r#"
            <- async_checked_divide {
                dividend <- 10
                divisor <- 2
            }
        "#);

        assert_eq!(output, 5);
    }
}
