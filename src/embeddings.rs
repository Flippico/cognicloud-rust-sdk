use serde::{Deserialize, Serialize};

use crate::endpoints::InvokeUsage;

/// Text to embed: one string or up to 256 strings. The endpoint's embedding deployment is the model.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EmbeddingsRequest {
    pub input: EmbeddingInput,
    /// The size of each vector, from 1 to 3072. Omit it for the model's default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum EmbeddingInput {
    Text(String),
    Texts(Vec<String>),
}

impl EmbeddingsRequest {
    /// One text to embed.
    pub fn text(input: impl Into<String>) -> Self {
        Self {
            input: EmbeddingInput::Text(input.into()),
            dimensions: None,
        }
    }

    /// Several texts to embed in one call.
    pub fn texts(inputs: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            input: EmbeddingInput::Texts(inputs.into_iter().map(Into::into).collect()),
            dimensions: None,
        }
    }

    pub fn dimensions(mut self, dimensions: u32) -> Self {
        self.dimensions = Some(dimensions);
        self
    }
}

/// The answer to an embeddings call: one vector per input, at each input's `index`, in input order.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingsResult {
    pub id: String,
    pub model: String,
    pub data: Vec<Embedding>,
    pub usage: InvokeUsage,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Embedding {
    pub index: u32,
    pub embedding: Vec<f64>,
}
