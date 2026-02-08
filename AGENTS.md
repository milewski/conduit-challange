## Key Conventions and Patterns

- **Avoid abbreviations.** Use complete words to ensure clarity. For example:
  - Use `output` instead of `out`
  - Use `input` instead of `in`

- **Use descriptive variable names.** Never use single-letter identifiers. Names should convey purpose clearly.
  - **Avoid:** `map_err(|e| e...)`
  - **Prefer:** `map_err(|error| error...)` or other descriptive names.

- **Improve existing code.** When encountering misspelled or single-letter variables in source code, refactor them to use clear, descriptive names.

## DSL Documentation

Module Structure

Modules are the fundamental building blocks of the DSL. They follow this basic structure:

```
name module {
  property <- value
}
```

The module name is optional. If omitted, the module is anonymous:

```
module {
  property <- value
}
```

Modules can be nested within properties:

```
module {
  property <- another_module {
    property <- value
  }
}
```

Modules have implicit input and output ports. These can be referenced explicitly using the :: operator.

```
module {
  # Access the output port of another module
  property_a <- another_module::output {
    property <- value
  }
  
  # Access a specific property of another module
  property_b <- another_module::property {
    property <- value
  }
  
  # Chain input assignment through a module's input port
  property_c <- another_module::input <- "./examples/conduit-example/cover.png"
}
```

Modules can declare explicit inputs and outputs using the `<-` operator at the top level:

```
-> property_a
-> property_b

module_a module {
  property_a <- property_a
  property_b <- property_b
}

<- module_a::property_a
```

Properties can contain arithmetic expressions enclosed in parentheses:

```
module_a module {
  property_a <- (1 + 1)
}

module_b module {
  property_a <- (1 + 1)
  property_b <- ((1 + 1) * 2)
  property_c <- ((1 + 1) * module_a::property_a)
}
```