## Key Conventions and Patterns

- **Avoid abbreviations.** Use complete words to ensure clarity. For example:
  - Use `output` instead of `out`
  - Use `input` instead of `in`

- **Use descriptive variable names.** Never use single-letter identifiers. Names should convey purpose clearly.
  - **Avoid:** `map_err(|e| e...)`
  - **Prefer:** `map_err(|error| error...)` or other descriptive names.
  - **Avoid:** `args, expr, err, ref, res, str, val, idx`
  - **Prefer:** `arguments, expression, error, reference, response, string, value, index`

- **Improve existing code.** When encountering misspelled or single-letter variables in source code, refactor them to use clear, descriptive names.

- Always run `cargo fmt` after making changes to maintain consistent code formatting.

## DSL Documentation

For detailed documentation on the DSL syntax and usage, refer to the [DSL Test](crates/conduit/src/test) folder. 
This file contains comprehensive examples and explanations of the DSL's features and conventions.

## Code Style

- Avoid the use of else statements when possible. Instead, use early returns to simplify code and reduce nesting.
- Use new lines to separate logical sections of code for better readability. Example:

  Bad: 
  ```
  let value_a = 1;
  let value_b = 2;
  some_function(value_a, value_b);
  ```
  
  Good:
  ```
  let value_a = 1;
  let value_b = 2;
  
  some_function(value_a, value_b);
  ```
  > Note the blank line between the variable declarations and the function call, which enhances readability.
  
  Another example of good formatting:
  ```
  let mut pairs = pair.into_inner();
  
  let (identifier_pair, related_property) = (
      pairs.next().unwrap_or_else(|| unreachable!()),
      pairs.next().unwrap_or_else(|| unreachable!()),
  );
  
  assert_eq!(identifier_pair.as_rule(), Rule::identifier);
  assert_eq!(related_property.as_rule(), Rule::property);
  
  let identifier_str = identifier_pair.as_str();
  let identifier = self
      .aliases
      .get(identifier_str)
      .cloned()
      .unwrap_or_else(|| identifier_str.to_string());
  
  Ok(Value::Relation {
      direction,
      identifier,
      property: related_property.as_str().to_string(),
  })
  ```
  > Note the symmetry / logical grouping of code blocks, which enhances readability and maintainability.
  
- Always add new lines between blocks, example:
  Bad:
  ```
  if true {}
  if true {}
  return Ok(());
  ```
  Good:
  ```
  if true {}

  if true {}
  
  return Ok(());
  ```