use conduit::Engine;

mod nodes;

fn main() {
    let pipeline = r#"
        constants _ {
            width <- 512
            height <- 1024
        }

        metadata metadata <- source read_file <- "./examples/conduit-example/cover.png"

        <- resizer {
            source <- source
            width <- (metadata::width / 2)
            height <- (metadata::height / 2)
            output -> write_file {
                destination <- "./examples/conduit-example/cover.smaller.png"
            }
        }
    "#;

    let mut engine = Engine::new();

    match engine.run_pipeline_blocking::<Vec<u8>>(pipeline) {
        Ok(data) => println!("Pipeline result: {} bytes", data.len()),
        Err(e) => println!("Pipeline error: {}", e),
    }
}
