use conduit::{input, try_pipeline};

#[allow(unused_imports)]
use example::nodes::*;

fn main() {
    let input = input! {
        source: "./examples/conduit-example/cover.png",
        prefix: "./examples/conduit-example/cover.responsive",
    };

    let output: Result<(u8, Vec<String>), _> = try_pipeline! { input, r#"
        -> source, prefix

        store _ {
            processed <- 0
            paths <- []
        }

        source_image read_file <- source

        for size in [ 128 256 512 ] {
            path _ {
               <- "{ prefix }.{ size }.png"
            }

            resizer {
                <- source_image
                width <- size
                height <- size
                -> write_file {
                    destination <- path
                }
            }

            store::processed <- (store::processed + 1)
            store::paths <<- path
        }

        <- store::processed
        <- store::paths
    "# };

    match output {
        Ok((processed, paths)) => {
            println!("Generated {} responsive images. Paths: {:#?}", processed, paths);
        }
        Err(error) => {
            println!("Failed to generate responsive images: {}", error);
        }
    }
}
