# Module: Security Access Boundary

## Goals
- Provide a configurable access-control boundary for LAN/public usage.
- Avoid breaking local workflows by default.
- Keep the design compatible with future user/tenant systems.

## In Scope
- HTTP API access control for /api routes.
- SSE and WebSocket access control (events, logs, diff streams).
- MCP server requests that proxy through HTTP.
- Config schema updates and normalization.

## Out of Scope / Right Boundary
- User accounts, OAuth, SSO, or RBAC.
- Multi-tenant data model.
- Rate limiting or full audit logging.
- UI login screens or session management.

## Design Summary
- AccessControlConfig:
  - mode: disabled | token
  - token: string (shared secret)
  - allow_localhost_bypass: bool (default true)
- Token can be supplied via:
  - Authorization: Bearer <token>
  - X-API-Token: <token>
  - token query param for SSE/WS when headers are unavailable
- /health remains open for health checks.
- Static frontend assets remain public; API routes are protected.
- Config responses must not leak the token. Redact token in UserSystemInfo.

## Failure Behavior Matrix
| Endpoint Type | Auth Required | Token Location | Failure Status | Failure Body |
| --- | --- | --- | --- | --- |
| HTTP /api/* | token mode only | Authorization/X-API-Token | 401 | ApiResponse JSON (success=false, message) |
| SSE /api/events | token mode only | ?token=... | 401 | ApiResponse JSON (success=false, message) |
| WS /api/* (upgrade) | token mode only | ?token=... | 401 (no upgrade) | empty or ApiResponse JSON |
| MCP proxy (HTTP) | token mode only | Authorization/X-API-Token | 401 | ApiResponse JSON (success=false, message) |

## Localhost Bypass
- When allow_localhost_bypass=true, requests from 127.0.0.1/::1 bypass token checks.
- When allow_localhost_bypass=false, all requests require token in token mode.

## Integration Points
- Server middleware in crates/server/src/middleware
- Router wiring in crates/server/src/routes/mod.rs
- SSE/WS handlers in crates/server/src/routes/events.rs and task_attempts routes
- MCP proxy in crates/server/src/mcp/task_server.rs

## Testing
- Unit tests for middleware token parsing and localhost bypass.
- Integration tests for /api/info and /api/tasks with and without token.
- Manual curl checks documented in task acceptance criteria.
