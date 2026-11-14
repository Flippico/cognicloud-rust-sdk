mod common;
use common::*;

use cognicloud_sdk::{parse_key, Client, EndpointClient, Error, KeyKind, MailboxListOptions};
use serde_json::json;
use wiremock::matchers::{any, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const BROWSER_2: &str =
    "cc_br_636f676e69636c6f7564_2222222222222222222222222222222222222222222222222222222222222222";

fn acme_account() -> String {
    format!("cc_acc_61636d65_{}", "a".repeat(64))
}

fn acme_key(prefix: &str) -> String {
    format!("cc_{prefix}_61636d65_{}", "b".repeat(64))
}

fn code(error: &Error) -> &str {
    error.code()
}

fn key_error<T: std::fmt::Debug>(result: Result<T, Error>) -> Error {
    result.unwrap_err()
}

#[test]
fn a_missing_account_key_is_refused_when_the_client_is_built() {
    for account in [None, Some(""), Some("   ")] {
        let mut builder = Client::builder().product_key(GATEWAY);
        if let Some(account) = account {
            builder = builder.account_key(account);
        }
        let error = key_error(builder.build());
        assert_eq!(code(&error), "missing_account_key", "{account:?}");
        assert!(matches!(error, Error::Key { .. }));
    }
}

#[test]
fn a_product_key_is_refused_as_the_account_key() {
    for product in [GATEWAY, BROWSER, MAIL, DATABASE, AGENTS] {
        let error = key_error(Client::builder().account_key(product).build());
        assert_eq!(code(&error), "invalid_key", "{product}");
    }
}

#[test]
fn an_account_key_is_refused_as_a_product_key() {
    let error = key_error(
        Client::builder()
            .account_key(ACCOUNT)
            .product_key(ACCOUNT)
            .build(),
    );
    assert_eq!(code(&error), "invalid_key");
    assert!(
        error.to_string().contains("This is an account key"),
        "{error}"
    );
}

#[test]
fn a_product_key_from_another_workspace_is_a_mismatch() {
    let error = key_error(
        Client::builder()
            .account_key(ACCOUNT)
            .product_key(acme_key("br"))
            .build(),
    );
    assert_eq!(code(&error), "key_mismatch");

    let error = key_error(EndpointClient::new("e1", ACCOUNT, acme_key("gw")));
    assert_eq!(code(&error), "key_mismatch");

    let error = key_error(EndpointClient::from_url(
        "https://cognicloud.dev/api/v1/ai-gateway/invoke/636f676e69636c6f7564/e1",
        ACCOUNT,
        acme_key("gw"),
    ));
    assert_eq!(code(&error), "key_mismatch");
}

#[test]
fn an_account_key_from_another_workspace_is_a_mismatch_for_the_endpoint() {
    let error = key_error(EndpointClient::new("e1", acme_account(), GATEWAY));
    assert_eq!(code(&error), "key_mismatch");
}

#[test]
fn an_endpoint_needs_an_ai_gateway_key_in_the_gateway_slot() {
    for key in [BROWSER, MAIL, DATABASE, AGENTS] {
        let error = key_error(EndpointClient::new("e1", ACCOUNT, key));
        assert_eq!(code(&error), "invalid_key", "{key}");
    }
    let error = key_error(EndpointClient::new("e1", ACCOUNT, ACCOUNT));
    assert_eq!(code(&error), "invalid_key");
}

#[tokio::test]
async fn a_missing_product_key_fails_at_the_call_before_any_request() {
    let server = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let client = Client::builder()
        .account_key(ACCOUNT)
        .base_url(server.uri())
        .browser_base_url(server.uri())
        .mail_base_url(server.uri())
        .database_base_url(server.uri())
        .agents_base_url(server.uri())
        .build()
        .unwrap();

    let errors = [
        (
            "AI Gateway",
            key_error(client.endpoints().list().await.map(|_| ())),
        ),
        ("AI Gateway", key_error(client.endpoint("e1").map(|_| ()))),
        (
            "Browser",
            key_error(client.browser().profiles().await.map(|_| ())),
        ),
        (
            "Mail",
            key_error(
                client
                    .mail()
                    .list(MailboxListOptions::default())
                    .await
                    .map(|_| ()),
            ),
        ),
        (
            "Database",
            key_error(client.database().list().await.map(|_| ())),
        ),
        (
            "Database",
            key_error(
                client
                    .database()
                    .data(None)
                    .query("SELECT 1", &[])
                    .await
                    .map(|_| ()),
            ),
        ),
        (
            "Agents",
            key_error(client.agents().list().await.map(|_| ())),
        ),
        (
            "Agents",
            key_error(client.runs().get("r1").await.map(|_| ())),
        ),
    ];
    for (label, error) in errors {
        let article = if label == "AI Gateway" { "an" } else { "a" };
        assert_eq!(code(&error), "missing_product_key", "{label}");
        assert!(matches!(error, Error::Key { .. }));
        assert!(
            error
                .to_string()
                .contains(&format!("Add {article} {label} product key")),
            "{error}"
        );
    }
}

#[test]
fn a_retired_key_is_refused_in_either_slot() {
    for old in [
        "cgb_6163_abc",
        "cgd_x",
        "cga_6163_x",
        "cgm_x",
        "cgv_x",
        "cc_live_x",
        "ccag_6163_x",
    ] {
        let error = key_error(Client::builder().account_key(old).build());
        assert_eq!(code(&error), "retired_key", "account {old}");
        assert!(error.to_string().contains("Settings → API keys"), "{error}");

        let error = key_error(
            Client::builder()
                .account_key(ACCOUNT)
                .product_key(old)
                .build(),
        );
        assert_eq!(code(&error), "retired_key", "product {old}");

        let error = key_error(EndpointClient::new("e1", old, GATEWAY));
        assert_eq!(code(&error), "retired_key", "endpoint account {old}");

        let error = key_error(EndpointClient::new("e1", ACCOUNT, old));
        assert_eq!(code(&error), "retired_key", "endpoint gateway {old}");
    }
}

#[test]
fn something_that_is_not_a_key_is_refused() {
    for other in [
        "eyJhbGciOi.x.y",
        "cc_acc_zz_abc",
        "cc_acc_616_abc",
        "cc_xx_6163_abc",
    ] {
        let error = key_error(Client::builder().account_key(other).build());
        assert_eq!(code(&error), "invalid_key", "{other}");
        let error = key_error(
            Client::builder()
                .account_key(ACCOUNT)
                .product_key(other)
                .build(),
        );
        assert_eq!(code(&error), "invalid_key", "product {other}");
    }
}

#[test]
fn parse_key_reads_the_kind_and_workspace_of_each_key() {
    let parsed = parse_key(ACCOUNT).unwrap();
    assert_eq!(parsed.kind, KeyKind::Account);
    assert_eq!(parsed.tenant_hex, "636f676e69636c6f7564");

    for (key, kind) in [
        (GATEWAY, KeyKind::Gateway),
        (DATABASE, KeyKind::Database),
        (BROWSER, KeyKind::Browser),
        (MAIL, KeyKind::Mail),
        (AGENTS, KeyKind::Agents),
        ("cc_vt_636f676e69636c6f7564_vvvv", KeyKind::Vault),
    ] {
        assert_eq!(parse_key(key).unwrap().kind, kind, "{key}");
    }

    assert_eq!(code(&key_error(parse_key(""))), "missing_key");
    assert_eq!(code(&key_error(parse_key("cgb_6163_abc"))), "retired_key");
    assert_eq!(code(&key_error(parse_key("cc_xx_6163_abc"))), "invalid_key");
}

#[tokio::test]
async fn the_last_key_for_a_product_replaces_the_first() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/browser/profiles"))
        .and(header("authorization", bearer(BROWSER_2)))
        .and(header(ACCOUNT_HEADER, ACCOUNT))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(1)
        .mount(&server)
        .await;
    let client = Client::builder()
        .account_key(ACCOUNT)
        .product_key(BROWSER)
        .product_key(BROWSER_2)
        .browser_base_url(format!("{}/v1/browser", server.uri()))
        .build()
        .unwrap();
    assert!(client.browser().profiles().await.unwrap().is_empty());
}

