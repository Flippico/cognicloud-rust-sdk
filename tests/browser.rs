use cognicloud_sdk::{BrowserStatus, Client, CreateBrowserSession, Error};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn session() -> serde_json::Value {
    json!({
        "id": "b903c5d0",
        "status": "active",
        "cdpUrl": "wss://cognicloud.flippi.co/api/v1/browser/sessions/b903c5d0/cdp?token=t",
        "liveUrl": "https://cognicloud.flippi.co/api/v1/browser/sessions/b903c5d0/live?token=t",
        "profileId": null,
        "proxyCountryCode": null,
        "solveCaptchas": false,
        "recording": false,
        "screen": null,
        "timeoutAt": "2026-10-21T10:15:00Z",
        "startedAt": "2026-10-21T10:00:00Z",
        "finishedAt": null
    })
}

async fn client(server: &MockServer) -> Client {
    Client::builder()
        .token("amadeus-token")
        .tenant("acme")
        .browser_base_url(format!("{}/v1/browser", server.uri()))
        .build()
        .unwrap()
}

#[tokio::test]
async fn starts_lists_reads_and_stops_sessions_at_the_browser_base() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/browser/sessions"))
        .and(header("authorization", "Bearer amadeus-token"))
        .and(header("tenant_id", "acme"))
        .and(body_json(json!({ "timeoutMinutes": 15 })))
        .respond_with(ResponseTemplate::new(201).set_body_json(session()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/browser/sessions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [session()],
            "concurrency": { "active": 1, "limit": 2 }
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/browser/sessions/b903c5d0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(session()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/browser/sessions/b903c5d0/stop"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "b903c5d0",
            "status": "stopped",
            "cdpUrl": null,
            "liveUrl": null,
            "profileId": null,
            "proxyCountryCode": null,
            "solveCaptchas": false,
            "recording": false,
            "screen": null,
            "timeoutAt": "2026-10-21T10:15:00Z",
            "startedAt": "2026-10-21T10:00:00Z",
            "finishedAt": "2026-10-21T10:05:00Z"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server).await;
    let browser = client.browser();
    let created = browser
        .sessions()
        .create(&CreateBrowserSession {
            timeout_minutes: Some(15),
        })
        .await
        .unwrap();
    assert_eq!(created.status, BrowserStatus::Active);
    assert!(created.cdp_url.is_some());
    let listed = browser.sessions().list().await.unwrap();
    assert_eq!(listed.concurrency.limit, 2);
    assert_eq!(
        browser.sessions().get("b903c5d0").await.unwrap().id,
        "b903c5d0"
    );
    let stopped = browser.sessions().stop("b903c5d0").await.unwrap();
    assert_eq!(stopped.status, BrowserStatus::Stopped);
    assert_eq!(stopped.cdp_url, None);
}

#[tokio::test]
async fn returns_the_service_error_code_when_a_session_cannot_start() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/browser/sessions"))
        .respond_with(ResponseTemplate::new(429).set_body_json(json!({
            "error": "concurrency_limit",
            "message": "Stop a session before starting another."
        })))
        .mount(&server)
        .await;
    let error = client(&server)
        .await
        .browser()
        .sessions()
        .create(&CreateBrowserSession::default())
        .await
        .unwrap_err();
    match error {
        Error::Api { status, code, .. } => {
            assert_eq!(status, 429);
            assert_eq!(code, "concurrency_limit");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn lists_countries_and_profiles_as_empty_until_they_exist() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/browser/countries"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/browser/profiles"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;
    let client = client(&server).await;
    assert!(client.browser().countries().await.unwrap().is_empty());
    assert!(client.browser().profiles().await.unwrap().is_empty());
}
