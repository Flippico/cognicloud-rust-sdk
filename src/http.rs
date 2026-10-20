use std::time::Duration;

use reqwest::Method;
use serde::de::DeserializeOwned;

use crate::client::Locale;
use crate::error::{Error, Result};

pub(crate) struct Http {
    pub client: reqwest::Client,
    pub base_url: String,
    pub locale: Locale,
    pub timeout: Duration,
    pub max_retries: u32,
}

pub(crate) struct Call<'a> {
    pub method: Method,
    pub path: String,
    pub headers: Vec<(&'static str, String)>,
    pub query: Vec<(&'a str, String)>,
    pub body: Option<serde_json::Value>,
    /// Calls that bill or change state are sent once: a retry could repeat them.
    pub retry: bool,
}

#[derive(Default, serde::Deserialize)]
struct ErrorBody {
    error: Option<String>,
    message: Option<String>,
}

fn is_retryable(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}

fn backoff(attempt: u32) -> Duration {
    Duration::from_millis(250 * 2u64.pow(attempt.min(6)))
}

impl Http {
    pub(crate) async fn send<T: DeserializeOwned>(&self, call: Call<'_>) -> Result<T> {
        let retries = if call.retry { self.max_retries } else { 0 };
        let mut attempt = 0;
        loop {
            let mut request = self
                .client
                .request(
                    call.method.clone(),
                    format!("{}{}", self.base_url.trim_end_matches('/'), call.path),
                )
                .timeout(self.timeout)
                .header("accept", "application/json")
                .header("accept-language", self.locale.as_str())
                .query(&call.query);
            for (name, value) in &call.headers {
                request = request.header(*name, value.as_str());
            }
            if let Some(body) = &call.body {
                request = request.json(body);
            }

            let response = match request.send().await {
                Ok(response) => response,
                Err(error) => {
                    if attempt < retries {
                        tokio::time::sleep(backoff(attempt)).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(if error.is_timeout() {
                        Error::Timeout
                    } else {
                        Error::Network(error)
                    });
                }
            };

            let status = response.status();
            if status.is_success() {
                let bytes = response.bytes().await.map_err(Error::Network)?;
                let bytes: &[u8] = if bytes.is_empty() { b"null" } else { &bytes };
                return serde_json::from_slice(bytes)
                    .map_err(|error| Error::Decode(error.to_string()));
            }
            if is_retryable(status.as_u16()) && attempt < retries {
                tokio::time::sleep(backoff(attempt)).await;
                attempt += 1;
                continue;
            }

            let bytes = response.bytes().await.unwrap_or_default();
            let body: ErrorBody = serde_json::from_slice(&bytes).unwrap_or_default();
            return Err(Error::Api {
                status: status.as_u16(),
                code: body.error.unwrap_or_else(|| "http_error".to_owned()),
                message: body.message.unwrap_or_else(|| {
                    status
                        .canonical_reason()
                        .unwrap_or("The request failed.")
                        .to_owned()
                }),
            });
        }
    }
}
