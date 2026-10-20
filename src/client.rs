use std::fmt;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::endpoints::EndpointsApi;
use crate::error::{Error, Result};
use crate::http::{Call, Http};

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
    /// Default `https://api.cognicloud.dev/v1`.
    pub base_url: String,
    pub locale: Locale,
    /// Per request. Default 30 seconds.
    pub timeout: Duration,
    /// For `429` and `5xx`, on calls that are safe to repeat. Default 2.
    pub max_retries: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            base_url: "https://api.cognicloud.dev/v1".to_owned(),
            locale: Locale::En,
            timeout: Duration::from_secs(30),
            max_retries: 2,
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

/// Calls one AI Gateway endpoint with its own credential.
pub struct EndpointClient {
    endpoint_id: String,
    credential: String,
    http: Http,
}

impl EndpointClient {
    /// A client with the default [`Options`].
    pub fn new(endpoint_id: impl Into<String>, credential: impl Into<String>) -> Self {
        Self::with_options(endpoint_id, credential, &Options::default())
            .expect("the default HTTP client always builds")
    }

    pub fn with_options(
        endpoint_id: impl Into<String>,
        credential: impl Into<String>,
        options: &Options,
    ) -> Result<Self> {
        Ok(Self {
            endpoint_id: endpoint_id.into(),
            credential: credential.into(),
            http: http(options)?,
        })
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
        let body = serde_json::to_value(body).map_err(|error| Error::Config(error.to_string()))?;
        self.http
            .send(Call {
                method: reqwest::Method::POST,
                path: format!("/invoke/{}", encode(&self.endpoint_id)),
                headers: vec![("authorization", format!("Bearer {}", self.credential))],
                query: Vec::new(),
                body: Some(body),
                retry: false,
            })
            .await
    }
}

impl fmt::Debug for EndpointClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EndpointClient")
            .field("endpoint_id", &self.endpoint_id)
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
    /// The signed-in person's Amadeus session token, sent as `x-amadeus-token`. Required.
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

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.options.timeout = timeout;
        self
    }

    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.options.max_retries = max_retries;
        self
    }

    pub fn build(self) -> Result<Client> {
        let token = self
            .token
            .ok_or_else(|| Error::Config("a session token is required".to_owned()))?;
        Ok(Client {
            token,
            http: http(&self.options)?,
        })
    }
}

/// Manages CogniCloud resources. This first version covers AI Gateway endpoints.
pub struct Client {
    token: String,
    http: Http,
}

impl Client {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    pub fn endpoints(&self) -> EndpointsApi<'_> {
        EndpointsApi::new(&self.http, &self.token)
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("token", &"Secret(****)")
            .finish()
    }
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
