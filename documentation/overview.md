# Overview

Conduit is a node-based DSL and runtime designed to express data processing pipelines as composable graphs. Nodes are small units of computation that accept inputs and produce outputs; edges wire outputs to inputs to form a directed acyclic graph (DAG).

A simple pipeline

```dsl
save_file {
  <- resize <- read_file <- "example.png"
  -> save_file <- "resized.png"
}
```

This pipeline reads `example.png`, resizes it, and writes the resized image to `resized.png`. The `read_file`, `resize` (often called `resizer` in code), and `save_file` nodes are connected so the output of one feeds into the next.

Nodes and composition

Nodes can have named and default inputs. You can pipe values into node inputs, reference store values with `store::`, and compose pipelines using callback nodes or inline blocks. Conduit aims to keep the DSL minimal while enabling powerful composition patterns.

Runtime

Conduit nodes are implemented in Rust and compiled into the runtime. Node implementations are asynchronous functions that return Result types and use a common NodeError for error handling. See the Rust Nodes page for concrete examples showing `read_file`, `resizer`, and `save_file` implementations.

Where to look next

- Node Syntax: Learn how to declare nodes and inputs in the DSL.
- Rust Nodes: Learn how to implement nodes in Rust and register them with the runtime.
- Examples: Real pipelines you can run and adapt.
