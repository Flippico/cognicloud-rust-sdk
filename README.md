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

Management calls use the signed-in person's Amadeus session token.

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

- `timeout_minutes` is 1 to 240 and defaults to 15 (`None` uses the default). The session stops by itself when the timeout passes.
- Closing the CDP connection does not stop the session. Call `sessions().stop` as above.
- `status` is `BrowserStatus::Active` or `BrowserStatus::Stopped`. `cdp_url` and `live_url` are `None` once the session has stopped. `live_url` is a page where a person can watch and take over the browser.
- `sessions().list()` returns the workspace's sessions, newest first, with `concurrency.active` and `concurrency.limit`. A start beyond the limit fails with status `429` and code `concurrency_limit`.
- Proxy countries, profiles, screen sizes, captcha solving and recording are not available yet. `countries()` and `profiles()` return empty lists, and `CreateBrowserSession` has no field for those options.

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
| `base_url` | `https://cognicloud.flippi.co/api/v1/ai-gateway` | Point at another environment. |
| `tenant` | `"cognicloud"` | The workspace to manage, and the tenant that owns an endpoint called by ID. |
| `locale` | `Locale::En` | `Accept-Language` for localised responses. |
| `timeout` | 30 seconds | Per request. |
| `max_retries` | `2` | For `429` and `5xx`, on calls that are safe to repeat. |
| `browser_base_url` | `https://cognicloud.flippi.co/api/v1/browser` | Browser sessions service. |

Set them on the builder, or with `Options` for `EndpointClient::with_options`.

## Development

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```
