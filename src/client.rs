use std::fmt;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::browser::BrowserApi;
use crate::embeddings::{EmbeddingsRequest, EmbeddingsResult};
use crate::endpoints::EndpointsApi;
use crate::error::{Error, Result};
use crate::http::{Call, Http};
use crate::responses::{ResponseResult, ResponsesRequest};

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
    /// Default `https://cognicloud.flippi.co/api/v1/ai-gateway`.
    pub base_url: String,
    /// The tenant (workspace) to manage, and the tenant that owns an endpoint called by ID. Default `"cognicloud"`.
    pub tenant: String,
    pub locale: Locale,
    /// Per request. Default 30 seconds.
    pub timeout: Duration,
    /// For `429` and `5xx`, on calls that are safe to repeat. Default 2.
    pub max_retries: u32,
    /// The browser service's base. Default `https://cognicloud.flippi.co/api/v1/browser`.
    pub browser_base_url: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            base_url: "https://cognicloud.flippi.co/api/v1/ai-gateway".to_owned(),
            tenant: "cognicloud".to_owned(),
            locale: Locale::En,
            timeout: Duration::from_secs(30),
            max_retries: 2,
            browser_base_url: "https://cognicloud.flippi.co/api/v1/browser".to_owned(),
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
#[derive(Debug, Default)]
pub struct ClientBuilder {
    token: Option<String>,
    options: Options,
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

    /// The tenant (workspace) to manage, sent as the `tenant_id` header. Default `"cognicloud"`.
    pub fn tenant(mut self, tenant: impl Into<String>) -> Self {
        self.options.tenant = tenant.into();
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

    pub fn build(self) -> Result<Client> {
        let token = self
            .token
            .ok_or_else(|| Error::Config("a session token is required".to_owned()))?;
        let mut browser = http(&self.options)?;
        browser.base_url = self.options.browser_base_url.clone();
        Ok(Client {
            token,
            tenant: self.options.tenant.clone(),
            http: http(&self.options)?,
            browser,
        })
    }
}

/// Manages CogniCloud resources: AI Gateway endpoints and browser sessions.
pub struct Client {
    token: String,
    tenant: String,
    http: Http,
    browser: Http,
}

impl Client {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    pub fn endpoints(&self) -> EndpointsApi<'_> {
        EndpointsApi::new(&self.http, &self.token, &self.tenant)
    }

    pub fn browser(&self) -> BrowserApi<'_> {
        BrowserApi::new(&self.browser, &self.token, &self.tenant)
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
