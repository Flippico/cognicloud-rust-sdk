use std::time::Duration;

use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::client::encode;
use crate::error::{Error, Result};
use crate::http::{Call, Http};
use crate::secret::Secret;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Active,
    Paused,
    /// A status this SDK version does not know yet. Never sent: serializing it fails.
    #[serde(other, skip_serializing)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub description: String,
    /// The AI Gateway endpoint that runs the agent.
    pub endpoint_id: String,
    pub instructions: String,
    pub tools: Vec<String>,
    pub database_id: Option<String>,
    pub max_steps: u32,
    pub timeout_seconds: u32,
    pub status: AgentStatus,
    pub invoke_url: String,
    /// The first characters of the agent's `ccag_` credential, to tell credentials apart.
    pub credential_prefix: String,
    pub template_slug: Option<String>,
    pub skill_ids: Vec<String>,
    /// `private` or `public`.
    pub visibility: String,
    pub listing_slug: Option<String>,
    pub published_version: Option<u32>,
    pub has_unpublished_changes: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgent {
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub endpoint_id: String,
    pub instructions: String,
    pub tools: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_id: Option<String>,
    /// 1 to 100.
    pub max_steps: u32,
    /// 10 to 900.
    pub timeout_seconds: u32,
    pub skill_ids: Vec<String>,
    /// The marketplace template the agent starts from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_slug: Option<String>,
}

/// A field left out stays as it is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    /// `Some(None)` clears the database.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_id: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_steps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_ids: Option<Vec<String>>,
    /// Pause or resume the agent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<AgentStatus>,
}

/// The agent's `ccag_` credential. Shown once; store it in your secret manager.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AgentCredential {
    pub secret: Secret<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CreatedAgent {
    pub agent: Agent,
    pub credential: AgentCredential,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishAgent {
    pub category: String,
    pub publisher_name: String,
    pub language: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub id: String,
    pub name: String,
    /// When the agent should use the skill.
    pub description: String,
    pub body: String,
    pub version: u32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CreateSkill {
    pub name: String,
    pub description: String,
    pub body: String,
}

/// A change adds a version and keeps the earlier one. A field left out stays as it is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct UpdateSkill {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillVersion {
    pub version: u32,
    pub name: String,
    pub description: String,
    pub body: String,
    pub created_at: String,
}

/// A `cga_` key. The full key is shown only when it is created in the dashboard.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentKey {
    pub id: String,
    pub name: String,
    pub prefix: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionLog {
    pub id: String,
    pub action: String,
    pub subject_id: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub outcome: String,
    pub error_code: Option<String>,
}

/// Pass `next_cursor` as `cursor` for the next page. It is `None` on the last page.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionLogList {
    pub items: Vec<ActionLog>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActionLogListOptions {
    /// Default 50, at most 200.
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
    Stopped,
    /// A status this SDK version does not know yet.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunErrorCode {
    MaxSteps,
    Timeout,
    ModelError,
    InvalidAnswer,
    /// The service stopped while the run was going.
    Interrupted,
    Stopped,
    /// A code this SDK version does not know yet.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RunStepDetail {
    pub number: u32,
    pub thought: String,
    /// `None` for the final answer.
    pub tool: Option<String>,
    /// A short description of the tool call. Tool results are not stored.
    pub summary: String,
    pub error: Option<String>,
}

/// A run of an agent from the agents service. Browser session runs are [`crate::AgentRun`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub agent_id: String,
    pub status: RunStatus,
    pub input: String,
    /// The final answer.
    pub output: Option<String>,
    pub steps: Vec<RunStepDetail>,
    pub error_code: Option<RunErrorCode>,
    pub error_message: Option<String>,
    pub max_steps: u32,
    pub timeout_seconds: u32,
    pub created_at: String,
    pub finished_at: Option<String>,
}

/// Pass `next_cursor` as `cursor` for the next page. It is `None` on the last page.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunList {
    pub items: Vec<Run>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunListOptions {
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

/// How [`RunsApi::run_and_wait`] polls. Default: every 2 seconds for up to 15 minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunWaitOptions {
    pub poll_interval: Duration,
    pub timeout: Duration,
}

impl Default for RunWaitOptions {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(2),
            timeout: Duration::from_secs(900),
        }
    }
}

pub(crate) struct Service<'a> {
    http: &'a Http,
    headers: Vec<(&'static str, String)>,
}

