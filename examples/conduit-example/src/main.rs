use conduit::node::NodeError;
use conduit::traits::Emitter;
use conduit::{graphviz, input, try_graphviz, try_pipeline};
use conduit_derive::{NodeEvent, node};
use std::io::Write;

mod nodes;

#[node]
async fn read_file(#[input] path: String) -> Result<Vec<u8>, NodeError> {
    tokio::fs::read(path).await.map_err(NodeError::from)
}

#[node]
async fn write_file(#[input] content: Vec<u8>, destination: String) -> Result<(), NodeError> {
    tokio::fs::write(destination, content).await.map_err(NodeError::from)
}

#[derive(NodeEvent)]
enum PromptEvents {
    Answer { value: u32 },
}

#[node]
async fn prompt(#[input] question: String, emitter: Emitter<PromptEvents>) -> Result<(), NodeError> {
    print!("{} ", question);
    std::io::stdout().flush().map_err(NodeError::from)?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).map_err(NodeError::from)?;

    let value = input
        .trim()
        .parse::<u32>()
        .map_err(|error| NodeError::Custom(format!("invalid numeric answer: {}", error)))?;

    emitter.emit(PromptEvents::Answer { value }).await;
    Ok::<(), NodeError>(())
}

fn main() {
    let pipeline = include_str!("./plan.conduit");
    let graph = graphviz!(pipeline);
    let input = input! {
        source: "./examples/conduit-example/cover.png",
        destination: "./examples/conduit-example/cover.resized.png",
    };

    println!("{}", graph);

    let result: Result<Vec<u8>, _> = try_pipeline!(input, pipeline);

    match result {
        Ok(data) => println!("{} bytes", data.len()),
        Err(e) => println!("Pipeline error: {}", e),
    }
}
