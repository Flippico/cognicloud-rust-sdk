# Changelog

All notable changes to this crate are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows [Semantic Versioning](https://semver.org/).

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
