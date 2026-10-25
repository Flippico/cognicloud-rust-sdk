use std::fmt;

use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

use crate::endpoints::{FinishReason, InvokeUsage};
use crate::secret::Secret;

/// A Responses call: text or messages in `input`, optional MCP tools, and an earlier response to continue.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ResponsesRequest {
    /// A string, or an array of `{ role, content }` messages.
    pub input: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// The `response_id` of an earlier call on the same endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<McpTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    /// Any other field, sent to the provider unchanged. The gateway refuses the fields it sets itself, such as `n`,
    /// `store` and `background`.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

impl ResponsesRequest {
    /// Adds a field the typed fields do not cover, such as `seed` or `reasoning`.
    pub fn extra(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.extra.insert(key.into(), value.into());
        self
    }

    /// A request with plain text as its input.
    pub fn text(input: impl Into<String>) -> Self {
        Self {
            input: Value::String(input.into()),
            ..Self::default()
        }
    }
}

/// An MCP server a Responses call may use, called by its label. Name the server's `url` and its bearer token with
/// [`McpTool::server`]; the gateway checks them, sets approval to `never`, and passes them to the provider. Without
/// `server` the tool names only the label, which the gateway must have configured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct McpTool {
    #[serde(rename = "type")]
    kind: McpKind,
    pub server_label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<McpHeaders>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum McpKind {
    Mcp,
}

/// The `Authorization` header of an MCP tool: one bearer token. It is never shown by `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct McpHeaders {
    authorization: Secret<String>,
}

impl fmt::Debug for McpHeaders {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("McpHeaders { Authorization: <redacted> }")
    }
}

impl Serialize for McpHeaders {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(
            "Authorization",
            &format!("Bearer {}", self.authorization.expose()),
        )?;
        map.end()
    }
}

impl McpTool {
    pub fn new(server_label: impl Into<String>) -> Self {
        Self {
            kind: McpKind::Mcp,
            server_label: server_label.into(),
            server_url: None,
            headers: None,
            allowed_tools: None,
        }
    }

    /// Names the server's address and its bearer token. The address must be https, without credentials or a fragment.
    pub fn server(mut self, url: impl Into<String>, bearer_token: impl Into<String>) -> Self {
        self.server_url = Some(url.into());
        self.headers = Some(McpHeaders {
            authorization: Secret::new(bearer_token.into()),
        });
        self
    }

    /// Limits the tools the model may call on this server.
    pub fn allowed_tools(mut self, names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.allowed_tools = Some(names.into_iter().map(Into::into).collect());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolChoice {
    Auto,
    Required,
    None,
}

/// The answer to a Responses call. `response_id` continues the conversation through `previous_response_id`, and
/// `output_items` holds the tool calls, tool lists and messages as the provider returned them.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseResult {
    pub id: String,
    pub model: String,
    pub output: String,
    pub finish_reason: Option<FinishReason>,
    pub usage: InvokeUsage,
    pub response_id: Option<String>,
    pub output_items: Option<Vec<Value>>,
}
