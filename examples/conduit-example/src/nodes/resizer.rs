use conduit::node::NodeError;
use conduit_derive::node;
use image::imageops;
use std::io::Cursor;

#[node]
async fn resizer(#[input] source: Vec<u8>, width: u32, height: u32) -> Result<Vec<u8>, NodeError> {
    let image = image::load_from_memory(&source).map_err(|error| NodeError::Custom(error.to_string()))?;
    let image = image.resize_to_fill(width, height, imageops::FilterType::Lanczos3);

    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);

    image
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|error| NodeError::Custom(error.to_string()))?;

    Ok::<Vec<u8>, NodeError>(buffer)
}
