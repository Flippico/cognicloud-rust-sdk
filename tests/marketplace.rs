use cognicloud_sdk::{ActionLogListOptions, Client};
use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn template() -> serde_json::Value {
    json!({
        "slug": "web-researcher",
        "kind": "cognicloud",
        "version": 1,
        "language": "en",
        "name": "Web researcher",
        "summary": "Finds answers.",
        "description": "Finds answers on the web.",
        "category": "research",
        "publisher": { "name": "CogniCloud", "workspace": "cognicloud" },
        "tools": ["browser"],
        "instructions": "Research.",
        "maxSteps": 20,
        "timeoutSeconds": 600,
        "skills": [{ "name": "Cite", "description": "When answering", "body": "Cite sources." }]
    })
}

fn client(server: &MockServer) -> Client {
    Client::builder()
        .token("amadeus-token")
        .tenant("acme")
        .marketplace_base_url(format!("{}/v1/marketplace", server.uri()))
        .build()
        .unwrap()
}

#[tokio::test]
async fn lists_templates_by_category_and_reads_one() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/marketplace/templates"))
        .and(header("authorization", "Bearer amadeus-token"))
        .and(header("tenant_id", "acme"))
        .and(query_param("category", "research"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([template()])))
        .expect(1)
        .mount(&server)
        .await;
    let mut community = template();
    community["publisher"] = json!({ "name": "Acme" });
    Mock::given(method("GET"))
        .and(path("/v1/marketplace/templates/web-researcher"))
        .respond_with(ResponseTemplate::new(200).set_body_json(community))
        .mount(&server)
        .await;

    let client = client(&server);
    let api = client.marketplace();
    let templates = api.list_templates(Some("research")).await.unwrap();
    assert_eq!(
        templates[0].publisher.workspace.as_deref(),
        Some("cognicloud")
    );
    let one = api.get_template("web-researcher").await.unwrap();
    assert_eq!(one.publisher.workspace, None);
    assert_eq!(one.skills[0].body, "Cite sources.");
}

#[tokio::test]
async fn pages_the_marketplace_actions() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/marketplace/actions"))
        .and(query_param("limit", "10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{
                "id": "a1",
                "action": "list_templates",
                "subjectId": null,
                "startedAt": "2026-11-03T10:00:00Z",
                "finishedAt": null,
                "durationMs": null,
                "outcome": "succeeded",
                "errorCode": null
            }],
            "nextCursor": null
        })))
        .mount(&server)
        .await;

    let page = client(&server)
        .marketplace()
        .actions(ActionLogListOptions {
            limit: Some(10),
            cursor: None,
        })
        .await
        .unwrap();
    assert_eq!(page.items[0].action, "list_templates");
}
