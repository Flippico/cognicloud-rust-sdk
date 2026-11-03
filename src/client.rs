use std::fmt;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::agents::{AgentsApi, RunsApi, SkillsApi};
use crate::browser::BrowserApi;
use crate::database::{self, DatabaseApi, DatabaseTable, QueryResult, Statement, TableApi};
use crate::embeddings::{EmbeddingsRequest, EmbeddingsResult};
use crate::endpoints::EndpointsApi;
use crate::error::{Error, Result};
use crate::http::{Call, Http};
use crate::mail::MailApi;
use crate::marketplace::MarketplaceApi;
use crate::responses::{ResponseResult, ResponsesRequest};
use crate::stream::ResponseStream;
use crate::web_builder::WebBuilderApi;

/// Language for localised responses, sent as `Accept-Language`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    #[default]
    En,
    Pl,
}

impl Locale {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Pl => "pl",
        }
    }
}

/// Settings shared by every client. The defaults match the JavaScript SDK.
#[derive(Debug, Clone)]
pub struct Options {
    /// Default `https://cognicloud.dev/api/v1/ai-gateway`.
    pub base_url: String,
    /// The tenant (workspace) to manage, and the tenant that owns an endpoint called by ID. Default `"cognicloud"`.
    pub tenant: String,
    pub locale: Locale,
    /// Per request. Default 30 seconds.
    pub timeout: Duration,
    /// For `429` and `5xx`, on calls that are safe to repeat. Default 2.
    pub max_retries: u32,
    /// The browser service's base. Default `https://cognicloud.dev/api/v1/browser`.
    pub browser_base_url: String,
    /// The mail service's base. Default `https://cognicloud.dev/api/v1/mail`.
    pub mail_base_url: String,
    /// The database service's base. Default `https://cognicloud.dev/api/v1/database`.
    pub database_base_url: String,
    /// The web builder's base. Default `https://cognicloud.dev/api/v1/web-builder`.
    pub web_builder_base_url: String,
    /// The agents service's base. Default `https://cognicloud.dev/api/v1/agents`.
    pub agents_base_url: String,
    /// The marketplace's base. Default `https://cognicloud.dev/api/v1/marketplace`.
    pub marketplace_base_url: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            base_url: "https://cognicloud.dev/api/v1/ai-gateway".to_owned(),
            tenant: "cognicloud".to_owned(),
            locale: Locale::En,
            timeout: Duration::from_secs(30),
            max_retries: 2,
            browser_base_url: "https://cognicloud.dev/api/v1/browser".to_owned(),
            mail_base_url: "https://cognicloud.dev/api/v1/mail".to_owned(),
            database_base_url: "https://cognicloud.dev/api/v1/database".to_owned(),
            web_builder_base_url: "https://cognicloud.dev/api/v1/web-builder".to_owned(),
            agents_base_url: "https://cognicloud.dev/api/v1/agents".to_owned(),
            marketplace_base_url: "https://cognicloud.dev/api/v1/marketplace".to_owned(),
        }
    }
}

fn http(options: &Options) -> Result<Http> {
    let client = reqwest::Client::builder()
        .build()
        .map_err(|error| Error::Config(error.to_string()))?;
    Ok(Http {
        client,
        base_url: options.base_url.clone(),
        locale: options.locale,
        timeout: options.timeout,
        max_retries: options.max_retries,
    })
}

