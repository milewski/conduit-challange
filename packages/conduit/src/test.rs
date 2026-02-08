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
    fn test_input_uses_default_value_if_not_provided_or_defined() {
        let input = input! { a: "a" };
        let (a, b): (String, String) = pipeline! {input, r#"
            -> a
            -> b <- "b"
            <- (a, b)
        "#};

        assert_eq!(a, "a");
        assert_eq!(b, "b");
    }

    #[test]
    fn test_output_is_optional() {
        let output: () = pipeline! {r#"
            -> name <- "example"
        "#};

        assert_eq!(output, ());
    }
}
