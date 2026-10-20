use serde::{Deserialize, Serialize};

use crate::client::encode;
use crate::error::Result;
use crate::http::{Call, Http};
use crate::secret::Secret;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EndpointStatus {
    Active,
    Paused,
}

/// Maps one of the application's fields to the provider's field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldMapping {
    pub source: String,
    pub target: String,
}

impl FieldMapping {
    pub fn new(source: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    pub id: String,
    pub name: String,
    pub provider_id: String,
    pub model: String,
    pub url: String,
    pub credential_prefix: String,
    pub token_limit: u64,
    pub tokens_used: u64,
    pub status: EndpointStatus,
    pub request_mapping: Vec<FieldMapping>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEndpoint {
    pub name: String,
    pub provider_id: String,
    pub model: String,
    pub token_limit: u64,
}

/// Only the fields that are `Some` are sent.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEndpoint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_limit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<EndpointStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_mapping: Option<Vec<FieldMapping>>,
}

#[derive(Debug, Deserialize)]
pub struct Credential {
    pub secret: Secret<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedEndpoint {
    pub endpoint: Endpoint,
    /// Shown once: store it before you lose it.
    pub credential: Credential,
}

#[derive(Debug, Deserialize)]
pub struct RotatedCredential {
    pub secret: Secret<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointUsage {
    pub endpoint_id: String,
    pub period_start: String,
    pub period_end: String,
    pub tokens_used: u64,
    pub token_limit: u64,
    pub tokens_remaining: u64,
    pub requests: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyUsage {
    pub day: String,
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

/// Dates are `YYYY-MM-DD` in UTC. Empty means the default range.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DailyUsageRange {
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ContentFilter,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvokeUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

/// The answer every model returns from an endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvokeResponse {
    pub id: String,
    pub model: String,
    pub output: String,
    pub finish_reason: Option<FinishReason>,
    pub usage: InvokeUsage,
}

#[derive(Deserialize)]
struct DailyUsageList {
    days: Vec<DailyUsage>,
}

/// Endpoint management, from [`Client::endpoints`](crate::Client::endpoints).
pub struct EndpointsApi<'a> {
    http: &'a Http,
    token: &'a str,
    tenant: &'a str,
}

impl<'a> EndpointsApi<'a> {
    pub(crate) fn new(http: &'a Http, token: &'a str, tenant: &'a str) -> Self {
        Self {
            http,
            token,
            tenant,
        }
    }

    fn session(&self) -> Vec<(&'static str, String)> {
        vec![
            ("authorization", format!("Bearer {}", self.token)),
            ("tenant_id", self.tenant.to_owned()),
        ]
    }

    fn path(id: &str, rest: &str) -> String {
        format!("/endpoints/{}{rest}", encode(id))
    }

    pub async fn create(&self, input: CreateEndpoint) -> Result<CreatedEndpoint> {
        self.http
            .send(Call {
                method: reqwest::Method::POST,
                path: "/endpoints".to_owned(),
                headers: self.session(),
                query: Vec::new(),
                body: Some(
                    serde_json::to_value(input).map_err(|e| crate::Error::Decode(e.to_string()))?,
                ),
                retry: false,
            })
            .await
    }

    pub async fn list(&self) -> Result<Vec<Endpoint>> {
        self.read("/endpoints", Vec::new()).await
    }

    pub async fn get(&self, id: &str) -> Result<Endpoint> {
        self.read(&Self::path(id, ""), Vec::new()).await
    }

    /// Sends only the fields that are set on `patch`.
    pub async fn update(&self, id: &str, patch: UpdateEndpoint) -> Result<Endpoint> {
        self.http
            .send(Call {
                method: reqwest::Method::PATCH,
                path: Self::path(id, ""),
                headers: self.session(),
                query: Vec::new(),
                body: Some(
                    serde_json::to_value(patch).map_err(|e| crate::Error::Decode(e.to_string()))?,
                ),
                retry: false,
            })
            .await
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        self.http
            .send(Call {
                method: reqwest::Method::DELETE,
                path: Self::path(id, ""),
                headers: self.session(),
                query: Vec::new(),
                body: None,
                retry: false,
            })
            .await
    }

    /// Issues a new credential. The old one stops working at once.
    pub async fn rotate_credential(&self, id: &str) -> Result<RotatedCredential> {
        self.http
            .send(Call {
                method: reqwest::Method::POST,
                path: Self::path(id, "/credential/rotate"),
                headers: self.session(),
                query: Vec::new(),
                body: None,
                retry: false,
            })
            .await
    }

    pub async fn usage(&self, id: &str) -> Result<EndpointUsage> {
        self.read(&Self::path(id, "/usage"), Vec::new()).await
    }

    /// Tokens per UTC day. Defaults to the 30 days ending today; days without calls are omitted.
    pub async fn daily_usage(&self, id: &str, range: DailyUsageRange) -> Result<Vec<DailyUsage>> {
        let mut query = Vec::new();
        if let Some(from) = range.from {
            query.push(("from", from));
        }
        if let Some(to) = range.to {
            query.push(("to", to));
        }
        let list: DailyUsageList = self.read(&Self::path(id, "/usage/daily"), query).await?;
        Ok(list.days)
    }

    async fn read<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: Vec<(&str, String)>,
    ) -> Result<T> {
        self.http
            .send(Call {
                method: reqwest::Method::GET,
                path: path.to_owned(),
                headers: self.session(),
                query,
                body: None,
                retry: true,
            })
            .await
    }
}
