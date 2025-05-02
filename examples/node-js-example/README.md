# Conduit Node.js Integration Example

This example demonstrates how to use the Conduit workflow DSL from a Node.js environment. It shows how to integrate Rust-based Conduit nodes with JavaScript/TypeScript applications using Foreign Function Interface (FFI).

## Overview

The Node.js integration allows you to:
- Define workflows using the Conduit DSL syntax in JavaScript/TypeScript
- Execute these workflows by calling into compiled Rust code
- Process data efficiently using native performance while maintaining a JavaScript interface

## Prerequisites

Before running this example, ensure you have:

- Node.js (v14 or later recommended)
- npm or yarn
- Rust (stable channel)
- The Conduit example library compiled as a dynamic library

## Setup Instructions

### 1. Build the Rust Library

First, build the Conduit example library with your custom nodes:

```bash
# From the project root
cargo build --release
```

This will generate the `libconduit_example_lib.so` file (or `.dll` on Windows, `.dylib` on macOS) in the `target/release` directory.

### 2. Install Node.js Dependencies

```bash
# From the node-js-example directory
yarn install
# or
npm install
```

### 3. Run the Example

```bash
# From the node-js-example directory
yarn test
# or
npm test
```

This will execute the example workflow defined in `src/Main.ts`, which resizes an image using the Conduit DSL.

## How It Works

### FFI Integration

The integration uses the `ffi-rs` package to create bindings between Node.js and the compiled Rust library:

1. **Library Loading**: The `Engine.ts` file loads the compiled Rust library using `open()` from ffi-rs.
2. **Function Definitions**: It defines the interface for calling Rust functions using `define()`.
3. **TypeScript Wrapper**: The `ConduitEngine` class provides a clean TypeScript interface for working with the Conduit engine.

### Workflow Definition

Workflows are defined as strings using the Conduit DSL syntax:

```typescript
const pipeline = `
    resizer {
        source <- read_file {
            input <- "../conduit-example/cover.png"
        }
        width <- 512
        height <- 215
        output -> write_file {
            destination <- "../conduit-example/cover.smaller.png"
        }
    }
`;
```

### Resource Management

The example demonstrates proper resource management:
- Creating and destroying the engine instance
- Closing the library when done
- Error handling for library loading and execution

## Project Structure

- `src/Engine.ts` - TypeScript wrapper for the Conduit engine
- `src/Main.ts` - Example usage of the engine
- `package.json` - Node.js project configuration
- `README.md` - This documentation

## Advanced Usage

### Creating Custom Nodes

To use custom nodes in your Node.js application:

1. Define your nodes in Rust (see the `conduit-example` project)
2. Compile them into a dynamic library
3. Load the library in your Node.js application
4. Reference your custom nodes in the workflow DSL

### Error Handling

The example includes basic error handling for:
- Library loading failures
- Engine creation failures
- Pipeline execution failures

For production use, you may want to add more robust error handling and logging.

## Performance Considerations

This FFI approach provides near-native performance for computationally intensive tasks while allowing you to use the familiar Node.js ecosystem for your application logic. The overhead of crossing the FFI boundary is minimal compared to the performance benefits of executing the core logic in Rust.

## Troubleshooting

If you encounter issues:

- Ensure the library path is correct in `Engine.ts`
- Verify that the library was built with the correct features
- Check that function signatures match between Rust and the FFI definitions
- Run with Node.js debugging enabled for more detailed error information

## License

This example is part of the Conduit project and is subject to the same license terms.
