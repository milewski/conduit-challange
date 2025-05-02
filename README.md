# Conduit - Node-Based Workflow DSL

Conduit is a domain-specific language (DSL) for creating node-based workflows in Rust. It enables you to build complex data processing pipelines with a simple, declarative syntax.

## Table of Contents
- [Quick Start](#quick-start)
- [Basic Syntax](#basic-syntax)
- [Node Components](#node-components)
- [Arrow Notation](#arrow-notation)
- [Module Implementation](#module-implementation)
- [Anonymous Nodes](#anonymous-nodes)
- [Node Sharing and Chaining](#node-sharing-and-chaining)
- [Examples](#examples)
- [Contributing](#contributing)
- [License](#license)

## Installation

Add Conduit to your Cargo.toml:

```toml
[dependencies]
conduit = { git = "https://github.com/milewski/conduit-challange.git", version = "0.1.0" }
conduit-derive = { git = "https://github.com/milewski/conduit-challange.git", version = "0.1.0" }
```

## Quick Start

```rust
use conduit::Engine;
use conduit_derive::Node;

#[derive(Node)]
pub struct HelloWorld {
    pub hello: Input<String>,
}

impl ExecutableNode for HelloWorld {
    fn run(&self) {
        println!("hello {}", self.hello.read());
    }
}

fn main() {
    let workflow = r#"
        hello_world {
            hello <- "world"
        }
    "#;

    let mut engine = Engine::new();
    engine.run_pipeline(workflow);
    
    // you should see hello world printed to the console
}
```

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

The true power of Conduit emerges when you create reusable nodes and chain them together to build complex workflows that can be executed from both Rust and your favorite language via FFI.

## Examples

Check out the [examples directory](./examples) for complete workflow examples.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the LICENSE file for details.
