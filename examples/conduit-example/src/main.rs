use conduit::Engine;
use conduit::node::NodeError;
use conduit_derive::{NodeOutput, node};

mod nodes;

#[derive(NodeOutput)]
struct Input {
    width: u32,
    height: u32,
}

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

    let mut engine = Engine::new();

    let input = Input {
        width: 123,
        height: 456,
    };

    println!("{}", engine.generate_dot_graph(pipeline).unwrap());

    match engine.run_pipeline_blocking::<Input, Vec<u8>>(pipeline, input) {
        Ok(data) => println!("Pipeline result: {} bytes image", data.len()),
        Err(e) => println!("Pipeline error: {}", e),
    }
}
