### Documentation

This is an simple example:

```
node_name module_name {
    attribute_1 <- 123
    attribute_2 <- "abc" 
}
```

a note is composed of 3 components: 

`node_name`: this is an arbritrary number, it can be anything, it is used to organize and reference your node from 
another node however it is optional, a node without a name is called anonymous node and it cannot be referrence from other nodes

`module_name`: this is the modules that will execute the core logic of your workflow, modules are simple rust files that 
receive inputs and write to outputs, this is an simple example of a module that read a file and output the content of it 
for the next node chained to it to consume

```rust
#[derive(Node)]
pub struct ReadFile {
    pub input: Input<String>,
    pub output: Output<Vec<u8>>,
}

impl ExecutableNode for ReadFile {
    fn run(&self) {
        if let Ok(data) = fs::read(self.input.read()) {
            self.output.write(data)
        }
    }
}
```

`attribute_1 <- 123`: this is how attributes, the direction of the arrow matters `<-` 
means the element on the right will be filled in to the attribute on the left as an INPUT
you are also able to do the, the values on the right can be `number` `string` `bool` a reference to another node `node_name::output_port` or an inline definition of another node more on this later


# anonymos nodes

nodes without a name are anonymous and they can be defined in 2 forms,

at the root level:

```
module_a {
    attribute_1 <- 123
}

module_b {
    attribute_1 <- 123
}
```

or inlined inside other node

```
module_a {
    attribute_1 <- module_b {
        attribute_1 <- 123
    }
}
```

when defining it inline the the convention is that an OUTPUT field on the node called `output` will be used,
if your module has multiple outputs or uses a different name that can be specified using this syntax:

```
module_a {
    attribute_1 <- module_b::another_port {
        attribute_1 <- 123
    }
}
```

you can also share input/outputs among multiple nodes, you just have to give a name to your node and link it as following:

```
file file_reader { input <- "./my-file.txt" }

module_a {
    attribute_1 <- file
}

module_b {
    attribute_1 <- file::output 
}
```

(note that the ::output is optional if the output field is called output) 
This way the file will  be efficienly shared among the two nodes, depending on the dependency graph the two nodes may 
even be qualified to be executed in parallel 

you can also chain node the inverse like this:

```
file file_reader { 
    input <- "./my-file.txt"
    output -> module_b {
        output -> write_file {
            destination <-- "output.text"
        } 
    }
}
```

Note how the arrows points to the where the data needs to flow, and the same rule as of ::output applies here too
for instance `output -> module_b` is the same as `output -> module_b::input`

and this is an introduction of how the syntax for this DSL works, the power really comes when you start
creating your own nodes and chaining multiple resusable pieces together.