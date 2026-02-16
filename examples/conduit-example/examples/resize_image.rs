use conduit::{input, try_pipeline};

#[allow(unused_imports)]
use example::nodes::*;

/// This example read a file in disk, resize it to the provided width / height and save it to a new destination
fn main() {
    let input = input! {
        width: 512,
        height: 512,
        source: "./examples/conduit-example/cover.png",
        destination: "./examples/conduit-example/cover.resized.png",
    };

    let output: Result<Vec<u8>, _> = try_pipeline! { input, r#"
        # This workflow resize an image based on the given width / height provided
        -> source, destination, width, height
        
        <- resizer {
            <- read_file <- source
            width <- width
            height <- height
            -> write_file {
                destination <- destination
            }
        }
    "# };

    match output {
        Ok(image) => println!("Resized images in bytes: {}", image.len()),
        Err(error) => println!("Failed to resize image: {}", error),
    }
}
