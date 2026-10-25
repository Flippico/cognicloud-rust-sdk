# Changelog

All notable changes to this crate are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows [Semantic Versioning](https://semver.org/).

## [0.9.0] - 2026-10-25

### Added

- `ResponsesRequest::extra(key, value)` adds any other field, sent to the provider unchanged. The gateway refuses the fields it sets itself (`n`, `best_of`, `background`, `store`, `stream_options`, `prompt`, `conversation`).

## [0.8.0] - 2026-10-25

### Added

- `McpTool::server(url, bearer_token)` names an MCP server's address and its bearer token on a tool. The gateway passes them to the provider unchanged, apart from its checks: https, no credentials or fragment, and one `Authorization` header. The token is never shown by `Debug`.
- A tool without `server` still names only the label, for servers the gateway is configured with.

## [0.7.0] - 2026-10-25

### Added

- `EndpointClient::stream(&body)` streams a Chat or Responses call: it sets `stream: true` and returns a `ResponseStream`.
- `EndpointClient::stream_responses(&ResponsesRequest)` streams a Responses call with tools or chaining.
- `ResponseStream::next()` yields `StreamEvent::Delta(text)` as text arrives, then one `StreamEvent::Done(ResponseResult)`. A provider failure is `StreamEvent::Error { code, message }`, and a stream that closes early is `Error` with code `stream_ended`.
- Streams are never retried, and have no total timeout: the gateway ends them at its own limit. Dropping a stream closes the connection; the gateway still settles the call.

## [0.6.0] - 2026-10-24

### Added

- `EndpointClient::embeddings(&EmbeddingsRequest)` embeds one text or up to 256 texts through the endpoint's embedding deployment, with optional `dimensions`. Returns one vector per input in input order, and bills the prompt tokens.

## [0.5.0] - 2026-10-24

### Added

- `EndpointClient::responses(&ResponsesRequest)` sends a Responses call: text or messages, optional MCP tools, `tool_choice`, `instructions`, `max_output_tokens`, and `previous_response_id` to continue an earlier response on the same endpoint.
- `McpTool::new(label).allowed_tools([...])`. The platform supplies the MCP server's address and credentials; callers name a server by label only.
- `ResponseResult` carries `response_id` and `output_items` (tool calls, tool lists and messages) next to the usual answer.

Existing methods are unchanged. Streaming is not part of this version.

## [0.4.0] - 2026-10-22

### Breaking

- The crate is named `cognicloud-sdk` (was `cognicloud`). Import it as `cognicloud_sdk`: `use cognicloud_sdk::{Client, ...};`.

## [0.3.0] - 2026-10-21

- Browser sessions: `client.browser().sessions()` creates, lists, reads and stops sessions; `countries()` and `profiles()` return empty lists until those features exist. `Options::browser_base_url` and `ClientBuilder::browser_base_url` point at another environment (default `https://cognicloud.flippi.co/api/v1/browser`). `Options` gained a field, so construct it with `..Options::default()`.

## [0.2.0] - 2026-10-20

Aligns the crate with the gateway as deployed, as the JavaScript SDK does, and adds file support.

### Breaking

- The default `base_url` is `https://cognicloud.flippi.co/api/v1/ai-gateway`. The old default, `https://api.cognicloud.dev/v1`, does not resolve.
- `EndpointClient` posts to `/invoke/{tenant}/{endpoint_id}`, the address the gateway issues. 0.1.0 posted to `/invoke/{endpoint_id}`, which the gateway answers with `404 not_found`.
- Management calls send the session token as `Authorization: Bearer <token>` and the workspace as the `tenant_id` header. 0.1.0 sent `x-amadeus-token`, which the gateway ignores.

### Added

- `EndpointClient::from_url(url, credential)` for the invoke URL CogniCloud issued (the endpoint's `url`), and `EndpointClient::url()`.
- `Options::tenant` and `ClientBuilder::tenant` (default `"cognicloud"`), and `encode_tenant(name)`.
- File helpers: `text_part`, `pdf_part`, `image_part` and `ImageType` for chat messages, and `input_text`, `input_file`, `input_image` for Responses input. Each file becomes a base64 data URL. `MAX_FILE_BYTES` is 10 MB, the gateway's limit.
- `Error::FileTooLarge` (code `file_too_large`), returned before anything is sent.

## [0.1.0] - 2026-10-20

First version. Covers AI Gateway endpoints only.

### Added

- `EndpointClient` with `invoke`, generic over the request and response types. `InvokeResponse` describes the shape every model returns.
- `Client::builder()` with `token`, `locale`, `base_url`, `timeout` and `max_retries`.
- `Client::endpoints()`: `create`, `list`, `get`, `update`, `delete`, `rotate_credential`, `usage` and `daily_usage`.
- `Secret<T>`: redacted in `Debug`, read with `expose()`.
- `Error` with `status()` and `code()`. Timeouts, network failures and bad configuration have their own variants.

### Behaviour

- `invoke` is never retried, because a retried call can be billed twice.
- Reads and other management calls retry `429` and `5xx` up to `max_retries` times, with exponential backoff from 250 ms.