#[test]
fn a_vault_key_is_accepted_though_no_product_uses_it() {
    Client::builder()
        .account_key(ACCOUNT)
        .product_key("cc_vt_636f676e69636c6f7564_vvvv")
        .build()
        .unwrap();
}

#[tokio::test]
async fn an_endpoint_client_sends_the_gateway_key_and_the_account_key() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/invoke/636f676e69636c6f7564/e1"))
        .and(header("authorization", bearer(GATEWAY)))
        .and(header(ACCOUNT_HEADER, ACCOUNT))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "1",
            "model": "gpt-5-mini",
            "output": "Hello",
            "finishReason": "stop",
            "usage": { "inputTokens": 1, "outputTokens": 1, "totalTokens": 2 }
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = Client::builder()
        .with_keys()
        .base_url(server.uri())
        .build()
        .unwrap();
    client
        .endpoint("e1")
        .unwrap()
        .invoke::<_, serde_json::Value>(&json!({ "input": "Hi" }))
        .await
        .unwrap();
}

#[tokio::test]
async fn a_database_key_alone_with_the_account_key_queries_its_own_database() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/database/query"))
        .and(header("authorization", bearer(DATABASE)))
        .and(header(ACCOUNT_HEADER, ACCOUNT))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "columns": [], "rows": [], "rowCount": 0, "truncated": false
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = Client::builder()
        .account_key(ACCOUNT)
        .product_key(DATABASE)
        .database_base_url(format!("{}/v1/database", server.uri()))
        .max_retries(0)
        .build()
        .unwrap();
    client
        .database()
        .data(None)
        .query("SELECT 1", &[])
        .await
        .unwrap();
    let received = server.received_requests().await.unwrap();
    assert!(received[0].headers.get("x-database-id").is_none());
}