impl<'a> Service<'a> {
    /// `tenant` is sent as the `tenant_id` header when given. A `ccag_` credential names its workspace, so it needs
    /// none.
    pub(crate) fn new(http: &'a Http, token: &str, tenant: Option<&str>) -> Self {
        let mut headers = vec![("authorization", format!("Bearer {token}"))];
        if let Some(tenant) = tenant {
            headers.push(("tenant_id", tenant.to_owned()));
        }
        Self { http, headers }
    }

    pub(crate) async fn call<T: DeserializeOwned>(
        &self,
        method: Method,
        path: String,
        query: Vec<(&'static str, String)>,
        body: Option<serde_json::Value>,
        retry: bool,
    ) -> Result<T> {
        self.http
            .send(Call {
                method,
                path,
                headers: self.headers.clone(),
                query,
                body,
                retry,
            })
            .await
    }

    pub(crate) async fn get<T: DeserializeOwned>(&self, path: String) -> Result<T> {
        self.call(Method::GET, path, Vec::new(), None, true).await
    }

    pub(crate) async fn send<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        method: Method,
        path: String,
        body: Option<&B>,
    ) -> Result<T> {
        let body = body
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| Error::Config(error.to_string()))?;
        self.call(method, path, Vec::new(), body, false).await
    }

    pub(crate) async fn actions(&self, options: ActionLogListOptions) -> Result<ActionLogList> {
        self.call(
            Method::GET,
            "/actions".to_owned(),
            page_query(options.limit, options.cursor),
            None,
            true,
        )
        .await
    }
}

fn page_query(limit: Option<u32>, cursor: Option<String>) -> Vec<(&'static str, String)> {
    let mut query = Vec::new();
    if let Some(limit) = limit {
        query.push(("limit", limit.to_string()));
    }
    if let Some(cursor) = cursor {
        query.push(("cursor", cursor));
    }
    query
}

/// Manages agents.
pub struct AgentsApi<'a> {
    service: Service<'a>,
}

impl<'a> AgentsApi<'a> {
    pub(crate) fn new(http: &'a Http, token: &str, tenant: Option<&str>) -> Self {
        Self {
            service: Service::new(http, token, tenant),
        }
    }

    pub async fn list(&self) -> Result<Vec<Agent>> {
        self.service.get("/agents".to_owned()).await
    }

    pub async fn get(&self, id: &str) -> Result<Agent> {
        self.service.get(format!("/agents/{}", encode(id))).await
    }

    /// The credential is returned only here and on [`rotate_credential`](Self::rotate_credential).
    pub async fn create(&self, input: &CreateAgent) -> Result<CreatedAgent> {
        self.service
            .send(Method::POST, "/agents".to_owned(), Some(input))
            .await
    }

    pub async fn update(&self, id: &str, changes: &UpdateAgent) -> Result<Agent> {
        self.service
            .send(
                Method::PATCH,
                format!("/agents/{}", encode(id)),
                Some(changes),
            )
            .await
    }

    /// Deletes the agent with its runs.
    pub async fn delete(&self, id: &str) -> Result<()> {
        self.service
            .send::<_, ()>(Method::DELETE, format!("/agents/{}", encode(id)), None)
            .await
    }

    /// The earlier credential stops working at once.
    pub async fn rotate_credential(&self, id: &str) -> Result<AgentCredential> {
        self.service
            .send::<_, ()>(
                Method::POST,
                format!("/agents/{}/credential/rotate", encode(id)),
                None,
            )
            .await
    }

    /// Planned: answers `503` until publishing is available.
    pub async fn publish(&self, id: &str, input: &PublishAgent) -> Result<Agent> {
        self.service
            .send(
                Method::POST,
                format!("/agents/{}/publish", encode(id)),
                Some(input),
            )
            .await
    }

    /// Planned: answers `503` until publishing is available.
    pub async fn unpublish(&self, id: &str) -> Result<Agent> {
        self.service
            .send::<_, ()>(
                Method::POST,
                format!("/agents/{}/unpublish", encode(id)),
                None,
            )
            .await
    }

    /// `cga_` keys. Managing keys needs the person's own sign-in, not a key.
    pub fn keys(&self) -> AgentKeysApi<'_> {
        AgentKeysApi {
            service: &self.service,
        }
    }

    /// The agents service's action log, newest first.
    pub async fn actions(&self, options: ActionLogListOptions) -> Result<ActionLogList> {
        self.service.actions(options).await
    }
}

