## ADDED Requirements

### Requirement: API responses 协商 gzip/brotli 压缩

当 client 通过 `Accept-Encoding` 表示支持时，server 应当（SHALL）对 `/api/*` 端点的 HTTP responses 进行压缩。

server 应当（SHALL）支持 `br`（Brotli）与 `gzip`。若 client 支持 `br`，server 应当（SHALL）优先选择 `br` 而不是 `gzip`。

#### Scenario: client 请求 brotli

- **WHEN** client 向 `/api/*` 下的 JSON API endpoint 发送 `Accept-Encoding: br`
- **THEN** response 包含 `Content-Encoding: br`

#### Scenario: client 请求 gzip

- **WHEN** client 向 `/api/*` 下的 JSON API endpoint 发送 `Accept-Encoding: gzip`（且不包含 `br`）
- **THEN** response 包含 `Content-Encoding: gzip`

#### Scenario: client 未请求压缩

- **WHEN** client 省略 `Accept-Encoding`（或不包含 `br`/`gzip`）
- **THEN** response 不包含 `Content-Encoding` header

### Requirement: Streaming endpoints 不做压缩

server 不得（SHALL NOT）对 Server-Sent Events responses（`Content-Type: text/event-stream`）应用压缩，并应当（SHALL）保持 streaming flush 语义。

#### Scenario: SSE 保持未压缩

- **WHEN** client 连接 `/api/events`（SSE）
- **THEN** response 不包含 `Content-Encoding`
