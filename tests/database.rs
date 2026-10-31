use cognicloud_sdk::{Client, DatabaseActionListOptions, DatabaseClient, Error, Statement};
use serde::Deserialize;
use serde_json::{json, Value};
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn result() -> Value {
    json!({
        "columns": [{ "name": "id", "type": "int4" }],
        "rows": [{ "id": 1 }],
        "rowCount": 1,
        "truncated": false
    })
}

fn db(server: &MockServer) -> DatabaseClient {
    DatabaseClient::with_base_url("cgd_key", format!("{}/v1/database", server.uri())).unwrap()
}

async fn answer(server: &MockServer, verb: &str, route: &str, expected: Value) {
    Mock::given(method(verb))
        .and(path(format!("/v1/database{route}")))
        .and(header("authorization", "Bearer cgd_key"))
        .and(body_json(expected))
        .respond_with(ResponseTemplate::new(200).set_body_json(result()))
        .expect(1)
        .mount(server)
        .await;
}

#[test]
fn refuses_a_missing_key() {
    assert!(matches!(DatabaseClient::new(" "), Err(Error::Config(_))));
}

#[tokio::test]
async fn runs_sql_with_the_key_and_no_tenant() {
    let server = MockServer::start().await;
    answer(
        &server,
        "POST",
        "/query",
        json!({ "sql": "SELECT id FROM orders WHERE status = $1", "params": ["paid"] }),
    )
    .await;

    #[derive(Deserialize)]
    struct Row {
        id: i32,
    }
    let result = db(&server)
        .query("SELECT id FROM orders WHERE status = $1", &[json!("paid")])
        .await
        .unwrap();
    assert_eq!(result.row_count, 1);
    assert_eq!(result.columns[0].kind, "int4");
    assert_eq!(result.rows_as::<Row>().unwrap()[0].id, 1);

    let requests: Vec<Request> = server.received_requests().await.unwrap();
    assert!(!requests[0].headers.contains_key("tenant_id"));
}

#[tokio::test]
async fn does_not_repeat_a_query_because_it_may_change_data() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/database/query"))
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_json(json!({ "error": "service_unavailable", "message": "Try again." })),
        )
        .expect(1)
        .mount(&server)
        .await;
    let error = db(&server)
        .query("DELETE FROM orders", &[])
        .await
        .unwrap_err();
    assert_eq!(error.status(), 503);
}

#[tokio::test]
async fn reports_the_postgres_code_of_a_failed_statement() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/database/query"))
        .respond_with(ResponseTemplate::new(400).set_body_json(
            json!({ "code": "23505", "error": "query_failed", "message": "duplicate key" }),
        ))
        .mount(&server)
        .await;
    let error = db(&server).query("INSERT", &[]).await.unwrap_err();
    assert_eq!(error.status(), 400);
    assert_eq!(error.code(), "query_failed");
    assert_eq!(error.query_code(), Some("23505"));
    assert!(error.to_string().contains("duplicate key"));
}

#[tokio::test]
async fn other_errors_have_no_query_code() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/database/tables"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_json(json!({ "error": "unauthorized", "message": "No." })),
        )
        .mount(&server)
        .await;
    let error = db(&server).tables().await.unwrap_err();
    assert_eq!(error.code(), "unauthorized");
    assert_eq!(error.query_code(), None);
}

#[tokio::test]
async fn sends_a_transaction() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/database/transaction"))
        .and(body_json(json!({ "statements": [
            { "sql": "UPDATE a SET b = 1", "params": [] },
            { "sql": "DELETE FROM a WHERE id = $1", "params": [2] }
        ] })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "results": [result(), result()] })),
        )
        .mount(&server)
        .await;
    let results = db(&server)
        .transaction(&[
            Statement::new("UPDATE a SET b = 1", vec![]),
            Statement::new("DELETE FROM a WHERE id = $1", vec![json!(2)]),
        ])
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn lists_tables() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/database/tables"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "tables": [
            { "name": "orders", "columns": [{ "name": "id", "type": "integer", "nullable": false }] }
        ] })))
        .mount(&server)
        .await;
    let tables = db(&server).tables().await.unwrap();
    assert_eq!(tables[0].name, "orders");
    assert!(!tables[0].columns[0].nullable);
}

#[tokio::test]
async fn builds_a_select_with_every_filter() {
    let server = MockServer::start().await;
    answer(
        &server,
        "POST",
        "/tables/order%20items/select",
        json!({
            "columns": ["id", "email"],
            "filters": [
                { "column": "a", "op": "eq", "value": 1 },
                { "column": "b", "op": "neq", "value": "two" },
                { "column": "c", "op": "gt", "value": 3 },
                { "column": "d", "op": "gte", "value": 4 },
                { "column": "e", "op": "lt", "value": 5 },
                { "column": "f", "op": "lte", "value": 6 },
                { "column": "g", "op": "like", "value": "%x" },
                { "column": "h", "op": "ilike", "value": "%y" },
                { "column": "i", "op": "is", "value": null },
                { "column": "j", "op": "in", "value": [1, 2] }
            ],
            "order": [{ "column": "id", "desc": true }, { "column": "email", "desc": false }],
            "limit": 10,
            "offset": 20
        }),
    )
    .await;
    db(&server)
        .table("order items")
        .select(&["id", "email"])
        .eq("a", 1)
        .neq("b", "two")
        .gt("c", 3)
        .gte("d", 4)
        .lt("e", 5)
        .lte("f", 6)
        .like("g", "%x")
        .ilike("h", "%y")
        .is("i", None)
        .is_in("j", vec![json!(1), json!(2)])
        .order("id", true)
        .order("email", false)
        .limit(10)
        .offset(20)
        .execute()
        .await
        .unwrap();
}

