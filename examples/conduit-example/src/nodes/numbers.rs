use conduit::node::NodeError;
use conduit_derive::node;

#[node]
async fn string_to_number(#[input] input: String, error_message: Option<String>) -> Result<u32, NodeError> {
    Ok::<u32, NodeError>(
        input
            .parse::<u32>()
            .map_err(|error| NodeError::Custom(error_message.unwrap_or_else(|| error.to_string())))?,
    )
}
