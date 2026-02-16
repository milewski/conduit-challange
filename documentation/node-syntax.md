# Node Syntax

This page describes the DSL used to declare nodes, inputs, and how to wire nodes together.

Basic node block

A node invocation in the DSL uses the node name as a block. Inputs may be provided by piping values from the left or by named arguments.

Example — chaining a pipeline:

```dsl
read_file {
  -> resize {
    width <- 512
    height <- 512
    -> save_file <- "resized.png"
  }
}
```

Default input vs named inputs

- Default input: Providing a bare value into a node block (e.g., `"example.png"` into `read_file`) sets the node's primary or default input.
- Named inputs: Use `name <- value` inside a node block to set a named input.

Using store values

`store::name` references values in the shared pipeline store. This is useful for configuration or passing values between unrelated nodes.

Examples from the repository

DSL example (pipeline):

```dsl
source read_file <- "cover.png"
resizer {
  source <- source
  width <- 512
  height <- 256
  -> save_file <- "cover.resized.png"
}
```

Rust implementation mapping

Each node in the DSL maps to a Rust async function in the runtime. Typical signatures look like:

```rust
// read_file node
async fn read_file(#[input] path: String) -> Result<Vec<u8>, NodeError> { /* ... */ }

// resizer node
async fn resizer(#[input] source: Vec<u8>, width: u32, height: u32) -> Result<Vec<u8>, NodeError> { /* ... */ }

// save_file node
async fn save_file(#[input] path: String, data: Vec<u8>) -> Result<(), NodeError> { /* ... */ }
```

When designing nodes, prefer small, single-responsibility functions that accept typed inputs and return typed outputs; the runtime handles wiring and type conversions where possible.