#[tokio::test]
async fn the_web_builder_and_marketplace_send_the_account_key_alone() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/web-builder/applications"))
        .and(header("authorization", bearer(ACCOUNT)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "items": [] })))
        .expect(1)
        .mount(&server)
        .await;
    let client = Client::builder()
        .account_key(ACCOUNT)
        .web_builder_base_url(format!("{}/v1/web-builder", server.uri()))
        .build()
        .unwrap();
    assert!(client.web_builder().list().await.unwrap().is_empty());
    let received = server.received_requests().await.unwrap();
    assert!(received[0].headers.get(ACCOUNT_HEADER).is_none());
}

#[test]
fn debug_output_hides_every_key() {
    let account = "cc_acc_636f676e69636c6f7564_acctsecret000111";
    let gateway = "cc_gw_636f676e69636c6f7564_gwsecret000222";
    let browser = "cc_br_636f676e69636c6f7564_brsecret000333";
    let builder = Client::builder()
        .account_key(account)
        .product_key(gateway)
        .product_key(browser);
    let client = builder.build().unwrap();
    let endpoint = client.endpoint("e1").unwrap();
    let standalone = EndpointClient::new("e1", account, gateway).unwrap();
    let printed = format!(
        "{:?} {:?} {:?} {:?}",
        Client::builder().account_key(account).product_key(gateway),
        client,
        endpoint,
        standalone
    );
    for secret in ["acctsecret", "gwsecret", "brsecret"] {
        assert!(!printed.contains(secret), "{printed}");
    }
    assert!(printed.contains("Secret(****)"), "{printed}");
}
