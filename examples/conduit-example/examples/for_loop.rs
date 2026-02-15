use conduit::{input, try_pipeline};

#[allow(unused_imports)]
use example::nodes::*;

fn main() {
    let pipeline = include_str!("./workflows/for_loop.conduit");
    let input = input! {
        source: "./examples/conduit-example/cover.png",
        prefix: "./examples/conduit-example/cover.responsive",
    };

    let output: Result<(u8, Vec<String>), _> = try_pipeline!(input, pipeline);

    match output {
        Ok((processed, paths)) => {
            println!("Generated {} responsive images. Paths: {:#?}", processed, paths);
        }
        Err(error) => {
            println!("Failed to generate responsive images: {}", error);
        }
    }
}
