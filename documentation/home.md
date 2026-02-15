# Conduit Documentation

Welcome to Conduit — a small, pragmatic DSL and runtime for defining and executing node-based data pipelines. This documentation covers core concepts, quick-start instructions, node syntax, Rust node implementations, expressions, control flow, and examples to help you build pipelines that are composable, testable, and performant.

Start here if you want a concise overview of the project, where to find examples, and how to implement your own nodes in Rust.

Key concepts

- Nodes: Units of work that accept inputs and produce outputs.
- Inputs: Named parameters or a default unnamed input that a node consumes.
- Store: A simple shared state abstraction for holding values across a pipeline.
- Pipelines: Directed acyclic graphs that connect node outputs to inputs.

Where to go next

- Quick start: A short guide to running the example workflows.
- Node syntax: The DSL used to declare nodes in pipeline files.
- Rust nodes: How to implement nodes in Rust using the Conduit runtime.
- Examples: Real-world examples including image resizing and file IO.

If you are evaluating Conduit, check out the examples/ directory to see working pipelines and node implementations.