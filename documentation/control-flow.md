# Control Flow

Conduit supports common control flow constructs in its DSL to express loops, conditionals, and event-driven logic. Control flow is intentionally limited to keep the DSL declarative and easy to reason about.

Loops

A typical pattern is using the `for` construct with `store` values to iterate and accumulate results.

```dsl
store _ { paths <- [] }
for i in { 0 }..{ 3 } {
  // generate a path using the loop index
  path <- "cover.{ i }.png"
  store::paths <<- path
}

// after the loop
<- store::paths
```

Conditionals

Basic conditional execution is supported via `if` blocks that evaluate expressions and conditionally execute nodes.

```dsl
if { store::count > 10 } {
  logger <- "count too high"
}
```

Events and callbacks

The runtime supports callback nodes and event-driven patterns when integrating with interactive workflows or external inputs. Callbacks are represented as nodes that can create and manage configuration at runtime.

Best practices

- Prefer pure node implementations for deterministic logic.
- Use `store` to maintain state across iterations and events.
- Keep loops and conditionals small and testable; if a flow becomes complex, consider moving logic into Rust nodes where you have richer tooling and testing.