/// How an invoke URL names its tenant: the tenant's name as UTF-8, in lowercase hex.
pub fn encode_tenant(tenant: &str) -> String {
    tenant.bytes().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn encode(segment: &str) -> String {
    segment
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

/// Calls one AI Gateway endpoint with its own credential.
pub struct EndpointClient {
    url: String,
    credential: String,
    http: Http,
}

impl EndpointClient {
    /// A client for the endpoint `endpoint_id` in the default tenant, with the default [`Options`].
    pub fn new(endpoint_id: impl Into<String>, credential: impl Into<String>) -> Self {
        Self::with_options(endpoint_id, credential, &Options::default())
            .expect("the default HTTP client always builds")
    }

    /// A client for the endpoint `endpoint_id`, with the given options.
    pub fn with_options(
        endpoint_id: impl Into<String>,
        credential: impl Into<String>,
        options: &Options,
    ) -> Result<Self> {
        let base = options.base_url.trim_end_matches('/');
        let url = format!(
            "{base}/invoke/{}/{}",
            encode_tenant(&options.tenant),
            encode(&endpoint_id.into())
        );
        Ok(Self {
            url,
            credential: credential.into(),
            http: http(options)?,
        })
    }

    /// A client for an invoke URL as CogniCloud issued it (the endpoint's `url`).
    pub fn from_url(url: impl Into<String>, credential: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            credential: credential.into(),
            http: http(&Options::default()).expect("the default HTTP client always builds"),
        }
    }

    /// The address every invoke is sent to.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Sends the body through the endpoint's request mapping. Choose the response type: [`InvokeResponse`] is
    /// the shape every model returns, or deserialise into your own struct.
    ///
    /// Never retried, because a retried call can be billed twice.
    pub async fn invoke<Req, Res>(&self, body: &Req) -> Result<Res>
    where
        Req: Serialize + ?Sized,
        Res: DeserializeOwned,
    {
        self.invoke_at(self.url.clone(), body).await
    }

    async fn invoke_at<Req, Res>(&self, path: String, body: &Req) -> Result<Res>
    where
        Req: Serialize + ?Sized,
        Res: DeserializeOwned,
    {
        let body = serde_json::to_value(body).map_err(|error| Error::Config(error.to_string()))?;
        self.http
            .send(Call {
                method: reqwest::Method::POST,
                path,
                headers: vec![("authorization", format!("Bearer {}", self.credential))],
                query: Vec::new(),
                body: Some(body),
                retry: false,
            })
            .await
    }
}

impl EndpointClient {
    /// Streams a Chat or Responses call: sets `stream: true` on the body and returns its events as they arrive. The
    /// body must be a JSON object. Never retried.
    pub async fn stream<Req>(&self, body: &Req) -> Result<ResponseStream>
    where
        Req: Serialize + ?Sized,
    {
        let mut body =
            serde_json::to_value(body).map_err(|error| Error::Config(error.to_string()))?;
        let object = body
            .as_object_mut()
            .ok_or_else(|| Error::Config("a streamed body must be a JSON object".to_owned()))?;
        object.insert("stream".to_owned(), serde_json::Value::Bool(true));
        let response = self
            .http
            .open(Call {
                method: reqwest::Method::POST,
                path: self.url.clone(),
                headers: vec![("authorization", format!("Bearer {}", self.credential))],
                query: Vec::new(),
                body: Some(body),
                retry: false,
            })
            .await?;
        Ok(ResponseStream::new(response))
    }

    /// Streams a Responses call, for tools or chaining. See [`EndpointClient::stream`].
    pub async fn stream_responses(&self, request: &ResponsesRequest) -> Result<ResponseStream> {
        self.stream(request).await
    }

    /// Embeds text through the endpoint's embedding deployment. Billed as prompt tokens. Never retried.
    pub async fn embeddings(&self, request: &EmbeddingsRequest) -> Result<EmbeddingsResult> {
        self.invoke_at(format!("{}/embeddings", self.url), request)
            .await
    }
}

impl EndpointClient {
    /// Sends a Responses call: text or messages, optional MCP tools, and optionally an earlier response to continue.
    ///
    /// Never retried, because a retried call can be billed twice.
    pub async fn responses(&self, request: &ResponsesRequest) -> Result<ResponseResult> {
        self.invoke(request).await
    }
}

impl fmt::Debug for EndpointClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EndpointClient")
            .field("url", &self.url)
            .field("credential", &"Secret(****)")
            .finish()
    }
}

/// Builds a [`Client`].
#[derive(Default)]
pub struct ClientBuilder {
    token: Option<String>,
    options: Options,
    tenant_set: bool,
}

impl fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("token", &self.token.as_ref().map(|_| "Secret(****)"))
            .field("options", &self.options)
            .field("tenant_set", &self.tenant_set)
            .finish()
    }
}

