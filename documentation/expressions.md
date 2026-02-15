---
title: Expressions
---

# Expressions

The language supports writing inline expressions that are evaluated at parse time or runtime, depending on the context.
Expressions can include arithmetic operations, string interpolation, and references to other node outputs or inputs.

## Grammar reference

- Relevant grammar rules in crates/conduit/schema.pest:
    - expression = numeric ~ (operation ~ numeric)*
    - numeric = number | string | relation | identifier | (expression)
    - operation includes: add (+), subtract (-), multiply (*), divide (/)

## Value types

- Number: integers and floats, e.g. `42`, `3.14`, `-10`
- String: double-quoted text with optional interpolation, e.g. `"hello"` or `"Hello {name}"`
- Boolean: `true`, `false`
- Relation / Reference: `node::property` (used to reference other node outputs)
- Tuple and Array: for grouping values
- Node / Anonymous Node: nodes can be used as inline values

## Operators and precedence

Operators supported (highest to lowest precedence):

1. Multiply `*` and Divide `/` (left-associative)
2. Add `+` and Subtract `-` (left-associative)

Examples of precedence and associativity:

```conduit
config _ {
  property_a <- (2 + 3 * 4)
  property_b <- (10 - 2 - 3)
}
```

In the example above, `property_a` evaluates to 14 and `property_b` evaluates to 5 (left-associative arithmetic).

## String interpolation

Strings support `{ ... }` interpolation where the contents are parsed as an expression and evaluated at runtime.
Interpolations may reference identifiers or nested expressions:

```conduit
greeting _ {
  <- "Hello { user.name }, your score is { (base + bonus) }"
}
```

## Using expressions inside node parameters

Expressions are commonly used when assigning inputs to nodes or storing computed values:

```conduit
-> x, y

store _ {
  sum <- (x + y)
  scaled <- ((x + y) * 2)
}

<- store::sum, store::scaled
```

## Compile-time constant expressions

Certain DSL contexts require expressions that can be evaluated at parse time (for example, range bounds used by `for`
loops). Constant expression evaluation uses integer arithmetic and returns an i32. The parser will raise errors for
division by zero or negative exponents in compile-time expressions.

## Expression evaluation and result types

At runtime, expressions are evaluated as floating-point values (f64) when used in arithmetic contexts. When an
expression is used as a node input value, the runtime converts the resulting numeric to a u32 if it is an integer within
`0..=u32::MAX`, otherwise it is returned as a floating-point number (f64).

Note: The grammar allows strings to appear inside expression syntax, but string values are not valid numeric operands
for arithmetic — attempting to use a non-numeric value in a numeric context will result in a runtime error. Node
references (e.g. `node::property`) and identifiers are resolved from node outputs or literal node inputs and must yield
numeric values for arithmetic to succeed.

## Real examples

1 - Celsius conversion (from examples):

```conduit
-> celsius

store _ {
  fahrenheit <- (celsius * 9 / 5 + 32)
  kelvin <- (celsius + 273.15)
}

<- store::fahrenheit, store::kelvin
```

2 - Complex expression with node references and interpolation:

```conduit
config _ { 
  multiplier <- 4 
}

name module { 
  width <- (32 * config::multiplier) 
}
```