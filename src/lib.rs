//! CogniCloud SDK: call AI Gateway endpoints and manage them.
//!
//! This first version covers AI Gateway endpoints. Agents, browser, scraping, vault and activity come later.

mod client;
mod endpoints;
mod error;
mod http;
mod secret;

pub use client::{Client, ClientBuilder, EndpointClient, Locale, Options};
pub use endpoints::{
    CreateEndpoint, CreatedEndpoint, DailyUsage, DailyUsageRange, Endpoint, EndpointStatus,
    EndpointUsage, EndpointsApi, FieldMapping, FinishReason, InvokeResponse, InvokeUsage,
    RotatedCredential, UpdateEndpoint,
};
pub use error::{Error, Result};
pub use secret::Secret;
