## Context

The backend is an Axum server that serves a React frontend and a JSON API under
`/api/*`. The app streams realtime state using both WebSocket and SSE endpoints
(e.g., `/api/events`). Today, API responses are not compressed.

The backend also makes outbound HTTPS requests (e.g., translation routes, OAuth
flows, and other integrations). `reqwest` is currently pinned to `0.12` in at
least `crates/server` and `crates/execution`.

## Goals / Non-Goals

**Goals:**
- Add transparent gzip + brotli response compression for `/api/*` without
  changing any response body shape.
- Keep streaming endpoints (SSE) working correctly (no buffering, no broken
  clients).
- Upgrade `reqwest` to `0.13` and use OS certificate store verification for
  better enterprise compatibility.

**Non-Goals:**
- No protocol changes for WebSocket/SSE payload formats.
- No changes to authentication / access-control behavior.
- No new remote/cloud deployment components.

## Decisions

### Decision: Use `tower_http::compression::CompressionLayer` for `/api/*`

We will enable `tower-http`'s compression features for brotli and gzip, and add
`CompressionLayer` at the API router boundary (not globally for every route).

Key behaviors:
- Prefer brotli (`br`) when the client supports it; otherwise fall back to gzip.
- Do not compress responses that are already encoded.

### Decision: Explicitly exclude SSE (`text/event-stream`) from compression

SSE relies on incremental flushing; compression can cause buffering and break
the realtime UX. We will exclude `Content-Type: text/event-stream` responses
from compression using a compression predicate (or by layering compression only
on non-SSE routers).

We will also ensure WebSocket upgrade responses are unaffected (they do not
carry a normal HTTP body).

### Decision: Upgrade to `reqwest` 0.13 with rustls + platform verifier

We will upgrade `reqwest` to `0.13` with `default-features = false` and use the
`rustls` feature set that consults the OS certificate store via rustls' platform
verifier (matching upstream behavior). This avoids TLS failures in environments
where a corporate root CA is installed in the OS trust store.

We will keep the change minimal: do not refactor all HTTP call sites; only
adjust code where the API has changed.

## Risks / Trade-offs

- **[SSE buffering]** → exclude `text/event-stream` responses and add a streaming
  smoke test.
- **[Dependency churn]** (`reqwest` + rustls feature changes) → upgrade the
  minimal set of crates, run `cargo test --workspace`, and add a small unit test
  that exercises client construction for HTTPS.
- **[Behavior differences]** between TLS stacks → document the chosen feature
  flags and avoid mixing multiple `reqwest` major versions.

## Migration Plan

1. Enable `tower-http` compression features and add the compression layer to the
   API router with SSE exclusions.
2. Upgrade `reqwest` dependencies to `0.13` (and adjust feature flags).
3. Run backend checks/lints/tests and add targeted smoke tests.
4. If issues are found, roll back by reverting the compression layer and/or
   pinning `reqwest` back to `0.12`.

## Open Questions

- Should we add an env toggle to disable compression for debugging? (Default:
  **no**, keep behavior simple.)
- Should compression apply to static assets as well? (Default: **no**, focus on
  `/api/*` only.)

