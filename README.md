# cognicloud-rust-sdk

Call CogniCloud AI Gateway endpoints and manage them from async Rust on Tokio. Same behaviour as [`@cognicloud/sdk`](https://github.com/Flippico/cognicloud-js-sdk), except that a missing product key is reported by the call that needs it, not when the API object is created.

> Version 2.0.0. Every call to a product takes an account key together with that product's key; see [API keys](#api-keys), [Migrating from 1.x](#migrating-from-1x) and [CHANGELOG.md](./CHANGELOG.md).

## Install

```toml
[dependencies]
cognicloud-sdk = "2"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
serde_json = "1"
```

Built on `reqwest` with `rustls`. Every request and response is a typed `serde` struct.

## API keys

Create the keys in the dashboard under **Settings → API keys**. Each is shown once.

- An **account key** (`cc_acc_...`) names the workspace and acts as the person who created it. Every client needs one.
- A **product key** reaches one product: `cc_gw_` AI Gateway, `cc_db_` Database, `cc_br_` Browser, `cc_ml_` Mail, `cc_ag_` Agents, `cc_st_` Storage. It can be narrowed to one resource, such as one endpoint, database or agent. Add one for each product you call.

Every call to a product sends both keys: the product key as the Bearer token and the account key in the `x-cognicloud-account-key` header. The servers refuse each key on its own: an account key alone gets `403`, a product key alone gets `401`.

The web builder and the marketplace have no product key yet, so they send the account key alone as the Bearer token. This is an exception, and it may change.

The keys name the workspace, so there is no `tenant` option and no `tenant_id` header. Keys cannot be created through the SDK or the API.

```rust
use cognicloud_sdk::Client;

let client = Client::builder()
    .account_key(std::env::var("COGNICLOUD_ACCOUNT_KEY")?)
    .product_key(std::env::var("COGNICLOUD_GATEWAY_KEY")?)
    .build()?;
```

`build()` checks the keys and fails with `Error::Key`:

| Code | When |
| --- | --- |
| `missing_account_key` | No account key, or a blank one. |
| `invalid_key` | The account key is not an account key, a product key is an account key, or the text is not a CogniCloud key. |
| `retired_key` | A key in a retired format (`cgb_`, `cgd_`, `cga_`, `ccag_`, `cgm_`, `cgv_`, `cc_live_`). |
| `missing_key` | A blank product key. |
| `key_mismatch` | A product key is from another workspace than the account key. |

A product you call without its product key fails with `missing_product_key` at the call, before any request is sent. The message names the product and says where to add the key.

A second product key for the same product replaces the first. `parse_key(key)` reads a key's kind and workspace without a request.

## Call an endpoint

```rust
use cognicloud_sdk::{Client, InvokeResponse};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), cognicloud_sdk::Error> {
    let client = Client::builder()
        .account_key(std::env::var("COGNICLOUD_ACCOUNT_KEY")?)
        .product_key(std::env::var("COGNICLOUD_GATEWAY_KEY")?)
        .build()?;
    let endpoint = client.endpoint(std::env::var("COGNICLOUD_ENDPOINT_ID")?)?;

    let answer: InvokeResponse = endpoint.invoke(&json!({ "input": "Summarise this ticket in one sentence." })).await?;
    println!("{} ({:?}, {} tokens)", answer.output, answer.finish_reason, answer.usage.total_tokens);
    Ok(())
}
```

`invoke` is generic: use [`InvokeResponse`] for the shape every model returns, or deserialise into your own struct. It is never retried, because a retried call can be billed twice.

The call acts as the endpoint's owner, with the account key and an AI Gateway key; a gateway key narrowed to one endpoint reaches only that one. `EndpointClient::new(endpoint_id, account_key, gateway_key)` and `EndpointClient::from_url(url, account_key, gateway_key)` make the same client without a `Client`. Both keys are required and must be from one workspace; the invoke URL's workspace comes from the account key.

## Send a PDF or an image

```rust
use cognicloud_sdk::{EndpointClient, pdf_part, text_part};
use serde_json::json;

let endpoint = EndpointClient::from_url(
    std::env::var("COGNICLOUD_ENDPOINT_URL")?,
    std::env::var("COGNICLOUD_ACCOUNT_KEY")?,
    std::env::var("COGNICLOUD_GATEWAY_KEY")?,
)?;
let cv = std::fs::read("cv.pdf")?;

let answer: serde_json::Value = endpoint
    .invoke(&json!({ "messages": [{ "role": "user", "content": [text_part("Match this CV to the job."), pdf_part("cv.pdf", &cv)?] }] }))
    .await?;
```

Files go up to 10 MB. The gateway reserves a quarter of a file's size in tokens before the call, so an endpoint's monthly limit must cover it.

## Manage endpoints

With the account key and an AI Gateway key:

```rust
use cognicloud_sdk::{Client, CreateEndpoint, DailyUsageRange, Locale, UpdateEndpoint};

let client = Client::builder()
    .account_key(account_key)
    .product_key(gateway_key)
    .locale(Locale::En)
    .build()?;

let created = client
    .endpoints()
    .create(CreateEndpoint {
        name: "Support bot".into(),
        provider_id: "azure-openai".into(),
        model: "gpt-5-mini".into(),
        token_limit: 1_000_000,
    })
    .await?;

client
    .endpoints()
    .update(&created.endpoint.id, UpdateEndpoint { token_limit: Some(2_000_000), ..Default::default() })
    .await?;
let usage = client.endpoints().usage(&created.endpoint.id).await?;
let days = client
    .endpoints()
    .daily_usage(&created.endpoint.id, DailyUsageRange { from: Some("2026-10-01".into()), to: None })
    .await?;
client.endpoints().delete(&created.endpoint.id).await?;
```

Also available: `endpoints().list()` and `endpoints().get(id)`.

Secrets are wrapped in `Secret<T>`: their `Debug` output is `Secret(****)`, and you read the value with `expose()`.

## Browser sessions

Browser calls need the account key and a browser key (`cc_br_`):

```rust
use cognicloud_sdk::{Client, CreateBrowserSession};

let browser = Client::builder()
    .account_key(std::env::var("COGNICLOUD_ACCOUNT_KEY")?)
    .product_key(std::env::var("COGNICLOUD_BROWSER_KEY")?)
    .build()?;
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

let client = Client::builder()
    .account_key(account_key)
    .product_key(browser_key)
    .build()?;
let session = client
    .browser()
    .sessions()
    .create(&CreateBrowserSession { timeout_minutes: Some(15), ..Default::default() })
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
- `screen` sets the window size, and `headful: Some(true)` starts a visible Chrome window instead of a headless one, for sites that turn headless Chrome away (browser service 0.10.0).
- Proxy countries, captcha solving and recording are not available yet. `countries()` returns an empty list, and `CreateBrowserSession` has no field for those options.

## Mail

Mailboxes on the Flippico mail server, with the account key and a mail key (`cc_ml_`). The password is returned once; store it.

```rust
let client = Client::builder()
    .account_key(account_key)
    .product_key(mail_key)
    .build()?;

let created = client.mail().create("support").await?;
let settings = client.mail().connection_settings().await?; // IMAP and SMTP hosts, and default_domain

// Your own domain: add it, publish domain.records at your DNS provider, then verify.
let domain = client.mail().domains().add("yourcompany.com").await?;
client.mail().domains().verify(&domain.id).await?;
client.mail().create_on("ceo", "yourcompany.com").await?;
```

`mail().list`, `get`, `rotate_password` and `delete` manage mailboxes; `mail().domains().list`, `get` and `delete` manage domains.

## Database

Hosted Postgres. Creating and listing databases needs the account key and a database key (`cc_db_`). `database().data(...)` queries one: with a database key narrowed to one database pass `None`; with a database key for the whole workspace, name the database with `data(Some(&database.id))`, which is sent as `x-database-id`.

```rust
use cognicloud_sdk::{Client, Statement};
use serde_json::json;

let cc = Client::builder()
    .account_key(account_key)
    .product_key(database_key)
    .build()?;
let database = cc.database().create("Orders").await?;
let db = cc.database().data(Some(&database.id));

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

`cc.database().query(&database.id, sql, params)` and `cc.database().tables(&database.id)` also work without `data`. `cc.database()` also has `list`, `get`, `delete` and `actions`.

## Storage

Buckets and objects, with the account key and a storage key (`cc_st_`).

```rust
use cognicloud_sdk::{
    Client, CreateStorageBucket, StorageDeleteOptions, StorageDownloadOptions,
    StorageObjectListOptions, StorageUploadOptions,
};

let client = Client::builder()
    .account_key(account_key)
    .product_key(storage_key)
    .build()?;
let storage = client.storage();

storage.buckets().create(CreateStorageBucket { name: "photos".into(), description: None }).await?;
storage.folders().create("photos", "2026/").await?;
storage.objects().upload("photos", "2026/a.jpg", bytes, StorageUploadOptions {
    content_type: Some("image/jpeg".into()),
}).await?;

let whole = storage.objects().download("photos", "2026/a.jpg", StorageDownloadOptions::default()).await?;
let meta = storage.objects().metadata("photos", "2026/a.jpg").await?;

// Every page of a listing: call `next` until it returns None, or join them with `all`.
let mut pages = storage.objects().pages("photos", StorageObjectListOptions {
    prefix: Some("2026/".into()),
    ..Default::default()
});
while let Some(page) = pages.next().await {
    for object in page?.objects {
        println!("{} {}", object.key, object.size);
    }
}

storage.objects().delete("photos", "2026/a.jpg").await?;
storage.buckets().delete("photos", StorageDeleteOptions { force: true }).await?;
```

- `buckets()` has `list`, `get`, `update` and `delete`. Deleting a bucket that still holds objects fails with `409` (`conflict`) unless `force` is set.
- `objects()` has `list`, `pages`, `upload`, `download`, `metadata` and `delete`. `download` takes `StorageDownloadOptions { range }` with a `StorageRange::Span(start, end)`, `From(start)` or `Last(n)`.
- An upload is at most 100 MB and is never retried. The content type defaults to `application/octet-stream`.
- Keys are checked before a request is sent. A key or prefix with an empty middle segment, a `.` or `..` segment, a leading `/` or a control character fails with `invalid_key`. A trailing `/` is allowed.

## Web builder

Build a React application and host it at `https://<subdomain>.cognicloud.dev`. Leave out `repo_url` to start from a Vite + React starter. Each person can have 3 applications. The web builder takes the account key alone for now (see [API keys](#api-keys)).

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

## Agents

An agent answers through one of your AI Gateway endpoints, with its instructions and skills. Agents calls need the account key and an Agents key (`cc_ag_`). Create one, then run it.

```rust
use cognicloud_sdk::{Client, CreateAgent, CreateSkill};

let cc = Client::builder()
    .account_key(account_key)
    .product_key(agents_key)
    .build()?;

let skill = cc.skills().create(&CreateSkill {
    name: "Cite".to_owned(),
    description: "When the answer uses a source".to_owned(),
    body: "Name every source you use.".to_owned(),
}).await?;

let created = cc.agents().create(&CreateAgent {
    name: "Researcher".to_owned(),
    endpoint_id,
    instructions: "Answer briefly.".to_owned(),
    tools: Vec::new(),
    max_steps: 10,
    timeout_seconds: 300,
    skill_ids: vec![skill.id],
    ..Default::default()
}).await?;
let credential = created.credential.secret.expose().clone(); // ccag_..., shown once
```

`agents()` also has `list`, `get`, `update`, `delete`, `rotate_credential` and `actions`. Pause an agent with `UpdateAgent { status: Some(AgentStatus::Paused), .. }`. `skills()` has `list`, `get`, `update`, `delete` and `versions`; every change keeps the earlier version. `publish` and `unpublish` are planned and answer `503` for now.

### Runs

```rust
use std::time::Duration;
use cognicloud_sdk::{RunListOptions, RunStatus, RunWaitOptions};

let run = cc
    .runs()
    .run_and_wait(&agent_id, "Summarise KSeF in two sentences.", RunWaitOptions::default())
    .await?;
let quick = cc
    .runs()
    .run_and_wait(&agent_id, "Say hi.", RunWaitOptions { poll_interval: Duration::from_millis(500), timeout: Duration::from_secs(60) })
    .await?;
match run.status {
    RunStatus::Completed => println!("{}", run.output.unwrap_or_default()),
    _ => println!("{:?}: {:?}", run.error_code, run.error_message),
}

let started = cc.runs().start(&agent_id, "Check the status page.").await?;
let now = cc.runs().get(&started.id).await?;
cc.runs().stop(&started.id).await?;
let page = cc.runs().list(&agent_id, RunListOptions { limit: Some(20), cursor: None }).await?;
```

- `start` returns at once with `RunStatus::Running`. A run ends `Completed`, `Failed` or `Stopped`; `steps` hold each step's `thought`, `tool` and `summary`.
- `run_and_wait` reads the run every `poll_interval` and returns it when it is no longer running, failed and stopped runs included. Only `timeout` passing is an error, `Error::Timeout`, and the run keeps going. `RunWaitOptions::default()` polls every 2 seconds for up to 15 minutes.
- `error_code` is `MaxSteps`, `Timeout`, `ModelError`, `InvalidAnswer`, `Interrupted` or `Stopped`. Codes and statuses this version does not know read as `Unknown`.
- Starting fails with `agent_paused` (409), `endpoint_missing` (422) or `limit_reached` (429, at most 3 running at once). It is never retried.
- In this release every tool answers "not available yet"; the run calls do not change when tools arrive.

An Agents key (`cc_ag_`) narrowed to one agent reaches only that agent and its runs, with the account key as for every product call; other calls answer `403 forbidden`.

```rust
let agent = Client::builder()
    .account_key(std::env::var("COGNICLOUD_ACCOUNT_KEY")?)
    .product_key(std::env::var("COGNICLOUD_AGENT_KEY")?)
    .build()?;
let run = agent.runs().start(&agent_id, "Go").await?;
```

### Marketplace

The marketplace takes the account key alone for now (see [API keys](#api-keys)).

```rust
let templates = cc.marketplace().list_templates(Some("research")).await?;
let template = cc.marketplace().get_template(&templates[0].slug).await?;
```

Create an agent from one with `CreateAgent { template_slug: Some(template.slug), .. }`. `marketplace().actions` pages the marketplace's action log.

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

`status()` is the HTTP status (`0` when no answer came back). `code()` is CogniCloud's stable error code, or a local one such as `timeout` or `network_error`. The key codes are in [API keys](#api-keys).

## Configuration

| Option | Default | |
| --- | --- | --- |
| `base_url` | `https://cognicloud.dev/api/v1/ai-gateway` | Point at another environment. |
| `locale` | `Locale::En` | `Accept-Language` for localised responses. |
| `timeout` | 30 seconds | Per request. |
| `max_retries` | `2` | For `429` and `5xx`, on calls that are safe to repeat. |
| `browser_base_url` | `https://cognicloud.dev/api/v1/browser` | Browser sessions service. |
| `mail_base_url` | `https://cognicloud.dev/api/v1/mail` | Mail service. |
| `database_base_url` | `https://cognicloud.dev/api/v1/database` | Database service. |
| `web_builder_base_url` | `https://cognicloud.dev/api/v1/web-builder` | Web builder service. |
| `agents_base_url` | `https://cognicloud.dev/api/v1/agents` | Agents service: agents, skills and runs. |
| `marketplace_base_url` | `https://cognicloud.dev/api/v1/marketplace` | Agent templates. |
| `storage_base_url` | `https://cognicloud.dev/api/v1/storage` | Storage service: buckets and objects. |

Set them on the builder, next to the required `account_key`, or with `Options` for `EndpointClient::with_options`. `Options` is non-exhaustive, so start from `Options::default()` and set the fields you need:

```rust
let mut options = cognicloud_sdk::Options::default();
options.storage_base_url = "https://staging.example.test/api/v1/storage".to_owned();
```

## Migrating from 1.x

| 1.x | 2.0 |
| --- | --- |
| `Client::builder().api_key(account_key)` | `Client::builder().account_key(account_key).product_key(gateway_key)`, with one product key for each product you call |
| `Client::builder().api_key(database_key)` (`cc_db_`) | `.account_key(account_key).product_key(database_key)`: the account key is now required |
| `Client::builder().api_key(browser_key)` (`cc_br_`) | `.account_key(account_key).product_key(browser_key)` |
| `Client::builder().api_key(mail_key)` (`cc_ml_`) | `.account_key(account_key).product_key(mail_key)` |
| `Client::builder().api_key(agents_key)` (`cc_ag_`) | `.account_key(account_key).product_key(agents_key)` |
| `EndpointClient::new(id, api_key)` | `EndpointClient::new(id, account_key, gateway_key)` |
| `EndpointClient::from_url(url, api_key)` | `EndpointClient::from_url(url, account_key, gateway_key)` |
| `EndpointClient::with_options(id, api_key, options)` | `EndpointClient::with_options(id, account_key, gateway_key, options)` |
| `client.endpoint(id)?` | Unchanged. The client needs an AI Gateway product key, or the call fails with `missing_product_key`. |
| `Client::builder().api_key(key)` for the web builder or marketplace | `Client::builder().account_key(account_key)`: the account key alone |

Create the account key and every product key before you upgrade. A key that does not pair with the account key fails at `build()` (`key_mismatch`), and a product call without its product key fails with `missing_product_key`.

## Migrating from 0.x

| 0.x | 1.0 |
| --- | --- |
| `Client::builder().token(session).tenant(..)` | `Client::builder().api_key(key)` with an account key |
| `EndpointClient::new(id, "cc_live_...")` | `client.endpoint(id)?`, or `EndpointClient::new(id, api_key)?` |
| `BrowserClient::new("cgb_...")` | `Client::builder().api_key("cc_br_...").build()?.browser()` |
| `DatabaseClient::new("cgd_...")` | `client.database().data(None)` with a `cc_db_` key |
| `.token("ccag_...")` | `.api_key("cc_ag_...")` narrowed to the agent |
| `endpoints().rotate_credential`, `database().keys`, `agents().keys()` | Settings → API keys in the dashboard |

The 0.x keys were retired in 1.0 and are refused with `retired_key`. The 1.0 column is what those versions used; see the table above for 2.0.

## Development

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```
