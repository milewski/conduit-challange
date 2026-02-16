# Control Flow

Conduit supports common control flow constructs in its DSL to express loops, conditionals, and event-driven logic. Control flow is intentionally limited to keep the DSL declarative and easy to reason about.

Loops

A typical pattern is using the `for` construct with `store` values to iterate and accumulate results.

```dsl
store _ { paths <- [] }

for index in 0..3 {
  store::paths <<- "cover.{ index }.png"
}

<- store::paths
```