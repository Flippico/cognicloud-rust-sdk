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
            ..Default::default()
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
async fn saves_a_session_as_a_profile_and_starts_from_it() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/browser/sessions/b903c5d0/profile"))
        .and(header("tenant_id", "acme"))
        .and(body_json(json!({ "name": "shop login" })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "5f1c9a2e",
            "name": "shop login",
            "sizeBytes": 20480,
            "createdAt": "2026-10-21T10:20:00Z",
            "lastUsedAt": null,
            "expiresAt": "2027-01-19T10:20:00Z"
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/browser/sessions"))
        .and(body_json(
            json!({ "timeoutMinutes": 15, "profileId": "5f1c9a2e" }),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(session()))
        .expect(1)
        .mount(&server)
        .await;
    let client = client(&server).await;

    let profile = client
        .browser()
        .sessions()
        .save_profile("b903c5d0", "shop login")
        .await
        .unwrap();
    assert_eq!(profile.id, "5f1c9a2e");
    assert_eq!(profile.size_bytes, 20480);
    assert_eq!(profile.expires_at, "2027-01-19T10:20:00Z");

    let options = CreateBrowserSession {
        timeout_minutes: Some(15),
        profile_id: Some(profile.id),
        ..Default::default()
    };
    client.browser().sessions().create(&options).await.unwrap();
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

#[tokio::test]
async fn a_session_takes_a_window_size_and_returns_it() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/browser/sessions"))
        .and(body_json(json!({ "timeoutMinutes": 15, "screen": { "width": 1280, "height": 720 } })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "b903c5d0", "status": "active",
            "cdpUrl": "wss://example.test/cdp", "liveUrl": "https://example.test/live",
            "profileId": null, "proxyCountryCode": null, "solveCaptchas": false, "recording": false,
            "screen": { "width": 1280, "height": 720 },
            "timeoutAt": "2026-10-21T10:15:00Z", "startedAt": "2026-10-21T10:00:00Z", "finishedAt": null
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = client(&server).await;
    let options = CreateBrowserSession {
        timeout_minutes: Some(15),
        screen: Some(cognicloud_sdk::BrowserScreen {
            width: 1280,
            height: 720,
        }),
        ..Default::default()
    };
    let session = client.browser().sessions().create(&options).await.unwrap();
    assert_eq!(
        session.screen.map(|screen| (screen.width, screen.height)),
        Some((1280, 720))
    );
}

#[tokio::test]
async fn a_browser_key_alone_sends_no_tenant_and_refuses_a_blank_key() {
    use cognicloud_sdk::BrowserClient;

    for key in ["", "   "] {
        assert!(matches!(BrowserClient::new(key), Err(Error::Config(_))));
    }

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/browser/profiles"))
        .and(header("authorization", "Bearer cgb_abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(1)
        .mount(&server)
        .await;
    let browser =
        BrowserClient::with_base_url("cgb_abc", format!("{}/v1/browser", server.uri())).unwrap();
    assert!(browser.browser().profiles().await.unwrap().is_empty());
    let received = server.received_requests().await.unwrap();
    assert!(
        received[0].headers.get("tenant_id").is_none(),
        "a browser key names its workspace"
    );
}

#[tokio::test]
async fn uploads_a_profile_archive_as_the_raw_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/browser/profiles/import"))
        .and(wiremock::matchers::query_param("name", "Shop account"))
        .and(header("content-type", "application/gzip"))
        .and(wiremock::matchers::body_bytes(vec![31u8, 139, 8]))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "5f1c9a2e", "name": "Shop account", "sizeBytes": 3,
            "createdAt": "2026-10-21T10:20:00Z", "lastUsedAt": null, "expiresAt": "2027-01-19T10:20:00Z"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = client(&server).await;
    let profile = client
        .browser()
        .import_profile("Shop account", vec![31, 139, 8])
        .await
        .unwrap();
    assert_eq!(profile.id, "5f1c9a2e");
}