pub struct AgentKeysApi<'a> {
    service: &'a Service<'a>,
}

impl AgentKeysApi<'_> {
    pub async fn list(&self) -> Result<Vec<AgentKey>> {
        self.service.get("/keys".to_owned()).await
    }

    pub async fn revoke(&self, id: &str) -> Result<()> {
        self.service
            .send::<_, ()>(Method::DELETE, format!("/keys/{}", encode(id)), None)
            .await
    }
}

/// Skills give agents instructions for particular tasks. Every change is kept as a version.
pub struct SkillsApi<'a> {
    service: Service<'a>,
}

impl<'a> SkillsApi<'a> {
    pub(crate) fn new(http: &'a Http, token: &str, tenant: Option<&str>) -> Self {
        Self {
            service: Service::new(http, token, tenant),
        }
    }

    pub async fn list(&self) -> Result<Vec<Skill>> {
        self.service.get("/skills".to_owned()).await
    }

    pub async fn get(&self, id: &str) -> Result<Skill> {
        self.service.get(format!("/skills/{}", encode(id))).await
    }

    pub async fn create(&self, input: &CreateSkill) -> Result<Skill> {
        self.service
            .send(Method::POST, "/skills".to_owned(), Some(input))
            .await
    }

    pub async fn update(&self, id: &str, changes: &UpdateSkill) -> Result<Skill> {
        self.service
            .send(
                Method::PATCH,
                format!("/skills/{}", encode(id)),
                Some(changes),
            )
            .await
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        self.service
            .send::<_, ()>(Method::DELETE, format!("/skills/{}", encode(id)), None)
            .await
    }

    /// The skill's earlier versions.
    pub async fn versions(&self, id: &str) -> Result<Vec<SkillVersion>> {
        self.service
            .get(format!("/skills/{}/versions", encode(id)))
            .await
    }
}

/// Starts, reads and stops runs of agents. Works with the person's session, a `cga_` key, or the agent's own `ccag_`
/// credential, which reaches only that agent's runs.
pub struct RunsApi<'a> {
    service: Service<'a>,
}

impl<'a> RunsApi<'a> {
    pub(crate) fn new(http: &'a Http, token: &str, tenant: Option<&str>) -> Self {
        Self {
            service: Service::new(http, token, tenant),
        }
    }

    /// Starts a run and returns at once with status `running`. The input is 1 to 20,000 characters. Fails with
    /// `agent_paused` (409), `endpoint_missing` (422) or `limit_reached` (429, at most 3 running). Never retried.
    pub async fn start(&self, agent_id: &str, input: &str) -> Result<Run> {
        self.service
            .send(
                Method::POST,
                format!("/agents/{}/runs", encode(agent_id)),
                Some(&serde_json::json!({ "input": input })),
            )
            .await
    }

    pub async fn get(&self, run_id: &str) -> Result<Run> {
        self.service.get(format!("/runs/{}", encode(run_id))).await
    }

    /// The agent's runs, newest first.
    pub async fn list(&self, agent_id: &str, options: RunListOptions) -> Result<RunList> {
        self.service
            .call(
                Method::GET,
                format!("/agents/{}/runs", encode(agent_id)),
                page_query(options.limit, options.cursor),
                None,
                true,
            )
            .await
    }

    /// Stopping a finished run changes nothing.
    pub async fn stop(&self, run_id: &str) -> Result<Run> {
        self.service
            .send::<_, ()>(Method::POST, format!("/runs/{}/stop", encode(run_id)), None)
            .await
    }

    /// Starts a run and reads it every `options.poll_interval` until it is no longer `running`. Returns
    /// [`Error::Timeout`] when `options.timeout` passes first; the run keeps going. Pass
    /// [`RunWaitOptions::default()`] for 2 seconds and 15 minutes.
    pub async fn run_and_wait(
        &self,
        agent_id: &str,
        input: &str,
        options: RunWaitOptions,
    ) -> Result<Run> {
        let RunWaitOptions {
            poll_interval,
            timeout,
        } = options;
        let deadline = tokio::time::Instant::now() + timeout;
        let mut run = self.start(agent_id, input).await?;
        while run.status == RunStatus::Running {
            let now = tokio::time::Instant::now();
            if now >= deadline {
                return Err(Error::Timeout);
            }
            tokio::time::sleep(poll_interval.min(deadline - now)).await;
            run = self.get(&run.id).await?;
        }
        Ok(run)
    }
}
