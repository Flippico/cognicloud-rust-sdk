use serde::{Deserialize, Serialize};

use crate::client::encode;
use crate::error::Result;
use crate::http::{Call, Http};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Application {
    pub id: String,
    pub subdomain: String,
    pub url: String,
    /// `building`, `running`, `failed` or `unknown`.
    pub status: String,
    pub repo_url: Option<String>,
    pub branch: Option<String>,
    pub build_command: String,
    pub output_dir: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateApplication {
    /// The application is served at `https://<subdomain>.cognicloud.dev`.
    pub subdomain: String,
    /// A public GitHub, GitLab or Bitbucket repository. Leave out to build the React starter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_url: Option<String>,
    /// Default `main`. Needs `repo_url`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Default `npm run build`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_command: Option<String>,
    /// The folder the build writes to. Default `dist`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_dir: Option<String>,
}

#[derive(Deserialize)]
struct ApplicationList {
    items: Vec<Application>,
}

#[derive(Deserialize)]
struct BuildLogs {
    logs: String,
}

/// Builds React applications and hosts them on their own subdomain.
pub struct WebBuilderApi<'a> {
    http: &'a Http,
    headers: Vec<(&'static str, String)>,
}

impl<'a> WebBuilderApi<'a> {
    pub(crate) fn new(http: &'a Http, token: &str, tenant: &str) -> Self {
        let headers = vec![
            ("authorization", format!("Bearer {token}")),
            ("tenant_id", tenant.to_owned()),
        ];
        Self { http, headers }
    }

    async fn call<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: String,
        body: Option<serde_json::Value>,
        retry: bool,
    ) -> Result<T> {
        self.http
            .send(Call {
                method,
                path,
                headers: self.headers.clone(),
                query: Vec::new(),
                body,
                retry,
            })
            .await
    }

    /// Newest first.
    pub async fn list(&self) -> Result<Vec<Application>> {
        let list: ApplicationList = self
            .call(reqwest::Method::GET, "/applications".to_owned(), None, true)
            .await?;
        Ok(list.items)
    }

    /// Starts the build and returns at once. Read `status` and the logs to follow it.
    pub async fn create(&self, input: &CreateApplication) -> Result<Application> {
        let body = serde_json::to_value(input).unwrap_or_default();
        self.call(
            reqwest::Method::POST,
            "/applications".to_owned(),
            Some(body),
            false,
        )
        .await
    }

    pub async fn get(&self, id: &str) -> Result<Application> {
        self.call(
            reqwest::Method::GET,
            format!("/applications/{}", encode(id)),
            None,
            true,
        )
        .await
    }

    /// Builds the latest code. The running version keeps serving until the new one is ready.
    pub async fn redeploy(&self, id: &str) -> Result<Application> {
        self.call(
            reqwest::Method::POST,
            format!("/applications/{}/redeploy", encode(id)),
            None,
            false,
        )
        .await
    }

    /// The last 500 lines of the latest build.
    pub async fn logs(&self, id: &str) -> Result<String> {
        let logs: BuildLogs = self
            .call(
                reqwest::Method::GET,
                format!("/applications/{}/logs", encode(id)),
                None,
                true,
            )
            .await?;
        Ok(logs.logs)
    }

    /// Takes the site offline and frees its subdomain. This cannot be undone.
    pub async fn delete(&self, id: &str) -> Result<()> {
        self.call(
            reqwest::Method::DELETE,
            format!("/applications/{}", encode(id)),
            None,
            false,
        )
        .await
    }
}
