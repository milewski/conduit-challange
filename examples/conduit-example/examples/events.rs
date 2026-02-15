use conduit::pipeline;

#[allow(unused_imports)]
use example::nodes::*;

fn main() {
    let (name, age): (String, u32) = pipeline!(r#"
        prompt {
            <- "What is your name?"
            on answer name {
                prompt {
                    <- "Hello { name }!, and how old are you?"
                    on answer age {
                        output _ {
                            name <- name
                            age <- string_to_number {
                                <- age
                                error_message <- "Invalid value for `age`: expected a number, got `{age}`."
                            }
                        }
                    }
                }
            }
        }

        <- output::name, output::age
    "#);

    println!("Answer: {}, {}", name, age);
}
