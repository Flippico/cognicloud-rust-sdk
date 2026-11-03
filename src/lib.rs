//! CogniCloud SDK: call AI Gateway endpoints and manage them.
//!
//! This version covers AI Gateway endpoints, browser sessions, mail, databases, the web builder, agents, their runs
//! and the marketplace. Scraping, vault and activity come later.

mod agents;
mod browser;
mod client;
mod database;
mod embeddings;
mod endpoints;
mod error;
mod files;
mod http;
mod mail;
mod marketplace;
mod responses;
mod secret;
mod stream;
mod web_builder;

pub use agents::{
    ActionLog, ActionLogList, ActionLogListOptions, Agent, AgentCredential, AgentKey, AgentKeysApi,
    AgentStatus, AgentsApi, CreateAgent, CreateSkill, CreatedAgent, PublishAgent, Run,
    RunErrorCode, RunList, RunListOptions, RunStatus, RunStepDetail, RunWaitOptions, RunsApi,
    Skill, SkillVersion, SkillsApi, UpdateAgent, UpdateSkill,
};
pub use browser::{
    AgentRun, AgentRunStep, BrowserApi, BrowserConcurrency, BrowserPage, BrowserPageAction,
    BrowserPageElement, BrowserProfile, BrowserScreen, BrowserSession, BrowserSessionList,
    BrowserSessionsApi, BrowserStatus, CreateBrowserSession, CreateStepRun, ProxyCountry, RunStep,
    StartAgentRun, StepRun,
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
pub use marketplace::{MarketplaceApi, Template, TemplatePublisher, TemplateSkill};
pub use responses::{McpTool, ResponseResult, ResponsesRequest, ToolChoice};
pub use secret::Secret;
pub use stream::{ResponseStream, StreamEvent};
pub use web_builder::{
    Application, CreateApplication, GithubRepositories, GithubRepository, UpdateApplication,
    WebBuilderAction, WebBuilderActionList, WebBuilderActionListOptions, WebBuilderApi,
};
