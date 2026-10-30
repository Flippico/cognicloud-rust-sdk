//! CogniCloud SDK: call AI Gateway endpoints and manage them.
//!
//! This version covers AI Gateway endpoints and browser sessions. Agents, scraping, vault and activity come later.

mod browser;
mod client;
mod database;
mod embeddings;
mod endpoints;
mod error;
mod files;
mod http;
mod mail;
mod responses;
mod secret;
mod stream;

pub use browser::{
    BrowserApi, BrowserConcurrency, BrowserProfile, BrowserScreen, BrowserSession,
    BrowserSessionList, BrowserSessionsApi, BrowserStatus, CreateBrowserSession, ProxyCountry,
};
pub use client::{
    encode_tenant, BrowserClient, Client, ClientBuilder, DatabaseClient, EndpointClient, Locale,
    Options,
};
pub use database::{
    DatabaseAction, DatabaseActionList, DatabaseActionListOptions, DatabaseApi, DatabaseKey,
    DatabaseTable, HostedDatabase, QueryResult, ResultColumn, SelectQuery, Statement, TableApi,
    TableColumn, WriteQuery,
};
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
pub use mail::{
    CreatedMailbox, MailApi, MailConnectionSettings, MailDnsRecord, MailDomain, MailDomainList,
    MailDomainsApi, MailServerEndpoint, Mailbox, MailboxList, MailboxListOptions,
    RotatedMailboxPassword,
};
pub use responses::{McpTool, ResponseResult, ResponsesRequest, ToolChoice};
pub use secret::Secret;
pub use stream::{ResponseStream, StreamEvent};
