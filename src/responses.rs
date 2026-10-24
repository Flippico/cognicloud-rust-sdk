use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::endpoints::{FinishReason, InvokeUsage};

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
}

impl ResponsesRequest {
    /// A request with plain text as its input.
    pub fn text(input: impl Into<String>) -> Self {
        Self {
            input: Value::String(input.into()),
            ..Self::default()
        }
    }
}

/// An MCP server the platform has configured, called by its label. The gateway supplies the server's address and
/// credentials, and never asks for approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct McpTool {
    #[serde(rename = "type")]
    kind: McpKind,
    pub server_label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum McpKind {
    Mcp,
}

impl McpTool {
    pub fn new(server_label: impl Into<String>) -> Self {
        Self {
            kind: McpKind::Mcp,
            server_label: server_label.into(),
            allowed_tools: None,
        }
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
