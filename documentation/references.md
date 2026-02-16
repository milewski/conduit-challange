# References

This page lists key references, source files, and APIs relevant to Conduit developers.

Crates and modules

- crates/conduit: Core runtime and DSL parsing
- examples/conduit-example/src/nodes: Example node implementations (read_file, resizer, save_file)
- examples/node-js-example: A TypeScript example showing the DSL usage in another runtime

Important files

- examples/conduit-example/examples/resize_image.rs — a complete example pipeline for resizing an image
- crates/conduit/src/dsl/parser.rs — DSL parsing logic
- crates/conduit/src/test — unit and integration tests demonstrating common patterns (store, loops, error handling)

Errors and debugging

Nodes should use the NodeError type for consistent error handling. When debugging, review test cases in `crates/conduit/src/test` for common failure modes and how the runtime surfaces parser or execution errors.

Further reading

- Check the `README.md` in the repository root for high-level project information.
- Explore the `examples` directory to see complete, executable pipelines.

If you add new nodes, update these references to point to your implementation and include tests demonstrating behavior and failure cases.