# cognicloud-rust-sdk

Call CogniCloud AI Gateway endpoints and manage them from async Rust on Tokio. Same behaviour as [`@cognicloud/sdk`](https://github.com/Flippico/cognicloud-js-sdk).

> Version 0.1.0. The first version covers AI Gateway endpoints. Agents, browser, scraping, vault and activity come later. See [CHANGELOG.md](./CHANGELOG.md).

## Install

```toml
[dependencies]
cognicloud = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
serde_json = "1"
```

Built on `reqwest` with `rustls`. Every request and response is a typed `serde` struct.

## Call an endpoint

```rust
use cognicloud::{EndpointClient, InvokeResponse};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), cognicloud::Error> {
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

## Manage endpoints

Management calls use the signed-in person's Amadeus session token.

```rust
use cognicloud::{Client, CreateEndpoint, DailyUsageRange, Locale, UpdateEndpoint};

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

## Errors

```rust
use cognicloud::Error;

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
| `base_url` | `https://api.cognicloud.dev/v1` | Point at another environment. |
| `locale` | `Locale::En` | `Accept-Language` for localised responses. |
| `timeout` | 30 seconds | Per request. |
| `max_retries` | `2` | For `429` and `5xx`, on calls that are safe to repeat. |

Set them on the builder, or with `Options` for `EndpointClient::with_options`.

## Development

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```
