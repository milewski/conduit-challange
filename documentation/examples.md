# Examples

This section highlights practical examples included in the repository and explains what they demonstrate.

Image resize example

Path: `examples/conduit-example`

This example demonstrates an interactive image processing workflow that:

1. Reads an image from disk using `read_file`.
2. Resizes the image with `resizer` (or `resize`).
3. Saves the result with `save_file`.

DSL snippet:

```dsl
source read_file <- "cover.png"
resizer {
  source <- source
  width <- 512
  height <- 256
  -> save_file <- "cover.resized.png"
}
```

Arithmetic and store example

Path: `examples/conduit-example/examples/arithmetic.rs`

This demonstrates using `store` to hold values and simple arithmetic expressions inside the pipeline.

Looping example

Path: `examples/conduit-example/examples/for_loop.rs`

Shows how to build arrays using `store::paths <<- ...` and iterate with `for` blocks to accumulate results.

Exploring the examples

Run the examples in the `examples` directory and open the Rust source files to see real node implementations and DSL usage patterns. These working examples are the best way to learn how nodes, expressions, and control flow work together.