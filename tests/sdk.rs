use cognicloud::{
    Client, CreateEndpoint, DailyUsageRange, EndpointClient, EndpointStatus, Error, FinishReason,
    InvokeResponse, Locale, Options, UpdateEndpoint,
};
use serde_json::{json, Value};
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn answer() -> Value {
    json!({
        "id": "0c4e0e4a-0000-0000-0000-000000000000",
        "model": "gpt-5-mini",
        "output": "Hello",
        "finishReason": "stop",
        "usage": { "inputTokens": 10, "outputTokens": 3, "totalTokens": 13 }
    })
}

fn endpoint() -> Value {
    json!({
        "id": "e1",
        "name": "Support bot",
        "providerId": "azure-openai",
        "model": "gpt-5-mini",
        "url": "https://api.cognicloud.dev/v1/invoke/e1",
        "credentialPrefix": "cc_live_1234",
        "tokenLimit": 1000,
        "tokensUsed": 0,
        "status": "active",
        "requestMapping": [],
        "createdAt": "2026-10-20T09:00:00Z"
    })
}

fn options(server: &MockServer) -> Options {
    Options {
        base_url: server.uri(),
        ..Options::default()
    }
}

#[tokio::test]
async fn invokes_the_endpoint_with_its_credential_and_returns_the_canonical_answer() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/invoke/e1"))
        .and(header("authorization", "Bearer cc_live_secret"))
        .and(body_json(json!({ "input": "Hi" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(answer()))
        .expect(1)
        .mount(&server)
        .await;

    let client = EndpointClient::with_options("e1", "cc_live_secret", &options(&server)).unwrap();
    let result: InvokeResponse = client.invoke(&json!({ "input": "Hi" })).await.unwrap();
    assert_eq!(result.output, "Hello");
    assert_eq!(result.finish_reason, Some(FinishReason::Stop));
    assert_eq!(result.usage.total_tokens, 13);
}

#[tokio::test]
async fn never_retries_an_invoke() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/invoke/e1"))
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_json(json!({ "error": "gateway_unavailable", "message": "x" })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = EndpointClient::with_options(
        "e1",
        "k",
        &Options {
            max_retries: 3,
            ..options(&server)
        },
    )
    .unwrap();
    let error = client
        .invoke::<_, Value>(&json!({ "input": "Hi" }))
        .await
        .unwrap_err();
    assert_eq!((error.status(), error.code()), (503, "gateway_unavailable"));
}

#[tokio::test]
async fn reports_a_refusal_with_its_stable_code() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/invoke/e1"))
        .respond_with(ResponseTemplate::new(409).set_body_json(
            json!({ "error": "endpoint_paused", "message": "The endpoint is paused." }),
        ))
        .mount(&server)
        .await;

    let client = EndpointClient::with_options("e1", "k", &options(&server)).unwrap();
    let error = client.invoke::<_, Value>(&json!({})).await.unwrap_err();
    assert!(matches!(&error, Error::Api { status: 409, .. }));
    assert_eq!(error.code(), "endpoint_paused");
    assert_eq!(
        error.to_string(),
        "CogniCloud error 409 endpoint_paused: The endpoint is paused."
    );
}

#[tokio::test]
async fn creates_an_endpoint_with_the_session_token_and_returns_its_one_time_credential() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/endpoints"))
        .and(header("x-amadeus-token", "amadeus-session"))
        .and(header("accept-language", "pl"))
        .and(body_json(json!({ "name": "Support bot", "providerId": "azure-openai", "model": "gpt-5-mini", "tokenLimit": 1000 })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "endpoint": endpoint(), "credential": { "secret": "cc_live_new" } })))
        .expect(1)
        .mount(&server)
        .await;

    let cc = Client::builder()
        .token("amadeus-session")
        .locale(Locale::Pl)
        .base_url(server.uri())
        .build()
        .unwrap();
    let created = cc
        .endpoints()
        .create(CreateEndpoint {
            name: "Support bot".into(),
            provider_id: "azure-openai".into(),
            model: "gpt-5-mini".into(),
            token_limit: 1000,
        })
        .await
        .unwrap();
    assert_eq!(created.endpoint.status, EndpointStatus::Active);
    assert_eq!(created.credential.secret.expose(), "cc_live_new");
    assert!(!format!("{:?}", created.credential.secret).contains("cc_live_new"));
}

#[tokio::test]
async fn updates_only_the_fields_that_are_set() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/endpoints/e1"))
        .and(body_json(json!({ "tokenLimit": 5000 })))
        .respond_with(ResponseTemplate::new(200).set_body_json(endpoint()))
        .expect(1)
        .mount(&server)
        .await;

    let cc = Client::builder()
        .token("t")
        .base_url(server.uri())
        .build()
        .unwrap();
    cc.endpoints()
        .update(
            "e1",
            UpdateEndpoint {
                token_limit: Some(5000),
                ..Default::default()
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn reads_daily_usage_with_an_optional_range() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/endpoints/e1/usage/daily"))
        .and(query_param("from", "2026-10-01"))
        .and(query_param("to", "2026-10-20"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "days": [{ "day": "2026-10-20", "requests": 2, "inputTokens": 10, "outputTokens": 3, "totalTokens": 13 }]
        })))
        .mount(&server)
        .await;

    let cc = Client::builder()
        .token("t")
        .base_url(server.uri())
        .build()
        .unwrap();
    let days = cc
        .endpoints()
        .daily_usage(
            "e1",
            DailyUsageRange {
                from: Some("2026-10-01".into()),
                to: Some("2026-10-20".into()),
            },
        )
        .await
        .unwrap();
    assert_eq!(days.len(), 1);
    assert_eq!(days[0].total_tokens, 13);
}

#[tokio::test]
async fn deletes_an_endpoint_without_expecting_a_body() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/endpoints/e1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let cc = Client::builder()
        .token("t")
        .base_url(server.uri())
        .build()
        .unwrap();
    cc.endpoints().delete("e1").await.unwrap();
}

#[tokio::test]
async fn retries_a_read_after_a_503() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/endpoints"))
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_json(json!({ "error": "unavailable", "message": "later" })),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/endpoints"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([endpoint()])))
        .mount(&server)
        .await;

    let cc = Client::builder()
        .token("t")
        .base_url(server.uri())
        .build()
        .unwrap();
    assert_eq!(cc.endpoints().list().await.unwrap().len(), 1);
}

#[tokio::test]
async fn reports_a_network_failure_with_status_zero() {
    let cc = Client::builder()
        .token("t")
        .base_url("http://127.0.0.1:1")
        .max_retries(0)
        .build()
        .unwrap();
    let error = cc.endpoints().get("e1").await.unwrap_err();
    assert_eq!((error.status(), error.code()), (0, "network_error"));
}

#[tokio::test]
async fn the_builder_requires_a_token() {
    let error = Client::builder().build().unwrap_err();
    assert!(matches!(error, Error::Config(_)));
}
