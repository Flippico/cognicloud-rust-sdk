use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::http::{Call, Http};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowserStatus {
    Active,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserScreen {
    pub width: u32,
    pub height: u32,
}

/// A session is [`BrowserStatus::Active`] until it stops; then its `cdp_url` and `live_url` are `None`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserSession {
    pub id: String,
    pub status: BrowserStatus,
    /// WebSocket for Playwright, Puppeteer or any CDP client. Carries the session token; treat it as a secret.
    pub cdp_url: Option<String>,
    /// A page where people can watch the session. Carries the session token; treat it as a secret.
    pub live_url: Option<String>,
    pub profile_id: Option<String>,
    pub proxy_country_code: Option<String>,
    pub solve_captchas: bool,
    pub recording: bool,
    pub screen: Option<BrowserScreen>,
    pub timeout_at: String,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BrowserConcurrency {
    pub active: u64,
    pub limit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BrowserSessionList {
    pub items: Vec<BrowserSession>,
    pub concurrency: BrowserConcurrency,
}

/// The options for a new session. Proxies, screens, captchas and recording are not available yet.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBrowserSession {
    /// Minutes until the session stops. Default 15; at most 240.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_minutes: Option<u32>,
    /// Start from a saved profile, with the logins it holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProxyCountry {
    pub code: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserProfile {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub created_at: String,
    pub last_used_at: Option<String>,
    /// Deleted after this time unless a session starts from it.
    pub expires_at: String,
}

/// Manages browser sessions with the signed-in person's session.
pub struct BrowserApi<'a> {
    http: &'a Http,
    headers: Vec<(&'static str, String)>,
}

impl<'a> BrowserApi<'a> {
    pub(crate) fn new(http: &'a Http, token: &str, tenant: &str) -> Self {
        Self {
            http,
            headers: vec![
                ("authorization", format!("Bearer {token}")),
                ("tenant_id", tenant.to_owned()),
            ],
        }
    }

    pub fn sessions(&self) -> BrowserSessionsApi<'_> {
        BrowserSessionsApi { api: self }
    }

    /// Proxy countries a session can route through. Empty until proxies are available.
    pub async fn countries(&self) -> Result<Vec<ProxyCountry>> {
        self.get("/countries").await
    }

    /// Saved profiles, newest first. Save one from a running session with [`BrowserSessionsApi::save_profile`].
    pub async fn profiles(&self) -> Result<Vec<BrowserProfile>> {
        self.get("/profiles").await
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.http
            .send(Call {
                method: reqwest::Method::GET,
                path: path.to_owned(),
                headers: self.headers.clone(),
                query: Vec::new(),
                body: None,
                retry: true,
            })
            .await
    }
}

/// Starts, lists, reads and stops browser sessions.
pub struct BrowserSessionsApi<'a> {
    api: &'a BrowserApi<'a>,
}

impl BrowserSessionsApi<'_> {
    /// Starts a session. Its `cdp_url` and `live_url` carry the session token; treat them as secrets.
    pub async fn create(&self, input: &CreateBrowserSession) -> Result<BrowserSession> {
        let body =
            serde_json::to_value(input).map_err(|error| crate::Error::Config(error.to_string()))?;
        self.api
            .http
            .send(Call {
                method: reqwest::Method::POST,
                path: "/sessions".to_owned(),
                headers: self.api.headers.clone(),
                query: Vec::new(),
                body: Some(body),
                retry: false,
            })
            .await
    }

    /// Newest first, with the workspace's concurrency.
    pub async fn list(&self) -> Result<BrowserSessionList> {
        self.api.get("/sessions").await
    }

    pub async fn get(&self, id: &str) -> Result<BrowserSession> {
        self.api
            .get(&format!("/sessions/{}", crate::client::encode(id)))
            .await
    }

    /// Closes the session's browser cleanly, keeps its profile under `name` and ends the session. Start a session with
    /// the returned `id` as [`CreateBrowserSession::profile_id`] to continue from those logins. A profile holds
    /// cookies and logins, so keep it private. Names are 1 to 80 characters and unique per person.
    pub async fn save_profile(&self, id: &str, name: &str) -> Result<BrowserProfile> {
        self.api
            .http
            .send(Call {
                method: reqwest::Method::POST,
                path: format!("/sessions/{}/profile", crate::client::encode(id)),
                headers: self.api.headers.clone(),
                query: Vec::new(),
                body: Some(serde_json::json!({ "name": name })),
                retry: false,
            })
            .await
    }

    /// Closing the CDP connection does not stop a session; this does.
    pub async fn stop(&self, id: &str) -> Result<BrowserSession> {
        self.api
            .http
            .send(Call {
                method: reqwest::Method::POST,
                path: format!("/sessions/{}/stop", crate::client::encode(id)),
                headers: self.api.headers.clone(),
                query: Vec::new(),
                body: None,
                retry: false,
            })
            .await
    }
}
