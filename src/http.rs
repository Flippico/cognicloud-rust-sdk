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
    /// Relative to the base URL, or an absolute URL (an endpoint's issued invoke URL).
    pub path: String,
    pub headers: Vec<(&'static str, String)>,
    pub query: Vec<(&'a str, String)>,
    pub body: Option<serde_json::Value>,
    /// Calls that bill or change state are sent once: a retry could repeat them.
    pub retry: bool,
}

/// A body sent as it is, with its own content type. An upload is sent once, never retried.
pub(crate) struct RawCall<'a> {
    pub path: String,
    pub headers: Vec<(&'static str, String)>,
    pub query: Vec<(&'a str, String)>,
    pub body: Vec<u8>,
    pub content_type: &'static str,
}

#[derive(Default, serde::Deserialize)]
struct ErrorBody {
    error: Option<String>,
    message: Option<String>,
}

fn is_retryable(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}

fn target(base_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_owned();
    }
    format!("{}{}", base_url.trim_end_matches('/'), path)
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
                .request(call.method.clone(), target(&self.base_url, &call.path))
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
            return Err(api_error(status, &bytes));
        }
    }

    pub(crate) async fn send_raw<T: DeserializeOwned>(&self, call: RawCall<'_>) -> Result<T> {
        let mut request = self
            .client
            .post(target(&self.base_url, &call.path))
            .timeout(self.timeout)
            .header("accept", "application/json")
            .header("accept-language", self.locale.as_str())
            .header("content-type", call.content_type)
            .query(&call.query)
            .body(call.body);
        for (name, value) in &call.headers {
            request = request.header(*name, value.as_str());
        }
        let response = request.send().await.map_err(|error| {
            if error.is_timeout() {
                Error::Timeout
            } else {
                Error::Network(error)
            }
        })?;
        let status = response.status();
        if status.is_success() {
            let bytes = response.bytes().await.map_err(Error::Network)?;
            let body: &[u8] = if bytes.is_empty() { b"null" } else { &bytes };
            return serde_json::from_slice(body)
                .map_err(|error| Error::Decode(error.to_string()));
        }
        let bytes = response.bytes().await.unwrap_or_default();
        Err(api_error(status, &bytes))
    }

    /// One attempt, never retried, and no total timeout: the answer is a stream that may run for minutes. The caller
    /// reads the body as it arrives.
    pub(crate) async fn open(&self, call: Call<'_>) -> Result<reqwest::Response> {
        let mut request = self
            .client
            .request(call.method.clone(), target(&self.base_url, &call.path))
            .header("accept", "text/event-stream")
            .header("accept-language", self.locale.as_str())
            .query(&call.query);
        for (name, value) in &call.headers {
            request = request.header(*name, value.as_str());
        }
        if let Some(body) = &call.body {
            request = request.json(body);
        }
        let response = request.send().await.map_err(|error| {
            if error.is_timeout() {
                Error::Timeout
            } else {
                Error::Network(error)
            }
        })?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let bytes = response.bytes().await.unwrap_or_default();
        Err(api_error(status, &bytes))
    }
}

fn api_error(status: reqwest::StatusCode, bytes: &[u8]) -> Error {
    let body: ErrorBody = serde_json::from_slice(bytes).unwrap_or_default();
    Error::Api {
        status: status.as_u16(),
        code: body.error.unwrap_or_else(|| "http_error".to_owned()),
        message: body.message.unwrap_or_else(|| {
            status
                .canonical_reason()
                .unwrap_or("The request failed.")
                .to_owned()
        }),
    }
}
