use conduit::Engine;

mod nodes;

fn main() {
    let pipeline = r#"
        resizer {
            source <- read_file {
                input <- "./examples/conduit-example/cover.png"
            }
            width <- 512
            height <- 215
            output -> write_file {
                destination <- "./examples/conduit-example/cover.smaller.png"
            }
        }
    "#;

    let mut engine = Engine::new();

    engine.run_pipeline(pipeline);
}
