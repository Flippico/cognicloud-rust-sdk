use cognicloud_sdk::{EmbeddingsRequest, Error};
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
async fn sends_the_inputs_and_returns_vectors_in_input_order() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/invoke/636f676e69636c6f7564/e1/embeddings"))
        .and(header("authorization", "Bearer cc_live_secret"))
        .and(body_json(
            json!({"input": ["first", "second"], "dimensions": 256}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "0c4e0e4a-0000-0000-0000-000000000000",
            "model": "embed-small",
            "data": [{"index": 0, "embedding": [0.1, 0.2]}, {"index": 1, "embedding": [0.3, 0.4]}],
            "usage": {"inputTokens": 12, "outputTokens": 0, "totalTokens": 12}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let request = EmbeddingsRequest::texts(["first", "second"]).dimensions(256);
    let result = endpoint(&server).await.embeddings(&request).await.unwrap();
    assert_eq!(result.model, "embed-small");
    assert_eq!(result.data.len(), 2);
    assert_eq!(result.data[1].embedding, vec![0.3, 0.4]);
    assert_eq!(result.usage.total_tokens, 12);
}

#[tokio::test]
async fn a_single_text_is_sent_as_a_string() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/invoke/636f676e69636c6f7564/e1/embeddings"))
        .and(body_json(json!({"input": "one text"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "x",
            "model": "embed-small",
            "data": [{"index": 0, "embedding": [1.0]}],
            "usage": {"inputTokens": 3, "outputTokens": 0, "totalTokens": 3}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let result = endpoint(&server)
        .await
        .embeddings(&EmbeddingsRequest::text("one text"))
        .await
        .unwrap();
    assert_eq!(result.data.len(), 1);
}

#[tokio::test]
async fn reports_a_refused_input_as_an_api_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/invoke/636f676e69636c6f7564/e1/embeddings"))
        .respond_with(ResponseTemplate::new(422).set_body_json(json!({"error": "invalid_request", "message": "input must be text or an array of text."})))
        .mount(&server)
        .await;
    match endpoint(&server)
        .await
        .embeddings(&EmbeddingsRequest::text("x"))
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
