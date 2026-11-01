use cognicloud_sdk::{Client, CreateApplication};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ID: &str = "7c1f0c2e-6a5b-4d3e-9f10-1a2b3c4d5e6f";

fn application() -> serde_json::Value {
    json!({
        "id": ID,
        "subdomain": "shop",
        "url": "https://shop.cognicloud.dev",
        "status": "building",
        "repoUrl": null,
        "branch": null,
        "buildCommand": "npm run build",
        "outputDir": "dist",
        "createdAt": "2026-11-01T10:00:00Z"
    })
}

fn client(server: &MockServer) -> Client {
    Client::builder()
        .token("amadeus-token")
        .tenant("acme")
        .web_builder_base_url(format!("{}/v1/web-builder", server.uri()))
        .build()
        .unwrap()
}

#[tokio::test]
async fn creates_an_application_with_the_session_and_tenant() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/web-builder/applications"))
        .and(header("authorization", "Bearer amadeus-token"))
        .and(header("tenant_id", "acme"))
        .and(body_json(json!({ "subdomain": "shop" })))
        .respond_with(ResponseTemplate::new(201).set_body_json(application()))
        .expect(1)
        .mount(&server)
        .await;

    let created = client(&server)
        .web_builder()
        .create(&CreateApplication {
            subdomain: "shop".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(created.url, "https://shop.cognicloud.dev");
    assert_eq!(created.repo_url, None);
}

#[tokio::test]
async fn lists_reads_redeploys_reads_logs_and_deletes() {
    let server = MockServer::start().await;
    let base = "/v1/web-builder/applications";
    let one = format!("{base}/{ID}");
    Mock::given(method("GET"))
        .and(path(base))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "items": [application()] })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(one.as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(application()))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("{one}/redeploy")))
        .respond_with(ResponseTemplate::new(200).set_body_json(application()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("{one}/logs")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "logs": "built" })))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path(one.as_str()))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let api = client.web_builder();
    assert_eq!(api.list().await.unwrap()[0].id, ID);
    assert_eq!(api.get(ID).await.unwrap().subdomain, "shop");
    assert_eq!(api.redeploy(ID).await.unwrap().status, "building");
    assert_eq!(api.logs(ID).await.unwrap(), "built");
    api.delete(ID).await.unwrap();
}

#[tokio::test]
async fn a_create_is_not_retried() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/web-builder/applications"))
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_json(json!({ "error": "unavailable", "message": "Hosting is down." })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let error = client(&server)
        .web_builder()
        .create(&CreateApplication {
            subdomain: "shop".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("Hosting is down."));
}
