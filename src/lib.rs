//! CogniCloud SDK: call AI Gateway endpoints and manage them.
//!
//! Every call to a product sends a product key and the account key from Settings → API keys together. The web builder
//! and the marketplace take the account key alone for now. This version covers AI Gateway endpoints, browser sessions,
//! mail, databases, the web builder, agents, their runs and the marketplace.

mod agents;
mod browser;
mod client;
mod credentials;
mod database;
mod embeddings;
mod endpoints;
mod error;
mod files;
mod http;
mod keys;
mod mail;
mod marketplace;
mod responses;
mod secret;
mod stream;
mod web_builder;

pub use agents::{
    ActionLog, ActionLogList, ActionLogListOptions, Agent, AgentCredential, AgentStatus, AgentsApi,
    CreateAgent, CreateSkill, CreatedAgent, PublishAgent, Run, RunErrorCode, RunList,
    RunListOptions, RunStatus, RunStepDetail, RunWaitOptions, RunsApi, Skill, SkillVersion,
    SkillsApi, UpdateAgent, UpdateSkill,
};
pub use browser::{
    AgentRun, AgentRunStep, BrowserApi, BrowserConcurrency, BrowserPage, BrowserPageAction,
    BrowserPageElement, BrowserProfile, BrowserScreen, BrowserSession, BrowserSessionList,
    BrowserSessionsApi, BrowserStatus, CreateBrowserSession, CreateStepRun, ProxyCountry, RunStep,
    StartAgentRun, StepRun,
};
pub use client::{encode_tenant, Client, ClientBuilder, EndpointClient, Locale, Options};
pub use database::{
    DatabaseAction, DatabaseActionList, DatabaseActionListOptions, DatabaseApi, DatabaseData,
    DatabaseTable, HostedDatabase, QueryResult, ResultColumn, SelectQuery, Statement, TableApi,
    TableColumn, WriteQuery,
};
pub use embeddings::{Embedding, EmbeddingInput, EmbeddingsRequest, EmbeddingsResult};
pub use endpoints::{
    CreateEndpoint, CreatedEndpoint, DailyUsage, DailyUsageRange, Endpoint, EndpointStatus,
    EndpointUsage, EndpointsApi, FieldMapping, FinishReason, InvokeResponse, InvokeUsage,
    UpdateEndpoint,
};
pub use error::{Error, Result};
pub use files::{
    image_part, input_file, input_image, input_text, pdf_part, text_part, ImageType, MAX_FILE_BYTES,
};
pub use keys::{parse_key, KeyKind, ParsedKey};
pub use mail::{
    CreatedMailbox, MailApi, MailConnectionSettings, MailDnsRecord, MailDomain, MailDomainList,
    MailDomainsApi, MailServerEndpoint, Mailbox, MailboxList, MailboxListOptions,
    RotatedMailboxPassword,
};
pub use marketplace::{MarketplaceApi, Template, TemplatePublisher, TemplateSkill};
pub use responses::{McpTool, ResponseResult, ResponsesRequest, ToolChoice};
pub use secret::Secret;
pub use stream::{ResponseStream, StreamEvent};
pub use web_builder::{
    Application, CreateApplication, GithubRepositories, GithubRepository, UpdateApplication,
    WebBuilderAction, WebBuilderActionList, WebBuilderActionListOptions, WebBuilderApi,
};
