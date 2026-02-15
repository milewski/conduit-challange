---
title: 'Expression'
---


Arithmetic expression can be defined like the following:

```
config _ {
    a <- 1 + 1;
    b <- 2 - 2;
    c <- 3 * 3;
    d <- 4 / 4;
}

<- "a: { config::a }, b: { config::b }, c: { config::c }, d: { config::d }"
```

Available operators are:
- `+` for addition
- `-` for subtraction
- `*` for multiplication
- `/` for division
