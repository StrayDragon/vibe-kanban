## Why

Vibe Kanban frequently moves large API payloads (normalized logs, diffs, task
lists, etc.) between the backend and the web UI. Adding HTTP response
compression reduces bandwidth and improves perceived latency, especially for
self-hosted deployments on slower or higher-latency links.

Separately, some users behind Cloudflare WARP or corporate proxies experience
TLS failures because the OS certificate store contains additional trust anchors
that are not present in bundled root sets. Upgrading to `reqwest` 0.13 with
rustls' platform verifier improves compatibility by consulting the OS trust
store.

## What Changes

- Enable gzip + brotli compression for `/api/*` HTTP responses (with explicit
  exclusions for streaming endpoints like SSE).
- Upgrade `reqwest` to `0.13` for outbound HTTP and ensure HTTPS verification
  uses the OS certificate store via rustls' platform verifier.
- Add targeted smoke tests and verification commands for both changes.

## Capabilities

### New Capabilities

- `api-response-compression`: Negotiate gzip/brotli compression for `/api/*`
  responses based on `Accept-Encoding`, while preserving streaming semantics.
- `reqwest-0-13-os-cert-store`: Outbound HTTPS requests validate using the OS
  certificate store to support enterprise environments.

### Modified Capabilities

<!-- None -->

## Impact

- Backend router layering (`crates/server`) and `tower-http` feature flags.
- Outbound HTTP usage in `crates/server`, `crates/execution`, and any other crate
  depending on `reqwest`.
- Cargo dependency graph and `Cargo.lock`.

## Goals / Non-Goals

**Goals:**
- Reduce payload sizes for large JSON responses without changing API shapes.
- Improve outbound HTTPS compatibility behind WARP/corporate proxies.

**Non-Goals:**
- No remote/cloud architecture changes.
- No API pagination or response-shape refactors.

## Risks

- Compression can interfere with streaming endpoints (SSE) → explicitly exclude
  `text/event-stream` responses and verify with a streaming smoke test.
- `reqwest` upgrade can change TLS/HTTP behavior → add focused smoke tests and
  run the existing backend checks/lints.

## Verification

- `curl` with `Accept-Encoding: br` / `gzip` returns the expected
  `Content-Encoding`.
- SSE endpoint `/api/events` still streams correctly (no buffering).
- `pnpm run backend:check`, `pnpm run lint`, `cargo test --workspace`.

