use std::fmt;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::agents::{AgentsApi, RunsApi, SkillsApi};
use crate::browser::BrowserApi;
use crate::database::DatabaseApi;
use crate::embeddings::{EmbeddingsRequest, EmbeddingsResult};
use crate::endpoints::EndpointsApi;
use crate::error::{Error, Result};
use crate::http::{Call, Http};
use crate::keys::parse_key;
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

/// Calls one AI Gateway endpoint with an account key or an AI Gateway key, as the endpoint's owner.
pub struct EndpointClient {
    url: String,
    credential: String,
    http: Http,
}

impl EndpointClient {
    /// A client for the endpoint `endpoint_id` in the key's workspace, with the default [`Options`].
    pub fn new(endpoint_id: impl Into<String>, api_key: impl Into<String>) -> Result<Self> {
        Self::with_options(endpoint_id, api_key, &Options::default())
    }

    /// A client for the endpoint `endpoint_id` in the key's workspace, with the given options.
    pub fn with_options(
        endpoint_id: impl Into<String>,
        api_key: impl Into<String>,
        options: &Options,
    ) -> Result<Self> {
        let credential = api_key.into();
        let tenant_hex = parse_key(&credential)?.tenant_hex;
        let base = options.base_url.trim_end_matches('/');
        let url = format!("{base}/invoke/{tenant_hex}/{}", encode(&endpoint_id.into()));
        Ok(Self {
            url,
            credential,
            http: http(options)?,
        })
    }

    /// A client for an invoke URL as CogniCloud issued it (the endpoint's `url`).
    pub fn from_url(url: impl Into<String>, api_key: impl Into<String>) -> Result<Self> {
        let credential = api_key.into();
        parse_key(&credential)?;
        Ok(Self {
            url: url.into(),
            credential,
            http: http(&Options::default())?,
        })
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
    api_key: Option<String>,
    options: Options,
}

impl fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("api_key", &self.api_key.as_ref().map(|_| "Secret(****)"))
            .field("options", &self.options)
            .finish()
    }
}

impl ClientBuilder {
    /// An account key (`cc_acc_`) for every product, or a product key (`cc_gw_`, `cc_db_`, `cc_br_`, `cc_ml_`,
    /// `cc_ag_`) for one, from Settings → API keys. It names its workspace. Required.
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
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
        let token = self.api_key.unwrap_or_default();
        parse_key(&token)?;
        let service = |base_url: &str| -> Result<Http> {
            let mut client = http(&self.options)?;
            client.base_url = base_url.to_owned();
            Ok(client)
        };
        Ok(Client {
            browser: service(&self.options.browser_base_url)?,
            mail: service(&self.options.mail_base_url)?,
            database: service(&self.options.database_base_url)?,
            web_builder: service(&self.options.web_builder_base_url)?,
            agents: service(&self.options.agents_base_url)?,
            marketplace: service(&self.options.marketplace_base_url)?,
            http: http(&self.options)?,
            options: self.options,
            token,
        })
    }
}

/// Every CogniCloud product with one API key.
pub struct Client {
    token: String,
    options: Options,
    http: Http,
    browser: Http,
    mail: Http,
    database: Http,
    web_builder: Http,
    agents: Http,
    marketplace: Http,
}

impl Client {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    pub fn endpoints(&self) -> EndpointsApi<'_> {
        EndpointsApi::new(&self.http, &self.token)
    }

    /// Calls one AI Gateway endpoint with this key, as its owner.
    pub fn endpoint(&self, endpoint_id: impl Into<String>) -> Result<EndpointClient> {
        EndpointClient::with_options(endpoint_id, self.token.clone(), &self.options)
    }

    pub fn browser(&self) -> BrowserApi<'_> {
        BrowserApi::new(&self.browser, &self.token, None)
    }

    pub fn mail(&self) -> MailApi<'_> {
        MailApi::new(&self.mail, &self.token)
    }

    pub fn database(&self) -> DatabaseApi<'_> {
        DatabaseApi::new(&self.database, &self.token)
    }

    pub fn web_builder(&self) -> WebBuilderApi<'_> {
        WebBuilderApi::new(&self.web_builder, &self.token)
    }

    /// Agents and their action log. A key narrowed to one agent gets `403 forbidden` on the list.
    pub fn agents(&self) -> AgentsApi<'_> {
        AgentsApi::new(&self.agents, &self.token, None)
    }

    pub fn skills(&self) -> SkillsApi<'_> {
        SkillsApi::new(&self.agents, &self.token, None)
    }

    /// Runs of agents. A key narrowed to one agent reaches only that agent's runs.
    pub fn runs(&self) -> RunsApi<'_> {
        RunsApi::new(&self.agents, &self.token, None)
    }

    pub fn marketplace(&self) -> MarketplaceApi<'_> {
        MarketplaceApi::new(&self.marketplace, &self.token, None)
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("api_key", &"Secret(****)")
            .finish()
    }
}
