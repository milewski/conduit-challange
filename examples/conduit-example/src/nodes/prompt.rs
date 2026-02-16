use conduit::node::NodeError;
use conduit::traits::Emitter;
use conduit_derive::{NodeEvent, node};
use std::io::Write;

#[derive(NodeEvent)]
enum PromptEvents {
    Answer { value: String },
}

#[node]
async fn prompt(#[input] question: String, emitter: Emitter<PromptEvents>) -> Result<(), NodeError> {
    print!("{} ", question);

    std::io::stdout().flush().map_err(NodeError::from)?;

    let mut input = String::new();

    std::io::stdin().read_line(&mut input).map_err(NodeError::from)?;

    let value = input.trim().to_string();

    emitter.emit(PromptEvents::Answer { value }).await;

    Ok(())
}
