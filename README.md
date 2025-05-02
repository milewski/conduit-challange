# Conduit - Node-Based Workflow DSL

Conduit is a domain-specific language (DSL) for creating node-based workflows in Rust. This document explains the syntax and concepts of the language, as well as how to use it from Node.js via FFI.

## Basic Syntax

A node in Conduit is defined using the following syntax:

```
node_name module_name {
    attribute_1 <- 123
    attribute_2 <- "abc" 
}
```

### Node Components

Each node consists of three main components:

1. **Node Name** (optional): An identifier used to reference the node from other nodes. If omitted, the node is considered "anonymous" and cannot be referenced elsewhere.

2. **Module Name**: Specifies the Rust module that implements the node's functionality. Modules are Rust files that define how inputs are processed and outputs are generated.

3. **Attributes**: Define the inputs and outputs of the node using arrow notation.

## Arrow Notation

The direction of arrows indicates data flow:

- `<-` (left arrow): Assigns a value to an input attribute
- `->` (right arrow): Directs output to another node

Values can be:
- Numbers: `123`
- Strings: `"abc"`
- Booleans: `true`, `false`
- References to other nodes: `node_name` or `node_name::output_port`
- Inline node definitions (nested nodes)

## Module Implementation

Modules are implemented as Rust structs. Here's an example of a file reading module:

```rust
#[derive(Node)]
pub struct ReadFile {
    pub input: Input<String>,
    pub output: Output<Vec<u8>>,
}

impl ExecutableNode for ReadFile {
    fn run(&self) {
        if let Ok(data) = fs::read(self.input.read()) {
            self.output.write(data)
        }
    }
}
```

## Anonymous Nodes

Nodes without names are called anonymous nodes and can be defined in two ways:

### At the Root Level

```
module_a {
    attribute_1 <- 123
}

module_b {
    attribute_1 <- 123
}
```

### Inline Within Another Node

```
module_a {
    attribute_1 <- module_b {
        attribute_1 <- 123
    }
}
```

When defining inline nodes, by default the `output` field is used. For other output fields:

```
module_a {
    attribute_1 <- module_b::another_port {
        attribute_1 <- 123
    }
}
```

## Node Sharing and Chaining

### Sharing Node Outputs

You can share a node's output among multiple nodes:

```
file file_reader { input <- "./my-file.txt" }

module_a {
    attribute_1 <- file
}

module_b {
    attribute_1 <- file::output 
}
```

Note: The `::output` suffix is optional when the output field is named "output".

This approach enables efficient resource sharing and potential parallel execution based on the dependency graph.

### Forward Chaining

You can also chain nodes using forward notation:

```
file file_reader { 
    input <- "./my-file.txt"
    output -> module_b {
        output -> write_file {
            destination <- "output.text"
        } 
    }
}
```

In this syntax:
- `output -> module_b` is equivalent to `output -> module_b::input`
- Arrows point in the direction of data flow

## Using Conduit from Node.js

Conduit can be used from Node.js applications via FFI (Foreign Function Interface). This allows you to define and execute workflows from JavaScript.

### Prerequisites

- Node.js (v14 or later recommended)
- npm or yarn
- Rust (stable channel)

### Building the Library

1. Build the Rust library:
   ```bash
   cargo build --release
   ```

2. Install Node.js dependencies:
   ```bash
   npm install
   ```

### Node.js API

The Node.js binding provides a simple API to interact with Conduit:

```javascript
const { ConduitEngine } = require('./node-binding');

// Create a new engine instance
const engine = new ConduitEngine();

try {
  // Define your workflow
  const pipeline = `
    resizer {
      source <- loader { input <- "./image.jpg" }
      width <- 128
      height <- 128
      output -> save { destination <- "resized.jpg" }
    }
  `;

  // Execute the workflow
  const success = engine.runPipeline(pipeline);
  
  if (success) {
    console.log('Pipeline executed successfully!');
  } else {
    console.error('Failed to execute pipeline');
  }
} finally {
  // Always clean up resources
  engine.destroy();
}
```

### API Reference

#### `ConduitEngine`

- `constructor()`: Creates a new Conduit engine instance
- `runPipeline(pipelineCode: string): boolean`: Executes the given pipeline code
- `destroy()`: Frees resources associated with the engine

## Getting Started

To create your own workflows:

1. Define your custom modules in Rust
2. Build the library with `cargo build --release`
3. Use the Node.js API to execute your workflows

The true power of Conduit emerges when you create reusable nodes and chain them together to build complex workflows that can be executed from both Rust and Node.js.