impl ClientBuilder {
    /// The signed-in person's Amadeus session token, sent as `Authorization: Bearer`. Required.
    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    pub fn locale(mut self, locale: Locale) -> Self {
        self.options.locale = locale;
        self
    }

    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.options.base_url = base_url.into();
        self
    }

    /// The tenant (workspace) to manage, sent as the `tenant_id` header. Default `"cognicloud"`. A `cga_` key or
    /// `ccag_` credential names its workspace, so without this no tenant goes to the agents service or marketplace.
    pub fn tenant(mut self, tenant: impl Into<String>) -> Self {
        self.options.tenant = tenant.into();
        self.tenant_set = true;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.options.timeout = timeout;
        self
    }

    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.options.max_retries = max_retries;
        self
    }

    pub fn browser_base_url(mut self, browser_base_url: impl Into<String>) -> Self {
        self.options.browser_base_url = browser_base_url.into();
        self
    }

    pub fn mail_base_url(mut self, mail_base_url: impl Into<String>) -> Self {
        self.options.mail_base_url = mail_base_url.into();
        self
    }

    pub fn database_base_url(mut self, database_base_url: impl Into<String>) -> Self {
        self.options.database_base_url = database_base_url.into();
        self
    }

    pub fn web_builder_base_url(mut self, web_builder_base_url: impl Into<String>) -> Self {
        self.options.web_builder_base_url = web_builder_base_url.into();
        self
    }

    pub fn agents_base_url(mut self, agents_base_url: impl Into<String>) -> Self {
        self.options.agents_base_url = agents_base_url.into();
        self
    }

    pub fn marketplace_base_url(mut self, marketplace_base_url: impl Into<String>) -> Self {
        self.options.marketplace_base_url = marketplace_base_url.into();
        self
    }

    pub fn build(self) -> Result<Client> {
        let token = self
            .token
            .filter(|token| !token.trim().is_empty())
            .ok_or_else(|| {
                Error::Config(
                    "a token is required: pass the signed-in person's token, or a browser key for browser calls".to_owned(),
                )
            })?;
        let mut browser = http(&self.options)?;
        browser.base_url = self.options.browser_base_url.clone();
        let mut mail = http(&self.options)?;
        mail.base_url = self.options.mail_base_url.clone();
        let mut database = http(&self.options)?;
        database.base_url = self.options.database_base_url.clone();
        let mut web_builder = http(&self.options)?;
        web_builder.base_url = self.options.web_builder_base_url.clone();
        let mut agents = http(&self.options)?;
        agents.base_url = self.options.agents_base_url.clone();
        let mut marketplace = http(&self.options)?;
        marketplace.base_url = self.options.marketplace_base_url.clone();
        let names_workspace = token.starts_with("ccag_") || token.starts_with("cga_");
        let agents_tenant =
            (self.tenant_set || !names_workspace).then(|| self.options.tenant.clone());
        Ok(Client {
            token,
            tenant: self.options.tenant.clone(),
            agents_tenant,
            http: http(&self.options)?,
            browser,
            mail,
            database,
            web_builder,
            agents,
            marketplace,
        })
    }
}

/// Manages CogniCloud resources: AI Gateway endpoints and browser sessions.
pub struct Client {
    token: String,
    tenant: String,
    agents_tenant: Option<String>,
    http: Http,
    browser: Http,
    mail: Http,
    database: Http,
    web_builder: Http,
    agents: Http,
    marketplace: Http,
}

/// The browser product alone. The key is a browser key from the dashboard (Browser → API keys). It names its
/// workspace, so no tenant is sent and no Amadeus token is needed.
pub struct BrowserClient {
    key: String,
    browser: Http,
}

impl BrowserClient {
    pub fn new(key: impl Into<String>) -> Result<Self> {
        let options = Options::default();
        Self::with_base_url(key, options.browser_base_url)
    }

