# Expressions

Expressions in Conduit allow you to compute values inline using familiar operators and references to store values. They are lightweight, readable, and designed for common pipeline tasks.

Basic expression examples

- Arithmetic: `(1 + 2) * 3`
- Store references: `{ store::value }` or `store::value` inside expressions
- Tuples and arrays for multiple inputs: `(a, b)`, `[1, 2, 3]`

Example — computing an output filename

```dsl
store _ {
  size <- 256
  path <<- "cover.{ size }.png"
}

// later in pipeline
save_file <- "cover.{ store::size }.png"
```

Operator precedence and types

Expressions follow standard precedence rules (multiplication before addition, etc.). Conduit performs basic type checking and will coerce numbers to strings for formatting when needed.

Using expressions in node inputs

You can use expressions to compute node inputs inline:

```dsl
resizer {
  source <- read_file <- "cover.png"
  width <- (base_width * 2)
  height <- (base_height)
}
```

Store helpers

The `store` construct is a convenient way to keep values around across the pipeline run. Use `store::name <- value` to set and `store::name` to reference. The DSL also supports append operators (e.g., `<<-`) for building arrays.

Best practices

- Keep expressions simple and readable.
- Move complex logic into nodes implemented in Rust to maintain clarity and testability.
- Use the store for shared state and configuration rather than passing many parameters through long chains.