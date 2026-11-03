use serde::Deserialize;

use crate::agents::{ActionLogList, ActionLogListOptions, Service};
use crate::client::encode;
use crate::error::Result;
use crate::http::Http;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TemplatePublisher {
    pub name: String,
    /// `cognicloud` for CogniCloud's own templates.
    #[serde(default)]
    pub workspace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TemplateSkill {
    pub name: String,
    pub description: String,
    pub body: String,
}

/// An agent to start from. Create one with its `slug` as [`crate::CreateAgent::template_slug`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub slug: String,
    pub kind: String,
    pub version: u32,
    pub language: Option<String>,
    pub name: String,
    pub summary: String,
    pub description: String,
    /// `research`, `monitoring`, `operations` or `data`.
    pub category: String,
    pub publisher: TemplatePublisher,
    pub tools: Vec<String>,
    pub instructions: String,
    pub max_steps: u32,
    pub timeout_seconds: u32,
    pub skills: Vec<TemplateSkill>,
}

/// Agent templates to start from.
pub struct MarketplaceApi<'a> {
    service: Service<'a>,
}

impl<'a> MarketplaceApi<'a> {
    pub(crate) fn new(http: &'a Http, token: &str, tenant: Option<&str>) -> Self {
        Self {
            service: Service::new(http, token, tenant),
        }
    }

    /// All templates, or those in one `category`.
    pub async fn list_templates(&self, category: Option<&str>) -> Result<Vec<Template>> {
        let query = category
            .map(|category| vec![("category", category.to_owned())])
            .unwrap_or_default();
        self.service
            .call(
                reqwest::Method::GET,
                "/templates".to_owned(),
                query,
                None,
                true,
            )
            .await
    }

    pub async fn get_template(&self, slug: &str) -> Result<Template> {
        self.service
            .get(format!("/templates/{}", encode(slug)))
            .await
    }

    /// The marketplace's action log, newest first.
    pub async fn actions(&self, options: ActionLogListOptions) -> Result<ActionLogList> {
        self.service.actions(options).await
    }
}
