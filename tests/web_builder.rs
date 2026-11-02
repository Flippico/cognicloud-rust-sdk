use cognicloud_sdk::{Client, CreateApplication, UpdateApplication, WebBuilderActionListOptions};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path, query_param};
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
async fn controls_an_application() {
    let server = MockServer::start().await;
    let one = format!("/v1/web-builder/applications/{ID}");
    Mock::given(method("PATCH"))
        .and(path(one.as_str()))
        .and(body_json(json!({ "branch": "next" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(application()))
        .expect(1)
        .mount(&server)
        .await;
    for action in ["pause", "resume"] {
        Mock::given(method("POST"))
            .and(path(format!("{one}/{action}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(application()))
            .expect(1)
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path(format!("{one}/runtime-logs")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "logs": "GET /" })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("{one}/repository")))
        .and(body_json(json!({ "name": "shop" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(application()))
        .expect(1)
        .mount(&server)
        .await;
    let mut deployed = application();
    deployed["autoDeploy"] = json!(true);
    Mock::given(method("PUT"))
        .and(path(format!("{one}/auto-deploy")))
        .respond_with(ResponseTemplate::new(200).set_body_json(deployed))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path(format!("{one}/auto-deploy")))
        .respond_with(ResponseTemplate::new(200).set_body_json(application()))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let api = client.web_builder();
    api.update(
        ID,
        &UpdateApplication {
            branch: Some("next".to_owned()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    api.pause(ID).await.unwrap();
    api.resume(ID).await.unwrap();
    assert_eq!(api.runtime_logs(ID).await.unwrap(), "GET /");
    api.create_repository(ID, "shop").await.unwrap();
    assert!(api.enable_auto_deploy(ID).await.unwrap().auto_deploy);
    assert!(!api.disable_auto_deploy(ID).await.unwrap().auto_deploy);
}

#[tokio::test]
async fn lists_actions_and_uses_github() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/web-builder/actions"))
        .and(query_param("applicationId", ID))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{
                "id": "a1",
                "action": "build_from_push",
                "applicationId": ID,
                "name": "shop",
                "startedAt": "2026-11-02T10:00:00Z",
                "finishedAt": null,
                "durationMs": null,
                "outcome": "succeeded",
                "errorCode": null
            }],
            "nextCursor": null
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/web-builder/github/authorize"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "authorizeUrl": "https://github.com/login" })),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/web-builder/github/connect"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "login": "acme" })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/web-builder/github/repositories"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "login": "acme",
            "items": [{
                "fullName": "acme/shop",
                "url": "https://github.com/acme/shop",
                "defaultBranch": "main",
                "private": false
            }]
        })))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/web-builder/github"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let api = client.web_builder();
    let actions = api
        .actions(WebBuilderActionListOptions {
            application_id: Some(ID.to_owned()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(actions.items[0].action, "build_from_push");
    assert_eq!(
        api.authorize_github("https://app.test/back").await.unwrap(),
        "https://github.com/login"
    );
    assert_eq!(api.connect_github("code", "state").await.unwrap(), "acme");
    assert_eq!(
        api.github_repositories().await.unwrap().items[0].default_branch,
        "main"
    );
    api.disconnect_github().await.unwrap();
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
