use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::client::encode;
use crate::error::{Error, Result};
use crate::http::{Call, Http};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostedDatabase {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Deserialize)]
struct DatabaseList {
    items: Vec<HostedDatabase>,
}

/// `access` is `read` or `write`. `prefix` tells keys apart; the secret is shown only when a key is created.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseKey {
    pub id: String,
    pub database_id: String,
    pub name: String,
    pub access: String,
    pub prefix: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

/// `action` is `create_database`, `delete_database`, `create_key` or `revoke_key`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseAction {
    pub id: String,
    pub action: String,
    pub database_id: Option<String>,
    pub name: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub outcome: String,
    pub error_code: Option<String>,
}

/// Pass `next_cursor` as `cursor` for the next page. It is `None` on the last page.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseActionList {
    pub items: Vec<DatabaseAction>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DatabaseActionListOptions {
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ResultColumn {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
}

/// `row_count` is the rows returned or changed, including any not sent back. `truncated` says more matched than came
/// back: at most 1,000 rows and 5 MB do.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub columns: Vec<ResultColumn>,
    pub rows: Vec<Map<String, Value>>,
    pub row_count: u64,
    pub truncated: bool,
}

impl QueryResult {
    /// The rows as your own type. Note that `numeric` values, and `bigint` values beyond 2^53, are strings.
    pub fn rows_as<T: DeserializeOwned>(&self) -> Result<Vec<T>> {
        self.rows
            .iter()
            .map(|row| {
                serde_json::from_value(Value::Object(row.clone()))
                    .map_err(|error| Error::Decode(error.to_string()))
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TableColumn {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub nullable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DatabaseTable {
    pub name: String,
    pub columns: Vec<TableColumn>,
}

#[derive(Deserialize)]
struct TableList {
    tables: Vec<DatabaseTable>,
}

#[derive(Deserialize)]
struct TransactionResults {
    results: Vec<QueryResult>,
}

/// One SQL statement, with `$1`, `$2` for `params`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Statement {
    pub sql: String,
    pub params: Vec<Value>,
}

impl Statement {
    pub fn new(sql: impl Into<String>, params: Vec<Value>) -> Self {
        Self {
            sql: sql.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct Filter {
    column: String,
    op: &'static str,
    value: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct Order {
    column: String,
    desc: bool,
}

fn auth(key: &str) -> Vec<(&'static str, String)> {
    vec![("authorization", format!("Bearer {key}"))]
}

async fn post<T: DeserializeOwned>(
    http: &Http,
    key: &str,
    path: String,
    body: Value,
    retry: bool,
) -> Result<T> {
    http.send(Call {
        method: reqwest::Method::POST,
        path,
        headers: auth(key),
        query: Vec::new(),
        body: Some(body),
        retry,
    })
    .await
}

macro_rules! filter_methods {
    ($query:ident) => {
        impl $query<'_> {
            fn filter(mut self, column: &str, op: &'static str, value: Value) -> Self {
                self.filters.push(Filter {
                    column: column.to_owned(),
                    op,
                    value,
                });
                self
            }

            pub fn eq(self, column: &str, value: impl Into<Value>) -> Self {
                self.filter(column, "eq", value.into())
            }

            pub fn neq(self, column: &str, value: impl Into<Value>) -> Self {
                self.filter(column, "neq", value.into())
            }

            pub fn gt(self, column: &str, value: impl Into<Value>) -> Self {
                self.filter(column, "gt", value.into())
            }

            pub fn gte(self, column: &str, value: impl Into<Value>) -> Self {
                self.filter(column, "gte", value.into())
            }

            pub fn lt(self, column: &str, value: impl Into<Value>) -> Self {
                self.filter(column, "lt", value.into())
            }

            pub fn lte(self, column: &str, value: impl Into<Value>) -> Self {
                self.filter(column, "lte", value.into())
            }

            pub fn like(self, column: &str, pattern: &str) -> Self {
                self.filter(column, "like", pattern.into())
            }

            pub fn ilike(self, column: &str, pattern: &str) -> Self {
                self.filter(column, "ilike", pattern.into())
            }

            /// Compares with `null`, `true` or `false`.
            pub fn is(self, column: &str, value: Option<bool>) -> Self {
                self.filter(column, "is", value.into())
            }

            pub fn is_in(self, column: &str, values: Vec<Value>) -> Self {
                self.filter(column, "in", Value::Array(values))
            }
        }
    };
}

/// A select, sent by `execute`. Without `limit` it returns up to 1,000 rows, the most one response can hold.
#[must_use = "a query does nothing until `execute` is awaited"]
pub struct SelectQuery<'a> {
    table: &'a TableApi<'a>,
    columns: Vec<String>,
    filters: Vec<Filter>,
    order: Vec<Order>,
    limit: Option<u64>,
    offset: Option<u64>,
}

filter_methods!(SelectQuery);

impl SelectQuery<'_> {
    pub fn order(mut self, column: &str, desc: bool) -> Self {
        self.order.push(Order {
            column: column.to_owned(),
            desc,
        });
        self
    }

    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn offset(mut self, offset: u64) -> Self {
        self.offset = Some(offset);
        self
    }

    pub async fn execute(self) -> Result<QueryResult> {
        let mut body =
            json!({ "columns": self.columns, "filters": self.filters, "order": self.order });
        if let Some(limit) = self.limit {
            body["limit"] = limit.into();
        }
        if let Some(offset) = self.offset {
            body["offset"] = offset.into();
        }
        self.table.post("select", body, true).await
    }
}

/// An update or delete, sent by `execute`. At least one filter is required.
#[must_use = "a query does nothing until `execute` is awaited"]
pub struct WriteQuery<'a> {
    table: &'a TableApi<'a>,
    verb: &'static str,
    values: Option<Value>,
    filters: Vec<Filter>,
}

filter_methods!(WriteQuery);

impl WriteQuery<'_> {
    pub async fn execute(self) -> Result<QueryResult> {
        let mut body = json!({ "filters": self.filters });
        if let Some(values) = self.values {
            body["values"] = values;
        }
        self.table.post(self.verb, body, false).await
    }
}

/// One table in the `public` schema. Table and column names are checked against the database.
pub struct TableApi<'a> {
    http: &'a Http,
    key: &'a str,
    path: String,
}

impl TableApi<'_> {
    async fn post(&self, verb: &str, body: Value, retry: bool) -> Result<QueryResult> {
        post(
            self.http,
            self.key,
            format!("{}/{verb}", self.path),
            body,
            retry,
        )
        .await
    }

    /// The named columns, or every column with an empty list.
    pub fn select(&self, columns: &[&str]) -> SelectQuery<'_> {
        SelectQuery {
            table: self,
            columns: columns.iter().map(|column| (*column).to_owned()).collect(),
            filters: Vec::new(),
            order: Vec::new(),
            limit: None,
            offset: None,
        }
    }

