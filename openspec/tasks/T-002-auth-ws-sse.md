# Task: T-002 Auth for SSE and WebSocket Streams

## Background / Motivation
- Issue: P0-SEC-01
- Evidence: SSE and WS endpoints are open and bypass HTTP auth middleware.

## Scope
### In Scope
- Token validation for SSE (/api/events) and WS streams (tasks/projects/diff/logs).
- Frontend token propagation to EventSource and WebSocket URLs.

### Out of Scope / Right Boundary
- Full session management.
- Encrypting tokens in transit (assumes TLS handled externally).

## Design
### Proposed
- Accept token via query param for SSE and WS: ?token=<token>.
- Reuse AccessControlConfig from T-001.
- Frontend helpers:
  - EventSource: append token param when present.
  - WebSocket: append token param in ws:// URL.

### Alternatives Considered
- Sec-WebSocket-Protocol header (not supported by EventSource).
- Cookie-based auth (requires login flow).

## Change List
- crates/server/src/routes/events.rs: validate token before starting SSE.
- crates/server/src/routes/tasks.rs: validate token before WS upgrade.
- crates/server/src/routes/projects.rs: validate token before WS upgrade.
- crates/server/src/routes/task_attempts.rs: validate token before WS upgrade.
- frontend/src/contexts/EventStreamContext.tsx: append token param to EventSource.
- frontend/src/hooks/useJsonPatchWsStream.ts: append token param to ws URL.
- frontend/src/utils/streamLogEntries.ts: append token param to ws URL.

## Acceptance Criteria
- When mode=token, SSE/WS without token return 401.
- When token is present, streams connect successfully.
- pnpm -C frontend run test passes.

## Risks & Rollback
- Risk: URL token leaks via logs/history.
- Mitigation: only set when required; document usage.
- Rollback: disable access_control mode.

## Effort Estimate
- 0.5-1 day.

## Acceptance Scripts
### SSE Auth
```bash
export BACKEND_PORT=3001

# Without token -> 401
curl -i "http://localhost:${BACKEND_PORT}/api/events"

# With token -> 200 and stream starts
curl -i "http://localhost:${BACKEND_PORT}/api/events?token=test-token"
```
Expected:
- First call returns 401.
- Second call returns 200 and keeps the connection open.

### WebSocket Auth (optional tooling)
```bash
export BACKEND_PORT=3001

# Requires wscat (npx wscat) or websocat installed.
npx wscat -c "ws://localhost:${BACKEND_PORT}/api/tasks/stream/ws?token=test-token"
```
Expected:
- Without token the handshake fails (401).
- With token the connection opens.
