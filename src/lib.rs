//! CogniCloud SDK: call AI Gateway endpoints and manage them.
//!
//! This version covers AI Gateway endpoints and browser sessions. Agents, scraping, vault and activity come later.

mod browser;
mod client;
mod embeddings;
mod endpoints;
mod error;
mod files;
mod http;
mod responses;
mod secret;
mod stream;

pub use browser::{
    BrowserApi, BrowserConcurrency, BrowserProfile, BrowserScreen, BrowserSession,
    BrowserSessionList, BrowserSessionsApi, BrowserStatus, CreateBrowserSession, ProxyCountry,
};
pub use client::{encode_tenant, Client, ClientBuilder, EndpointClient, Locale, Options};
pub use embeddings::{Embedding, EmbeddingInput, EmbeddingsRequest, EmbeddingsResult};
pub use endpoints::{
    CreateEndpoint, CreatedEndpoint, DailyUsage, DailyUsageRange, Endpoint, EndpointStatus,
    EndpointUsage, EndpointsApi, FieldMapping, FinishReason, InvokeResponse, InvokeUsage,
    RotatedCredential, UpdateEndpoint,
};
pub use error::{Error, Result};
pub use files::{
    image_part, input_file, input_image, input_text, pdf_part, text_part, ImageType, MAX_FILE_BYTES,
};
pub use responses::{McpTool, ResponseResult, ResponsesRequest, ToolChoice};
pub use secret::Secret;
pub use stream::{ResponseStream, StreamEvent};
