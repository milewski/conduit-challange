---
title: Grammar
---

# Grammar

The Conduit DSL grammar is defined in `crates/conduit/schema.pest`.

## Top-level Structure
- `nodes = { SOI ~ NEWLINE* ~ ((input_def | pipeline_result | for_loop | sequence_group | anonymous_node | node) ~ NEWLINE*)* ~ EOI }`
- Supports: input/output definitions, for-loops, sequence groups, named/anonymous nodes, and pipelines.

## Key Grammar Rules
- Input definition: `-> x, y`
- Pipeline result: `<- value`
- For loop: `for i in 0..10 { ... }`
- Sequence group: `( ... )`
- Node: `identifier module { ... }`
- Anonymous node: `module { ... }`

See [References](./references.md) for the full grammar file.