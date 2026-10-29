use cognicloud_sdk::{Client, MailboxListOptions};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn mailbox() -> serde_json::Value {
    json!({
        "id": "7c1f0c2e-6a5b-4d3e-9f10-1a2b3c4d5e6f",
        "address": "support@mail.example.com",
        "localPart": "support",
        "domain": "mail.example.com",
        "createdAt": "2026-10-29T10:00:00Z",
        "updatedAt": "2026-10-29T10:00:00Z"
    })
}

async fn client(server: &MockServer) -> Client {
    Client::builder()
        .token("amadeus-token")
        .tenant("acme")
        .mail_base_url(format!("{}/v1/mail", server.uri()))
        .build()
        .unwrap()
}

#[tokio::test]
async fn creates_a_mailbox_and_returns_its_password_once() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/mail/mailboxes"))
        .and(header("authorization", "Bearer amadeus-token"))
        .and(header("tenant_id", "acme"))
        .and(body_json(json!({ "localPart": "support" })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "mailbox": mailbox(), "password": "Vf9!mQ2xTr7#pL4wNz8K" })))
        .mount(&server)
        .await;

    let created = client(&server).await.mail().create("support").await.unwrap();
    assert_eq!(created.mailbox.address, "support@mail.example.com");
    assert_eq!(created.password, "Vf9!mQ2xTr7#pL4wNz8K");
}

#[tokio::test]
async fn lists_with_paging_and_reads_connection_settings() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/mail/mailboxes"))
        .and(query_param("limit", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "items": [mailbox()], "total": 1, "limit": 5, "offset": 0 })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/mail/mailboxes/settings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "usernameFormat": "full mailbox address",
            "imap": { "host": "mail.example.com", "port": 993, "security": "ssl" },
            "smtp": { "host": "mail.example.com", "port": 465, "security": "ssl" }
        })))
        .mount(&server)
        .await;

    let client = client(&server).await;
    let listed = client.mail().list(MailboxListOptions { limit: Some(5), offset: None }).await.unwrap();
    assert_eq!(listed.total, 1);
    let settings = client.mail().connection_settings().await.unwrap();
    assert_eq!(settings.imap.port, 993);
    assert_eq!(settings.smtp.port, 465);
}

#[tokio::test]
async fn deletes_and_rotates_a_password() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/v1/mail/mailboxes/mb-1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mail/mailboxes/mb-1/password/rotate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "password": "new-pass-1!" })))
        .mount(&server)
        .await;

    let client = client(&server).await;
    client.mail().delete("mb-1").await.unwrap();
    assert_eq!(client.mail().rotate_password("mb-1").await.unwrap().password, "new-pass-1!");
}
