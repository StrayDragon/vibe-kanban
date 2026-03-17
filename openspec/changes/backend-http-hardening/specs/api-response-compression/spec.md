## ADDED Requirements

### Requirement: API responses negotiate gzip/brotli compression

The server SHALL compress HTTP responses for `/api/*` endpoints when the client
indicates support via `Accept-Encoding`.

The server SHALL support `br` (Brotli) and `gzip`. If the client supports `br`,
the server SHALL prefer `br` over `gzip`.

#### Scenario: Client requests brotli

- **WHEN** a client sends `Accept-Encoding: br` to a JSON API endpoint under
  `/api/*`
- **THEN** the response includes `Content-Encoding: br`

#### Scenario: Client requests gzip

- **WHEN** a client sends `Accept-Encoding: gzip` (and not `br`) to a JSON API
  endpoint under `/api/*`
- **THEN** the response includes `Content-Encoding: gzip`

#### Scenario: Client does not request compression

- **WHEN** a client omits `Accept-Encoding` (or does not include `br`/`gzip`)
- **THEN** the response does not include a `Content-Encoding` header

### Requirement: Streaming endpoints are not compressed

The server SHALL NOT apply compression to Server-Sent Events responses
(`Content-Type: text/event-stream`) and SHALL preserve streaming flush
semantics.

#### Scenario: SSE remains uncompressed

- **WHEN** a client connects to `/api/events` (SSE)
- **THEN** the response does not include `Content-Encoding`

