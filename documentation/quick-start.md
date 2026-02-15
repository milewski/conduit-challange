# Quick Start

This short guide gets you from repository checkout to running a Conduit example.

Prerequisites

- Rust toolchain (stable)
- Cargo

Run the example

1. Build the example crate:

```bash
cargo build --manifest-path examples/conduit-example/Cargo.toml
```

2. Run the example workflow (from the repository root):

```bash
cargo run --manifest-path examples/conduit-example/Cargo.toml --example resize_image
```

The `conduit-example` demonstrates a simple interactive image processing workflow: it reads an image from disk, asks for target width and height, resizes the image, and saves the result.

Explore the DSL

Open `examples/conduit-example/examples/resize_image.rs` to see the DSL in action. The examples show how nodes are connected, how inputs and outputs flow through the graph, and how the `store` construct is used to hold intermediate values.

Next steps

- Read the Node Syntax page to learn the DSL details.
- Read the Rust Nodes page to learn how to write and register custom nodes in Rust.