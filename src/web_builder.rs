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
    /// `building`, `running`, `failed`, `paused` or `unknown`.
    pub status: String,
    pub repo_url: Option<String>,
    pub branch: Option<String>,
    pub build_command: String,
    pub output_dir: String,
    pub created_at: String,
    /// Whether pushes to the branch build and deploy the site.
    #[serde(default)]
    pub auto_deploy: bool,
}

/// A field left out stays as it is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateApplication {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebBuilderAction {
    pub id: String,
    pub action: String,
    pub application_id: Option<String>,
    pub name: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<i64>,
    /// `started`, `succeeded` or `failed`.
    pub outcome: String,
    pub error_code: Option<String>,
}

/// Pass `next_cursor` as `cursor` for the next page. It is `None` on the last page.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebBuilderActionList {
    pub items: Vec<WebBuilderAction>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WebBuilderActionListOptions {
    pub limit: Option<u32>,
    pub cursor: Option<String>,
    /// One application's history.
    pub application_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubRepository {
    pub full_name: String,
    pub url: String,
    pub default_branch: String,
    pub private: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GithubRepositories {
    pub login: String,
    pub items: Vec<GithubRepository>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GithubAuthorization {
    authorize_url: String,
}

#[derive(Deserialize)]
struct GithubLogin {
    login: String,
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

    /// Changes build settings and starts a build.
    pub async fn update(&self, id: &str, changes: &UpdateApplication) -> Result<Application> {
        let body = serde_json::to_value(changes).unwrap_or_default();
        self.call(
            reqwest::Method::PATCH,
            format!("/applications/{}", encode(id)),
            Some(body),
            false,
        )
        .await
    }

    /// Takes the site offline and keeps its settings and subdomain.
    pub async fn pause(&self, id: &str) -> Result<Application> {
        self.call(
            reqwest::Method::POST,
            format!("/applications/{}/pause", encode(id)),
            None,
            false,
        )
        .await
    }

    /// Builds the site again after [`pause`](Self::pause).
    pub async fn resume(&self, id: &str) -> Result<Application> {
        self.call(
            reqwest::Method::POST,
            format!("/applications/{}/resume", encode(id)),
            None,
            false,
        )
        .await
    }

    /// The web server's latest request logs.
    pub async fn runtime_logs(&self, id: &str) -> Result<String> {
        let logs: BuildLogs = self
            .call(
                reqwest::Method::GET,
                format!("/applications/{}/runtime-logs", encode(id)),
                None,
                true,
            )
            .await?;
        Ok(logs.logs)
    }

    /// Creates a public GitHub repository from the starter and builds from it.
    pub async fn create_repository(&self, id: &str, name: &str) -> Result<Application> {
        self.call(
            reqwest::Method::POST,
            format!("/applications/{}/repository", encode(id)),
            Some(serde_json::json!({ "name": name })),
            false,
        )
        .await
    }

    /// Turns on building and deploying on every push to the branch.
    pub async fn enable_auto_deploy(&self, id: &str) -> Result<Application> {
        self.call(
            reqwest::Method::PUT,
            format!("/applications/{}/auto-deploy", encode(id)),
            None,
            false,
        )
        .await
    }

    pub async fn disable_auto_deploy(&self, id: &str) -> Result<Application> {
        self.call(
            reqwest::Method::DELETE,
            format!("/applications/{}/auto-deploy", encode(id)),
            None,
            false,
        )
        .await
    }

    /// Newest first.
    pub async fn actions(
        &self,
        options: WebBuilderActionListOptions,
    ) -> Result<WebBuilderActionList> {
        let mut query = Vec::new();
        if let Some(limit) = options.limit {
            query.push(("limit", limit.to_string()));
        }
        if let Some(cursor) = options.cursor {
            query.push(("cursor", cursor));
        }
        if let Some(application_id) = options.application_id {
            query.push(("applicationId", application_id));
        }
        self.http
            .send(Call {
                method: reqwest::Method::GET,
                path: "/actions".to_owned(),
                headers: self.headers.clone(),
                query,
                body: None,
                retry: true,
            })
            .await
    }

    /// The address to send the person to. GitHub returns to `redirect_uri` with `code` and `state`.
    pub async fn authorize_github(&self, redirect_uri: &str) -> Result<String> {
        let authorization: GithubAuthorization = self
            .call(
                reqwest::Method::POST,
                "/github/authorize".to_owned(),
                Some(serde_json::json!({ "redirectUri": redirect_uri })),
                false,
            )
            .await?;
        Ok(authorization.authorize_url)
    }

    /// Returns the GitHub login.
    pub async fn connect_github(&self, code: &str, state: &str) -> Result<String> {
        let connected: GithubLogin = self
            .call(
                reqwest::Method::POST,
                "/github/connect".to_owned(),
                Some(serde_json::json!({ "code": code, "state": state })),
                false,
            )
            .await?;
        Ok(connected.login)
    }

    /// The 100 most recently pushed repositories.
    pub async fn github_repositories(&self) -> Result<GithubRepositories> {
        self.call(
            reqwest::Method::GET,
            "/github/repositories".to_owned(),
            None,
            true,
        )
        .await
    }

    /// Removes this service's access. Deployed applications keep running.
    pub async fn disconnect_github(&self) -> Result<()> {
        self.call(reqwest::Method::DELETE, "/github".to_owned(), None, false)
            .await
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
