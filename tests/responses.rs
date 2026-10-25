use cognicloud_sdk::{Error, McpTool, ResponsesRequest, ToolChoice};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn endpoint(server: &MockServer) -> cognicloud_sdk::EndpointClient {
    cognicloud_sdk::EndpointClient::with_options(
        "e1",
        "cc_live_secret",
        &cognicloud_sdk::Options {
            base_url: server.uri(),
            ..Default::default()
        },
    )
    .unwrap()
}

#[tokio::test]
async fn sends_mcp_tools_and_returns_the_response_id_and_output_items() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/invoke/636f676e69636c6f7564/e1"))
        .and(header("authorization", "Bearer cc_live_secret"))
        .and(body_json(json!({
            "input": "Check this invoice.",
            "instructions": "Be brief.",
            "tools": [{"type": "mcp", "server_label": "ksef", "allowed_tools": ["validate_invoice_xml"]}],
            "tool_choice": "required"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "0c4e0e4a-0000-0000-0000-000000000000",
            "model": "responses",
            "output": "Valid.",
            "finishReason": "stop",
            "usage": {"inputTokens": 10, "outputTokens": 3, "totalTokens": 13},
            "responseId": "resp_1",
            "outputItems": [{"type": "mcp_call", "name": "validate_invoice_xml"}]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let request = ResponsesRequest {
        instructions: Some("Be brief.".into()),
        tools: vec![McpTool::new("ksef").allowed_tools(["validate_invoice_xml"])],
        tool_choice: Some(ToolChoice::Required),
        ..ResponsesRequest::text("Check this invoice.")
    };
    let answer = endpoint(&server).await.responses(&request).await.unwrap();
    assert_eq!(answer.output, "Valid.");
    assert_eq!(answer.response_id.as_deref(), Some("resp_1"));
    assert_eq!(answer.output_items.unwrap().len(), 1);
}

#[tokio::test]
async fn continues_a_conversation_with_the_previous_response_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/invoke/636f676e69636c6f7564/e1"))
        .and(body_json(
            json!({"input": "And this one?", "previous_response_id": "resp_1"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "0c4e0e4a-0000-0000-0000-000000000001",
            "model": "responses",
            "output": "Also valid.",
            "finishReason": "stop",
            "usage": {"inputTokens": 5, "outputTokens": 2, "totalTokens": 7},
            "responseId": "resp_2"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let request = ResponsesRequest {
        previous_response_id: Some("resp_1".into()),
        ..ResponsesRequest::text("And this one?")
    };
    let answer = endpoint(&server).await.responses(&request).await.unwrap();
    assert_eq!(answer.response_id.as_deref(), Some("resp_2"));
    assert_eq!(answer.output_items, None);
}

#[tokio::test]
async fn reports_a_refused_tool_as_an_api_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/invoke/636f676e69636c6f7564/e1"))
        .respond_with(ResponseTemplate::new(422).set_body_json(
            json!({"error": "invalid_request", "message": "Unknown MCP server other."}),
        ))
        .mount(&server)
        .await;
    let request = ResponsesRequest {
        tools: vec![McpTool::new("other")],
        ..ResponsesRequest::text("x")
    };
    match endpoint(&server)
        .await
        .responses(&request)
        .await
        .unwrap_err()
    {
        Error::Api { status, code, .. } => {
            assert_eq!(status, 422);
            assert_eq!(code, "invalid_request");
        }
        other => panic!("unexpected error {other:?}"),
    }
}

#[test]
fn a_caller_server_serializes_its_url_and_bearer_header_and_hides_the_token_in_debug() {
    let tool = McpTool::new("ksef")
        .server("https://ksef-api.flippi.co/mcp/v1", "abc123")
        .allowed_tools(["validate_invoice_xml"]);
    let wire = serde_json::to_value(&tool).unwrap();
    assert_eq!(
        wire,
        json!({
            "type": "mcp",
            "server_label": "ksef",
            "server_url": "https://ksef-api.flippi.co/mcp/v1",
            "headers": {"Authorization": "Bearer abc123"},
            "allowed_tools": ["validate_invoice_xml"],
        })
    );
    let debug = format!("{tool:?}");
    assert!(!debug.contains("abc123"), "{debug}");
}

#[test]
fn a_label_only_tool_sends_no_address_or_header() {
    let wire = serde_json::to_value(McpTool::new("ksef")).unwrap();
    assert_eq!(wire, json!({"type": "mcp", "server_label": "ksef"}));
}
