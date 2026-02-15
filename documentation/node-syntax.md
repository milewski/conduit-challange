---
title: Node Syntax
---

# Node Syntax

## Named Node
```
identifier module { ... }
```

## Anonymous Node
```
module { ... }
```

## Shorthand
```
direction value
# Example: -> 42
```

## Node Body
- `{ parameter | event_handler }*`
- Parameters: `[property, ...] direction value`
- Event handler: `on event_name(s) -> callback` or block

See [Examples](./examples.md) for real-world usage.