# cognicloud-rust-sdk

Call CogniCloud AI Gateway endpoints and manage them from async Rust on Tokio. Same behaviour as [`@cognicloud/sdk`](https://github.com/Flippico/cognicloud-js-sdk).

> Version 0.3.0. Covers AI Gateway endpoints and browser sessions; agents, scraping, vault and activity come later. See [CHANGELOG.md](./CHANGELOG.md).

## Install

```toml
[dependencies]
cognicloud-sdk = "0.4"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
serde_json = "1"
```

Built on `reqwest` with `rustls`. Every request and response is a typed `serde` struct.

## Call an endpoint

```rust
use cognicloud_sdk::{EndpointClient, InvokeResponse};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), cognicloud_sdk::Error> {
    let endpoint = EndpointClient::new(
        std::env::var("COGNICLOUD_ENDPOINT_ID")?,
        std::env::var("COGNICLOUD_KEY")?,
    );

    let answer: InvokeResponse = endpoint.invoke(&json!({ "input": "Summarise this ticket in one sentence." })).await?;
    println!("{} ({:?}, {} tokens)", answer.output, answer.finish_reason, answer.usage.total_tokens);
    Ok(())
}
```

`invoke` is generic: use [`InvokeResponse`] for the shape every model returns, or deserialise into your own struct. It is never retried, because a retried call can be billed twice.

## Send a PDF or an image

```rust
use cognicloud_sdk::{EndpointClient, pdf_part, text_part};
use serde_json::json;

let endpoint = EndpointClient::from_url(std::env::var("COGNICLOUD_ENDPOINT_URL")?, std::env::var("COGNICLOUD_KEY")?);
let cv = std::fs::read("cv.pdf")?;

let answer: serde_json::Value = endpoint
    .invoke(&json!({ "messages": [{ "role": "user", "content": [text_part("Match this CV to the job."), pdf_part("cv.pdf", &cv)?] }] }))
    .await?;
```

Files go up to 10 MB. The gateway reserves a quarter of a file's size in tokens before the call, so an endpoint's monthly limit must cover it.

## Manage endpoints

Calls act as the account of the token: the signed-in person's Amadeus session token, or a browser key for browser calls. A blank token is refused when the client is built, with `Error::Config`.

```rust
use cognicloud_sdk::{Client, CreateEndpoint, DailyUsageRange, Locale, UpdateEndpoint};

let client = Client::builder().token(amadeus_token).locale(Locale::En).build()?;

let created = client
    .endpoints()
    .create(CreateEndpoint {
        name: "Support bot".into(),
        provider_id: "azure-openai".into(),
        model: "gpt-5-mini".into(),
        token_limit: 1_000_000,
    })
    .await?;
// Shown once: store it now.
store_secret("COGNICLOUD_KEY", created.credential.secret.expose());

client
    .endpoints()
    .update(&created.endpoint.id, UpdateEndpoint { token_limit: Some(2_000_000), ..Default::default() })
    .await?;
let usage = client.endpoints().usage(&created.endpoint.id).await?;
let days = client
    .endpoints()
    .daily_usage(&created.endpoint.id, DailyUsageRange { from: Some("2026-10-01".into()), to: None })
    .await?;
client.endpoints().rotate_credential(&created.endpoint.id).await?;
client.endpoints().delete(&created.endpoint.id).await?;
```

Also available: `endpoints().list()` and `endpoints().get(id)`.

Secrets are wrapped in `Secret<T>`: their `Debug` output is `Secret(****)`, and you read the value with `expose()`.

## Browser sessions

Browser calls accept the signed-in person's token, or a browser key. Create a key in the dashboard under **Browser → API keys** and pass it as the token. A key keeps working after its owner signs out. Keys cannot be created through the SDK or the API.

Use the browser product with a browser key alone. The key names its workspace, so no Amadeus token or tenant is needed:

```rust
use cognicloud_sdk::{BrowserClient, CreateBrowserSession};

let browser = BrowserClient::new(std::env::var("COGNICLOUD_BROWSER_KEY")?)?;
let session = browser.browser().sessions().create(&CreateBrowserSession { timeout_minutes: Some(15), ..Default::default() }).await?;
```

Upload a profile folder packed as a `.tar.gz` (see the app docs for how to pack it), then start a session from it:

```rust
let profile = browser.browser().import_profile("Shop account", std::fs::read("shop-profile.tar.gz")?).await?;
let session = browser.browser().sessions().create(&CreateBrowserSession { profile_id: Some(profile.id.clone()), ..Default::default() }).await?;
```


A browser session is a Chrome that your agent drives over the Chrome DevTools Protocol. Start one, connect your automation library to its `cdp_url`, and stop it when you are done. The example uses `chromiumoxide` and `futures`:

```rust
use cognicloud_sdk::{Client, CreateBrowserSession};
use futures::StreamExt;

let client = Client::builder().token(amadeus_token).build()?;
let session = client
    .browser()
    .sessions()
    .create(&CreateBrowserSession { timeout_minutes: Some(15) })
    .await?;

let (browser, mut handler) = chromiumoxide::Browser::connect(session.cdp_url.clone().unwrap()).await?;
tokio::spawn(async move { while handler.next().await.is_some() {} });
let page = browser.new_page("https://example.com").await?;
println!("{:?}", page.get_title().await?);

client.browser().sessions().stop(&session.id).await?;
```

Without a CDP client, read and use the page, or let an agent do it through an AI Gateway endpoint. Each step of an agent run is a call billed to the endpoint.

```rust
use cognicloud_sdk::{BrowserPageAction, CreateStepRun, RunStep, StartAgentRun};

let sessions = client.browser().sessions();
let page = sessions.read_page(&session.id, false).await?;
sessions.act(&session.id, &BrowserPageAction::Click { index: 3 }, false).await?;

let run = sessions.start_agent_run(&session.id, &StartAgentRun {
    task: "Find the price of the Pro plan".to_owned(),
    endpoint_id,
    credential,
    max_steps: None,
}).await?;
let result = sessions.get_agent_run(&session.id, &run.id).await?.result;

// For serverless callers: a stored run that does one step per request.
let stored = sessions.create_step_run(&session.id, &CreateStepRun {
    task: "Find the price".to_owned(),
    endpoint_id,
    max_steps: None,
}).await?;
let after = sessions.run_step(&session.id, &stored.id, &RunStep {
    credential,
    timeout_seconds: Some(120),
}).await?;
```

- `timeout_minutes` is 1 to 240 and defaults to 15 (`None` uses the default). The session stops by itself when the timeout passes.
- Closing the CDP connection does not stop the session. Call `sessions().stop` as above.
- `status` is `BrowserStatus::Active` or `BrowserStatus::Stopped`. `cdp_url` and `live_url` are `None` once the session has stopped. `live_url` is a page where a person can watch and take over the browser.
- `sessions().list()` returns the workspace's sessions, newest first, with `concurrency.active` and `concurrency.limit`. A start beyond the limit fails with status `429` and code `concurrency_limit`.
- Proxy countries, profiles, screen sizes, captcha solving and recording are not available yet. `countries()` and `profiles()` return empty lists, and `CreateBrowserSession` has no field for those options.

## Mail

Mailboxes on the Flippico mail server. The token is a signed-in person's token or a mail key (`cgm_...`). The password is returned once; store it.

```rust
let created = client.mail().create("support").await?;
let settings = client.mail().connection_settings().await?; // IMAP and SMTP hosts, and default_domain

// Your own domain: add it, publish domain.records at your DNS provider, then verify.
let domain = client.mail().domains().add("yourcompany.com").await?;
client.mail().domains().verify(&domain.id).await?;
client.mail().create_on("ceo", "yourcompany.com").await?;
```

`mail().list`, `get`, `rotate_password` and `delete` manage mailboxes; `mail().domains().list`, `get` and `delete` manage domains. Adding, verifying and deleting domains need a signed-in person's token.

## Database

Hosted Postgres. A signed-in person creates databases; a database key (`cgd_...`) from the dashboard queries one of them and needs nothing else. A `read` key can only read. A `write` key runs as the database owner.

```rust
use cognicloud_sdk::{Client, DatabaseClient, Statement};
use serde_json::json;

let cc = Client::builder().token(session_token).build()?;
let database = cc.database().create("Orders").await?;

let db = DatabaseClient::new(std::env::var("COGNICLOUD_DATABASE_KEY")?)?;

db.query("CREATE TABLE orders (id serial PRIMARY KEY, email text, total numeric)", &[]).await?;
let rows = db.query("SELECT * FROM orders WHERE total > $1", &[json!(100)]).await?.rows;

let orders = db.table("orders");
orders.insert(&[json!({ "email": "a@example.com", "total": "19.90" })]).await?;
let paid = orders.select(&["id", "email"]).gt("total", 10).order("id", true).limit(20).execute().await?;
orders.update(&json!({ "total": "0" }))?.eq("id", 1).execute().await?;
orders.delete().eq("id", 1).execute().await?;

db.transaction(&[
    Statement::new("UPDATE accounts SET balance = balance - $1 WHERE id = $2", vec![json!(10), json!(1)]),
    Statement::new("UPDATE accounts SET balance = balance + $1 WHERE id = $2", vec![json!(10), json!(2)]),
])
.await?;
let tables = db.tables().await?;
```

- Filters are `eq`, `neq`, `gt`, `gte`, `lt`, `lte`, `like`, `ilike`, `is` and `is_in`. `update` and `delete` need at least one; to change every row, use `query`.
- `select`, `update` and `delete` run on `execute()`. `insert` runs at once.
- A result has `rows`, `columns`, `row_count` and `truncated`, and `rows_as::<T>()` decodes rows into your own type. One request stops after 15 seconds and returns at most 1,000 rows and 5 MB.
- `numeric` values, and `bigint` values beyond 2^53, come back as strings, so nothing is rounded. `bytea` comes back as base64.
- A failed statement returns `Error::Query { query_code, message }`: `query_code` is a Postgres SQLSTATE such as `23505`, or `query_timeout`.
- `query` and `transaction`, inserts, updates and deletes are never retried, because they may change data.

Pass values as `params`, never inside the SQL. They need no quotes and cannot break the statement:

```rust
db.query("INSERT INTO members (id, user_id) VALUES ($1, $2)", &[json!(id), json!(user_id)]).await?;
```

A signed-in person can run SQL in their own database without a key: `cc.database().query(&database.id, sql, params)` and `cc.database().tables(&database.id)`.

`cc.database()` also has `list`, `get`, `delete`, `keys`, `revoke_key` and `actions`. Keys are created in the dashboard.

## Web builder

Build a React application and host it at `https://<subdomain>.cognicloud.dev`. Leave out `repo_url` to start from a Vite + React starter. Each person can have 3 applications.

```rust
use cognicloud_sdk::CreateApplication;

let app = cc.web_builder().create(&CreateApplication {
    subdomain: "shop".to_owned(),
    repo_url: Some("https://github.com/acme/shop".to_owned()),
    ..Default::default()
}).await?;

let status = cc.web_builder().get(&app.id).await?.status;
let logs = cc.web_builder().logs(&app.id).await?;
```

`create` starts the build and returns at once, with `status` set to `building`. It then becomes `running` or `failed`. `branch`, `build_command` and `output_dir` default to `main`, `npm run build` and `dist`. `list`, `redeploy` and `delete` do the rest.

```rust
use cognicloud_sdk::{UpdateApplication, WebBuilderActionListOptions};

let api = cc.web_builder();
api.update(&app.id, &UpdateApplication {
    branch: Some("next".to_owned()),
    ..Default::default()
}).await?;
api.pause(&app.id).await?;
api.resume(&app.id).await?;
let requests = api.runtime_logs(&app.id).await?;
let history = api.actions(WebBuilderActionListOptions {
    application_id: Some(app.id.clone()),
    ..Default::default()
}).await?;

api.create_repository(&app.id, "shop").await?;
api.enable_auto_deploy(&app.id).await?;
api.disable_auto_deploy(&app.id).await?;
```

`update` starts a build, and a paused site has `status` set to `paused`. `create_repository` and the auto-deploy methods need a connected GitHub account: `authorize_github`, `connect_github`, `github_repositories` and `disconnect_github`.

## Responses with MCP tools

```rust
use cognicloud_sdk::{McpTool, ResponsesRequest, ToolChoice};

let answer = endpoint
    .responses(&ResponsesRequest {
        instructions: Some("Check invoices.".into()),
        tools: vec![McpTool::new("ksef").allowed_tools(["validate_invoice_xml"])],
        tool_choice: Some(ToolChoice::Required),
        ..ResponsesRequest::text("Is this invoice valid?")
    })
    .await?;

// Continue the same conversation on the same endpoint:
let next = endpoint
    .responses(&ResponsesRequest {
        previous_response_id: answer.response_id.clone(),
        ..ResponsesRequest::text("And this one?")
    })
    .await?;
```

To use your own MCP server, name its address and bearer token on the tool with `McpTool::new(label).server(url, token)`. The gateway checks that the address is https, without credentials or a fragment, and sends the token to the provider in the request. It sets approval to `never` itself, and the SDK cannot change that. Tokens are never shown by `Debug`, and the gateway does not store them. A tool without `server` names a server the gateway is configured with.

## Embeddings

```rust
use cognicloud_sdk::EmbeddingsRequest;

let result = endpoint
    .embeddings(&EmbeddingsRequest::texts(["first text", "second text"]).dimensions(256))
    .await?;
let first = &result.data[0].embedding;
```

The endpoint must point at an embedding deployment. Each call is billed as prompt tokens.

## Streaming

```rust
use cognicloud_sdk::StreamEvent;

let mut stream = endpoint
    .stream(&serde_json::json!({ "messages": [{ "role": "user", "content": "Write a haiku." }] }))
    .await?;
while let Some(event) = stream.next().await {
    match event? {
        StreamEvent::Delta(text) => print!("{text}"),
        StreamEvent::Done(answer) => println!("\n{} tokens", answer.usage.total_tokens),
        StreamEvent::Error { code, message } => eprintln!("{code}: {message}"),
    }
}
```

The stream ends after one `Done` or one `Error`. Streams are not retried, so a failed start returns an error before any event.

## Extra provider fields

`ResponsesRequest::extra` adds any field the typed fields do not cover. The gateway passes it to the provider unchanged, except for the fields it sets itself, which it refuses.

```rust
let request = ResponsesRequest::text("Summarise this ticket.")
    .extra("seed", 7)
    .extra("reasoning", serde_json::json!({ "effort": "low" }));
```

## Errors

```rust
use cognicloud_sdk::Error;

match endpoint.invoke::<_, serde_json::Value>(&json!({ "input": "Hi" })).await {
    Err(error) if error.code() == "credits_exhausted" => {
        // top up credits
    }
    other => { other?; }
}
```

`status()` is the HTTP status (`0` when no answer came back). `code()` is CogniCloud's stable error code, or a local one such as `timeout` or `network_error`.

## Configuration

| Option | Default | |
| --- | --- | --- |
| `base_url` | `https://cognicloud.dev/api/v1/ai-gateway` | Point at another environment. |
| `tenant` | `"cognicloud"` | The workspace to manage, and the tenant that owns an endpoint called by ID. |
| `locale` | `Locale::En` | `Accept-Language` for localised responses. |
| `timeout` | 30 seconds | Per request. |
| `max_retries` | `2` | For `429` and `5xx`, on calls that are safe to repeat. |
| `browser_base_url` | `https://cognicloud.dev/api/v1/browser` | Browser sessions service. |
| `mail_base_url` | `https://cognicloud.dev/api/v1/mail` | Mail service. |
| `database_base_url` | `https://cognicloud.dev/api/v1/database` | Database service. |
| `web_builder_base_url` | `https://cognicloud.dev/api/v1/web-builder` | Web builder service. |

Set them on the builder, or with `Options` for `EndpointClient::with_options`.

## Development

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```