#[tokio::test]
async fn selects_every_column_by_default() {
    let server = MockServer::start().await;
    answer(
        &server,
        "POST",
        "/tables/orders/select",
        json!({ "columns": [], "filters": [], "order": [] }),
    )
    .await;
    db(&server)
        .table("orders")
        .select(&[])
        .execute()
        .await
        .unwrap();
}

#[tokio::test]
async fn inserts_updates_and_deletes() {
    let server = MockServer::start().await;
    let filters = json!([{ "column": "id", "op": "eq", "value": 1 }]);
    answer(
        &server,
        "POST",
        "/tables/orders/insert",
        json!({ "rows": [{ "email": "a" }, { "email": "b" }] }),
    )
    .await;
    answer(
        &server,
        "POST",
        "/tables/orders/update",
        json!({ "values": { "total": 5 }, "filters": filters }),
    )
    .await;
    answer(
        &server,
        "POST",
        "/tables/orders/delete",
        json!({ "filters": filters }),
    )
    .await;

    let db = db(&server);
    let orders = db.table("orders");
    orders
        .insert(&[json!({ "email": "a" }), json!({ "email": "b" })])
        .await
        .unwrap();
    orders
        .update(&json!({ "total": 5 }))
        .unwrap()
        .eq("id", 1)
        .execute()
        .await
        .unwrap();
    orders.delete().eq("id", 1).execute().await.unwrap();
}

async fn person(server: &MockServer) -> Client {
    Client::builder()
        .token("session")
        .tenant("acme")
        .database_base_url(format!("{}/v1/database", server.uri()))
        .build()
        .unwrap()
}

#[tokio::test]
async fn manages_databases_as_the_signed_in_person() {
    let server = MockServer::start().await;
    let database = json!({ "id": "db1", "name": "Orders", "createdAt": "2026-10-30T10:00:00Z" });
    Mock::given(method("POST"))
        .and(path("/v1/database/databases"))
        .and(header("authorization", "Bearer session"))
        .and(header("tenant_id", "acme"))
        .and(body_json(json!({ "name": "Orders" })))
        .respond_with(ResponseTemplate::new(201).set_body_json(&database))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/database/databases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "items": [&database] })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/database/databases/db1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&database))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/database/databases/db1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = person(&server).await;
    let databases = client.database();
    assert_eq!(databases.create("Orders").await.unwrap().id, "db1");
    assert_eq!(databases.list().await.unwrap()[0].name, "Orders");
    assert_eq!(databases.get("db1").await.unwrap().name, "Orders");
    databases.delete("db1").await.unwrap();
}

#[tokio::test]
async fn lists_and_revokes_keys_and_pages_through_actions() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/database/databases/db1/keys"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "id": "key1", "databaseId": "db1", "name": "Agent", "access": "read",
            "prefix": "cgd_1234", "createdAt": "2026-10-30T10:00:00Z", "lastUsedAt": null
        }])))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/database/databases/db1/keys/key1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/database/actions"))
        .and(query_param("limit", "5"))
        .and(query_param("cursor", "abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{
                "id": "a1", "action": "create_key", "databaseId": "db1", "name": "Agent",
                "startedAt": "2026-10-30T10:00:00Z", "finishedAt": "2026-10-30T10:00:01Z",
                "durationMs": 12, "outcome": "succeeded", "errorCode": null
            }],
            "nextCursor": "next"
        })))
        .mount(&server)
        .await;

    let client = person(&server).await;
    let databases = client.database();
    assert_eq!(databases.keys("db1").await.unwrap()[0].access, "read");
    databases.revoke_key("db1", "key1").await.unwrap();
    let page = databases
        .actions(DatabaseActionListOptions {
            limit: Some(5),
            cursor: Some("abc".to_owned()),
        })
        .await
        .unwrap();
    assert_eq!(page.next_cursor.as_deref(), Some("next"));
    assert_eq!(page.items[0].duration_ms, Some(12));
}

#[tokio::test]
async fn runs_sql_and_lists_tables_as_the_owner() {
    let server = MockServer::start().await;
    let value = "a078e7865b1073c4d2c1eaa92a9b5507";
    Mock::given(method("POST"))
        .and(path("/v1/database/databases/db1/query"))
        .and(header("authorization", "Bearer session"))
        .and(header("tenant_id", "acme"))
        .and(body_json(
            json!({ "sql": "SELECT id FROM orders WHERE id = $1", "params": [value] }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(result()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/database/databases/db1/tables"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "tables": [{ "name": "orders", "columns": [] }] })),
        )
        .mount(&server)
        .await;

    let client = person(&server).await;
    let databases = client.database();
    let found = databases
        .query(
            "db1",
            "SELECT id FROM orders WHERE id = $1",
            &[json!(value)],
        )
        .await
        .unwrap();
    assert_eq!(found.row_count, 1);
    assert_eq!(databases.tables("db1").await.unwrap()[0].name, "orders");
}

#[tokio::test]
async fn does_not_repeat_an_owner_query() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/database/databases/db1/query"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;
    let client = person(&server).await;
    let error = client
        .database()
        .query("db1", "DELETE FROM orders", &[])
        .await
        .unwrap_err();
    assert_eq!(error.status(), 503);
}
