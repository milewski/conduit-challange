# Rust Nodes

Nodes are implemented in Rust as asynchronous functions. The runtime discovers and invokes these functions based on the DSL node names. This page shows how to declare inputs, implement nodes, and highlights examples taken from the repository.

Function signature

Nodes typically follow this pattern:

```rust
async fn my_node(#[input] primary: Type, named: OtherType) -> Result<ReturnType, NodeError> {
    // implementation
}
```

The `#[input]` attribute marks the function parameter that receives the default input value when the DSL provides a bare value rather than a named input.

Example: read_file

Taken from the repository, a minimal `read_file` node looks like this:

```rust
async fn read_file(#[input] path: String) -> Result<Vec<u8>, NodeError> {
    let bytes = std::fs::read(path).map_err(|e| NodeError::io(e))?;
    Ok(bytes)
}
```

Example: resizer (resizer node)

The resizer node accepts an image buffer and dimensions and returns a resized image buffer:

```rust
async fn resizer(#[input] source: Vec<u8>, width: u32, height: u32) -> Result<Vec<u8>, NodeError> {
    let image = image::load_from_memory(&source).map_err(|e| NodeError::image(e))?;
    let resized = image.resize_to_fill(width, height, image::imageops::FilterType::Lanczos3);
    let mut out = Vec::new();
    resized.write_to(&mut std::io::Cursor::new(&mut out), image::ImageOutputFormat::Png)
        .map_err(|e| NodeError::image(e))?;
    Ok(out)
}
```

Example: save_file

```rust
async fn save_file(#[input] path: String, data: Vec<u8>) -> Result<(), NodeError> {
    std::fs::write(path, data).map_err(|e| NodeError::io(e))?;
    Ok(())
}
```

Store and side-effects

Some nodes write to the pipeline `store` rather than returning a value; this is done via special nodes in the DSL (e.g., `store _ { name <- value }`) and by the runtime managing shared state.

Testing and registration

Nodes are registered with the runtime in the crate under `examples/conduit-example/src/nodes` or similar modules. When adding nodes, write unit tests mirroring real-world pipelines and ensure error handling uses `NodeError` with meaningful messages.