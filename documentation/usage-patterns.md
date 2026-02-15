---
title: Usage Patterns
---

# Usage Patterns

## Chaining
Sequence groups allow chaining of nodes:
```
(
  nodeA moduleA -> 42
  nodeB moduleB -> nodeA.output
)
```

## Loops
```
for i in 0..10 {
  nodeA moduleA -> i
}
```

## Event Handling
```
on [event1, event2] -> callback
```

## Implicit Property Assignment
Properties can be assigned implicitly within node bodies.

See [Examples](./examples.md) for more patterns.