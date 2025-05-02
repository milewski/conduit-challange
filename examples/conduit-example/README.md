# Conduit Example

This example demonstrates a simple image processing workflow using Conduit's node-based DSL. The workflow reads an image file, resizes it, and saves the result to a new location.

## Nodes Used

The example utilizes three custom nodes:

1. **read_file**: Reads any file from the filesystem given a path
   - Input: `input` - File path as a string
   - Output: File contents as a buffer

2. **write_file**: Writes data to a file at a specified location
   - Input: `destination` - Target file path
   - Input: Receives a buffer from another node's output
   - Output: None (side effect: file creation)

3. **resizer**: Processes an image buffer and resizes it to specified dimensions
   - Input: `source` - Image data buffer
   - Input: `width` - Target width in pixels
   - Input: `height` - Target height in pixels
   - Output: Resized image as a buffer

## Running the Example

Execute the example with:

```bash
cargo run --example basic
```

Upon successful execution, you'll find a resized image file named `cover.smaller.png` in the `examples/conduit-example` directory.

## Code Explanation

The example demonstrates Conduit's node chaining capabilities:

```rust
fn main() {
    let pipeline = r#"
        resizer {
            source <- read_file {
                input <- "./examples/conduit-example/cover.png"
            }
            width <- 512
            height <- 215
            output -> write_file {
                destination <- "./examples/conduit-example/cover.smaller.png"
            }
        }
    "#;

    let mut engine = Engine::new();
    engine.run_pipeline(pipeline);
}
```

### Workflow Breakdown

1. The `read_file` node reads the source image file
2. Its output is connected to the `source` input of the `resizer` node
3. The `resizer` node processes the image with the specified dimensions
4. The resized image is then passed to the `write_file` node
5. The `write_file` node saves the processed image to the specified destination

## Node Registration

All nodes defined in the `nodes` namespace are automatically registered with the engine. The node name in the DSL corresponds to the filename of the node implementation.