    /// For another environment, or a test server.
    pub fn with_base_url(
        key: impl Into<String>,
        browser_base_url: impl Into<String>,
    ) -> Result<Self> {
        let key = key.into();
        if key.trim().is_empty() {
            return Err(Error::Config(
                "a browser key is required: create one in the dashboard under Browser → API keys"
                    .to_owned(),
            ));
        }
        let mut browser = http(&Options::default())?;
        browser.base_url = browser_base_url.into();
        Ok(Self { key, browser })
    }

    pub fn browser(&self) -> BrowserApi<'_> {
        BrowserApi::new(&self.browser, &self.key, None)
    }
}

/// Queries one database. The key is a database key from the dashboard (Database → API keys). It names its database
/// and workspace, so no tenant is sent and no Amadeus token is needed.
pub struct DatabaseClient {
    key: String,
    database: Http,
}

impl DatabaseClient {
    pub fn new(key: impl Into<String>) -> Result<Self> {
        Self::with_base_url(key, Options::default().database_base_url)
    }

    /// For another environment, or a test server.
    pub fn with_base_url(
        key: impl Into<String>,
        database_base_url: impl Into<String>,
    ) -> Result<Self> {
        let key = key.into();
        if key.trim().is_empty() {
            return Err(Error::Config(
                "a database key is required: create one in the dashboard under Database → API keys"
                    .to_owned(),
            ));
        }
        let mut database = http(&Options::default())?;
        database.base_url = database_base_url.into();
        Ok(Self { key, database })
    }

    /// Runs one SQL statement, with `$1`, `$2` for `params`. It stops after 15 seconds and returns at most 1,000 rows
    /// and 5 MB. Never retried, because it may change data.
    pub async fn query(&self, sql: &str, params: &[serde_json::Value]) -> Result<QueryResult> {
        database::query(&self.database, &self.key, sql, params).await
    }

    /// Runs 1 to 20 statements that commit together, or none do. Needs a write key. Never retried.
    pub async fn transaction(&self, statements: &[Statement]) -> Result<Vec<QueryResult>> {
        database::transaction(&self.database, &self.key, statements).await
    }

    /// Tables and views in the `public` schema that this key can read.
    pub async fn tables(&self) -> Result<Vec<DatabaseTable>> {
        database::tables(&self.database, &self.key).await
    }

    pub fn table(&self, name: &str) -> TableApi<'_> {
        database::table(&self.database, &self.key, name)
    }
}

impl fmt::Debug for DatabaseClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DatabaseClient")
            .field("key", &"Secret(****)")
            .finish()
    }
}

impl Client {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    pub fn endpoints(&self) -> EndpointsApi<'_> {
        EndpointsApi::new(&self.http, &self.token, &self.tenant)
    }

    pub fn browser(&self) -> BrowserApi<'_> {
        BrowserApi::new(&self.browser, &self.token, Some(&self.tenant))
    }

    pub fn mail(&self) -> MailApi<'_> {
        MailApi::new(&self.mail, &self.token, &self.tenant)
    }

    pub fn database(&self) -> DatabaseApi<'_> {
        DatabaseApi::new(&self.database, &self.token, &self.tenant)
    }

    pub fn web_builder(&self) -> WebBuilderApi<'_> {
        WebBuilderApi::new(&self.web_builder, &self.token, &self.tenant)
    }

    /// Agents, their keys and action log. A `ccag_` credential gets `403 forbidden` here.
    pub fn agents(&self) -> AgentsApi<'_> {
        AgentsApi::new(&self.agents, &self.token, self.agents_tenant.as_deref())
    }

    pub fn skills(&self) -> SkillsApi<'_> {
        SkillsApi::new(&self.agents, &self.token, self.agents_tenant.as_deref())
    }

    /// Runs of agents. A client built with an agent's `ccag_` credential reaches only that agent's runs.
    pub fn runs(&self) -> RunsApi<'_> {
        RunsApi::new(&self.agents, &self.token, self.agents_tenant.as_deref())
    }

    pub fn marketplace(&self) -> MarketplaceApi<'_> {
        MarketplaceApi::new(
            &self.marketplace,
            &self.token,
            self.agents_tenant.as_deref(),
        )
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("tenant", &self.tenant)
            .field("token", &"Secret(****)")
            .finish()
    }
}
