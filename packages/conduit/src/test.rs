#[cfg(test)]
mod tests {
    use crate::{input, pipeline};

    #[test]
    fn test_input_output() {
        let input = input! { name: "example" };
        let output: String = pipeline! {input, r#"
            -> name
            <- name
        "#};

        assert_eq!(output, "example");
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
        let (a, (b, c)): (u32, (u32, u32)) = pipeline! { "<- (1, (2, 3))" };

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
    fn test_expressions() {
        let (a, b,): (u32, u32) = pipeline! {r#"
            <- (
                (1 + 2 * 3),
                (1 + 2 * 3),
            )
        "#};

        assert_eq!(a, 7);
        assert_eq!(b, 7);
    }
}
