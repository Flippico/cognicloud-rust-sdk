use cognicloud_sdk::{Error, StreamEvent};
use serde_json::json;
use wiremock::matchers::{body_json, method, path};
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

fn sse(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(body.to_owned(), "text/event-stream")
}

const DONE: &str = r#"{"id":"c1","model":"gpt-5-mini","output":"Hel lo","finishReason":"stop","usage":{"inputTokens":4,"outputTokens":2,"totalTokens":6},"responseId":null,"outputItems":null}"#;

#[tokio::test]
async fn yields_deltas_in_order_then_the_done_answer_and_sends_stream_true() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/invoke/636f676e69636c6f7564/e1"))
        .and(body_json(json!({"messages": [{"role": "user", "content": "hi"}], "stream": true})))
        .respond_with(sse(&format!(
            "event: delta\ndata: {{\"text\":\"Hel\"}}\n\nevent: delta\r\ndata: {{\"text\":\"lo\"}}\r\n\r\nevent: done\ndata: {DONE}\n\n"
        )))
        .expect(1)
        .mount(&server)
        .await;

    let mut stream = endpoint(&server)
        .await
        .stream(&json!({"messages": [{"role": "user", "content": "hi"}]}))
        .await
        .unwrap();
    assert_eq!(
        stream.next().await.unwrap().unwrap(),
        StreamEvent::Delta("Hel".into())
    );
    assert_eq!(
        stream.next().await.unwrap().unwrap(),
        StreamEvent::Delta("lo".into())
    );
    let StreamEvent::Done(answer) = stream.next().await.unwrap().unwrap() else {
        panic!("expected done")
    };
    assert_eq!(answer.output, "Hel lo");
    assert_eq!(answer.usage.total_tokens, 6);
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn a_provider_error_event_is_yielded_and_ends_the_stream() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/invoke/636f676e69636c6f7564/e1"))
        .respond_with(sse("event: delta\ndata: {\"text\":\"Hi\"}\n\nevent: error\ndata: {\"code\":\"upstream_failure\",\"message\":\"The provider did not finish the answer.\"}\n\n"))
        .mount(&server)
        .await;

    let mut stream = endpoint(&server)
        .await
        .stream_responses(&cognicloud_sdk::ResponsesRequest::text("x"))
        .await
        .unwrap();
    stream.next().await.unwrap().unwrap();
    assert_eq!(
        stream.next().await.unwrap().unwrap(),
        StreamEvent::Error {
            code: "upstream_failure".into(),
            message: "The provider did not finish the answer.".into()
        }
    );
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn a_stream_that_closes_early_reports_stream_ended() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/invoke/636f676e69636c6f7564/e1"))
        .respond_with(sse("event: delta\ndata: {\"text\":\"Hi\"}\n\n"))
        .mount(&server)
        .await;

    let mut stream = endpoint(&server)
        .await
        .stream(&json!({"input": "x"}))
        .await
        .unwrap();
    stream.next().await.unwrap().unwrap();
    match stream.next().await.unwrap().unwrap() {
        StreamEvent::Error { code, .. } => assert_eq!(code, "stream_ended"),
        other => panic!("unexpected event {other:?}"),
    }
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn a_refused_stream_is_an_api_error_before_any_event() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/invoke/636f676e69636c6f7564/e1"))
        .respond_with(ResponseTemplate::new(422).set_body_json(json!({"error": "invalid_request", "message": "Streaming needs an API version of 2024-10-21 or later."})))
        .mount(&server)
        .await;

    match endpoint(&server).await.stream(&json!({"input": "x"})).await {
        Err(Error::Api { status, code, .. }) => {
            assert_eq!(status, 422);
            assert_eq!(code, "invalid_request");
        }
        Err(other) => panic!("unexpected error {other:?}"),
        Ok(_) => panic!("expected a refusal"),
    }
}

#[tokio::test]
async fn a_streamed_body_must_be_an_object() {
    let server = MockServer::start().await;
    let result = endpoint(&server)
        .await
        .stream(&json!(["not", "an", "object"]))
        .await;
    assert!(matches!(result, Err(Error::Config(_))));
}
