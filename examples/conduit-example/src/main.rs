use conduit::Engine;
use conduit_derive::NodeOutput;

mod nodes;

#[derive(NodeOutput)]
struct Input {
    width: u32,
    height: u32,
}

fn main() {
    let pipeline = r#"
        -> width <- metadata::width
        -> height <- 1024

        constants _ {
            width <- 512
            height <- 1024
        }

        metadata metadata <- source read_file <- "./examples/conduit-example/cover.png"

        <- resizer {
            source <- source
            width <- (width / 2)
            height <- (height / 2)
            output -> write_file {
                destination <- "./examples/conduit-example/cover.{ metadata::name }.png"
            }
        }
    "#;
    let mut engine = Engine::new();

    let input = Input {
        width: 123,
        height: 456,
    };

    match engine.run_pipeline_blocking::<Input, Vec<u8>>(pipeline, input) {
        Ok(data) => println!("Pipeline result: {} bytes image", data.len()),
        Err(e) => println!("Pipeline error: {}", e),
    }
}
