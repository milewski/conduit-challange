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
        -> width <- 1024
        -> height <- 1024

        <- resizer {
            <- read_file <- "./examples/conduit-example/cover.png"
            width <- width
            height <- height
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
