## Key Conventions and Patterns

- **Avoid abbreviations.** Use complete words to ensure clarity. For example:
  - Use `output` instead of `out`
  - Use `input` instead of `in`

- **Use descriptive variable names.** Never use single-letter identifiers. Names should convey purpose clearly.
  - **Avoid:** `map_err(|e| e...)`
  - **Prefer:** `map_err(|error| error...)` or other descriptive names.
  - **Avoid:** `args, expr, err, ref, res, str`
  - **Prefer:** `arguments, expression, error, reference, response, string`

- **Improve existing code.** When encountering misspelled or single-letter variables in source code, refactor them to use clear, descriptive names.

- Always run `cargo fmt` after making changes to maintain consistent code formatting.

## DSL Documentation

For detailed documentation on the DSL syntax and usage, refer to the [DSL Test](./packages/conduit/src/test.rs) file. 
This file contains comprehensive examples and explanations of the DSL's features and conventions.