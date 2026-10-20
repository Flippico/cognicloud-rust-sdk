//! Helpers that turn PDFs and images into the content parts the gateway accepts. Files travel as base64 data URLs.
//! Use them in an `invoke` body, for example `json!({ "messages": [{ "role": "user", "content": [text_part(..), pdf_part(..)?] }] })`.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Value};

use crate::error::{Error, Result};

/// The largest file the gateway accepts, decoded.
pub const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;

/// An image type the gateway accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageType {
    Png,
    Jpeg,
    Webp,
    Gif,
}

impl ImageType {
    fn mime(self) -> &'static str {
        match self {
            ImageType::Png => "image/png",
            ImageType::Jpeg => "image/jpeg",
            ImageType::Webp => "image/webp",
            ImageType::Gif => "image/gif",
        }
    }
}

fn data_url(mime: &str, data: &[u8]) -> Result<String> {
    if data.len() > MAX_FILE_BYTES {
        return Err(Error::FileTooLarge);
    }
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(data)))
}

/// A text part for a chat message.
pub fn text_part(text: &str) -> Value {
    json!({ "type": "text", "text": text })
}

/// A PDF part for a chat message.
pub fn pdf_part(filename: &str, data: &[u8]) -> Result<Value> {
    Ok(
        json!({ "type": "file", "file": { "filename": filename, "file_data": data_url("application/pdf", data)? } }),
    )
}

/// An image part for a chat message.
pub fn image_part(kind: ImageType, data: &[u8]) -> Result<Value> {
    Ok(json!({ "type": "image_url", "image_url": { "url": data_url(kind.mime(), data)? } }))
}

/// Text for a Responses API input message.
pub fn input_text(text: &str) -> Value {
    json!({ "type": "input_text", "text": text })
}

/// A PDF for a Responses API input message.
pub fn input_file(filename: &str, data: &[u8]) -> Result<Value> {
    Ok(
        json!({ "type": "input_file", "filename": filename, "file_data": data_url("application/pdf", data)? }),
    )
}

/// An image for a Responses API input message.
pub fn input_image(kind: ImageType, data: &[u8]) -> Result<Value> {
    Ok(json!({ "type": "input_image", "image_url": data_url(kind.mime(), data)? }))
}