    /// Inserts now and returns the new rows. Each row is a JSON object or a type that serializes to one; a column a
    /// row leaves out takes its default. Needs a write key.
    pub async fn insert<T: Serialize>(&self, rows: &[T]) -> Result<QueryResult> {
        let rows = serde_json::to_value(rows).map_err(|error| Error::Config(error.to_string()))?;
        self.post("insert", json!({ "rows": rows }), false).await
    }

    /// Changes the rows the filters match and returns them. Needs a write key.
    pub fn update<T: Serialize>(&self, values: &T) -> Result<WriteQuery<'_>> {
        let values =
            serde_json::to_value(values).map_err(|error| Error::Config(error.to_string()))?;
        Ok(WriteQuery {
            table: self,
            verb: "update",
            values: Some(values),
            filters: Vec::new(),
        })
    }

    /// Deletes the rows the filters match and returns them. Needs a write key.
    pub fn delete(&self) -> WriteQuery<'_> {
        WriteQuery {
            table: self,
            verb: "delete",
            values: None,
            filters: Vec::new(),
        }
    }
}

pub(crate) async fn query(
    http: &Http,
    key: &str,
    sql: &str,
    params: &[Value],
) -> Result<QueryResult> {
    let body = json!({ "sql": sql, "params": params });
    post(http, key, "/query".to_owned(), body, false).await
}

pub(crate) async fn transaction(
    http: &Http,
    key: &str,
    statements: &[Statement],
) -> Result<Vec<QueryResult>> {
    let body = json!({ "statements": statements });
    let results: TransactionResults =
        post(http, key, "/transaction".to_owned(), body, false).await?;
    Ok(results.results)
}

