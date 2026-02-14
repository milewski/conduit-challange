use conduit::node::NodeError;
use conduit::{Engine, input, try_pipeline};
use conduit_derive::{NodeOutput, node};

mod nodes;

#[node]
async fn read_file(#[input] path: String) -> Result<Vec<u8>, NodeError> {
    tokio::fs::read(path).await.map_err(NodeError::from)
}

#[node]
async fn write_file(#[input] content: Vec<u8>, destination: String) -> Result<(), NodeError> {
    tokio::fs::write(destination, content).await.map_err(NodeError::from)
}

fn main() {
    let pipeline = r#"
        store _ {
            width <- 0
            height <- 0
        }

        prompt {
            question <- "Enter the desired width?"
            # if no value is associated to the name of the event is also the name of the param passed to the callback function, it is only availiable within this block and cannot polute / or be used else where
            on answer -> {
                store::width <- answer
            }
        }

        prompt {
            question <- "Enter the desired height?"
            # the param of the answer can also be manually provided by defining the name you want to call it in the example bellow value
            on answer value -> {
                store::height <- value
            }
        }

        <- resizer {
            <- read_file <- "./examples/conduit-example/cover.png"
            width <- store::width
            height <- store::height
            -> write_file {
                destination <- "./examples/conduit-example/cover.example.png"
            }
        }
    "#;

    let input = input! {
        width: 123,
        height: 456,
    };

    let result: Result<Vec<u8>, _> = try_pipeline!(input, pipeline);

    match result {
        Ok(data) => println!("{} bytes", data.len()),
        Err(e) => println!("Pipeline error: {}", e),
    }
}
