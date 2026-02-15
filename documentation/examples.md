---
title: Examples
---

# Examples

## Basic Node
```
input -> x, y
nodeA moduleA -> 42
```

## For Loop
```
for i in 0..10 {
  nodeA moduleA -> i
}
```

## Event Handler
```
on event -> { ... }
```

## Inline Node
```
name module { property <- anonymous_module::custom {} }
```

## Expression
```
name module { width <- (32 * config::multiplier) }
```

See [References](./references.md) for more test cases and snapshots.