pub(crate) async fn tables(http: &Http, key: &str) -> Result<Vec<DatabaseTable>> {
    let list: TableList = http
        .send(Call {
            method: reqwest::Method::GET,
            path: "/tables".to_owned(),
            headers: auth(key),
            query: Vec::new(),
            body: None,
            retry: true,
        })
        .await?;
    Ok(list.tables)
}

pub(crate) fn table<'a>(http: &'a Http, key: &'a str, name: &str) -> TableApi<'a> {
    TableApi {
        http,
        key,
        path: format!("/tables/{}", encode(name)),
    }
}

/// Creates and removes hosted Postgres databases. Needs a signed-in person's token; a key is refused.
pub struct DatabaseApi<'a> {
    http: &'a Http,
    token: &'a str,
    tenant: &'a str,
}

impl<'a> DatabaseApi<'a> {
    pub(crate) fn new(http: &'a Http, token: &'a str, tenant: &'a str) -> Self {
        Self {
            http,
            token,
            tenant,
        }
    }

    fn headers(&self) -> Vec<(&'static str, String)> {
        vec![
            ("authorization", format!("Bearer {}", self.token)),
            ("tenant_id", self.tenant.to_owned()),
        ]
    }

    fn call(&self, method: reqwest::Method, path: String) -> Call<'static> {
        Call {
            method,
            path,
            headers: self.headers(),
            query: Vec::new(),
            body: None,
            retry: true,
        }
    }

    /// Newest first.
    pub async fn list(&self) -> Result<Vec<HostedDatabase>> {
        let list: DatabaseList = self
            .http
            .send(self.call(reqwest::Method::GET, "/databases".to_owned()))
            .await?;
        Ok(list.items)
    }

    pub async fn get(&self, id: &str) -> Result<HostedDatabase> {
        let path = format!("/databases/{}", encode(id));
        self.http.send(self.call(reqwest::Method::GET, path)).await
    }

    /// Creates an empty database. You can have up to 3.
    pub async fn create(&self, name: &str) -> Result<HostedDatabase> {
        let mut call = self.call(reqwest::Method::POST, "/databases".to_owned());
        call.body = Some(json!({ "name": name }));
        call.retry = false;
        self.http.send(call).await
    }

    /// Deletes the database, all its data and its keys. This cannot be undone.
    pub async fn delete(&self, id: &str) -> Result<()> {
        let mut call = self.call(
            reqwest::Method::DELETE,
            format!("/databases/{}", encode(id)),
        );
        call.retry = false;
        self.http.send::<Value>(call).await.map(|_| ())
    }

    /// Active keys, newest first. Keys are created in the dashboard and shown there once.
    pub async fn keys(&self, id: &str) -> Result<Vec<DatabaseKey>> {
        let path = format!("/databases/{}/keys", encode(id));
        self.http.send(self.call(reqwest::Method::GET, path)).await
    }

    /// The key stops working on its next request.
    pub async fn revoke_key(&self, id: &str, key_id: &str) -> Result<()> {
        let path = format!("/databases/{}/keys/{}", encode(id), encode(key_id));
        let mut call = self.call(reqwest::Method::DELETE, path);
        call.retry = false;
        self.http.send::<Value>(call).await.map(|_| ())
    }

    /// Runs one SQL statement in your own database as its owner, with `$1`, `$2` for `params`. It has the same limits
    /// as a key. Never retried, because it may change data.
    pub async fn query(&self, id: &str, sql: &str, params: &[Value]) -> Result<QueryResult> {
        let mut call = self.call(
            reqwest::Method::POST,
            format!("/databases/{}/query", encode(id)),
        );
        call.body = Some(json!({ "sql": sql, "params": params }));
        call.retry = false;
        self.http.send(call).await
    }

    /// Tables and views in the database's `public` schema.
    pub async fn tables(&self, id: &str) -> Result<Vec<DatabaseTable>> {
        let path = format!("/databases/{}/tables", encode(id));
        let list: TableList = self
            .http
            .send(self.call(reqwest::Method::GET, path))
            .await?;
        Ok(list.tables)
    }

    /// Creating and deleting databases and keys, newest first. Queries are not listed.
    pub async fn actions(&self, options: DatabaseActionListOptions) -> Result<DatabaseActionList> {
        let mut call = self.call(reqwest::Method::GET, "/actions".to_owned());
        if let Some(limit) = options.limit {
            call.query.push(("limit", limit.to_string()));
        }
        if let Some(cursor) = options.cursor {
            call.query.push(("cursor", cursor));
        }
        self.http.send(call).await
    }
}
