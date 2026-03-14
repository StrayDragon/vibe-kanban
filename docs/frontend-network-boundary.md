# Frontend Network Boundary (HTTP / WS / SSE)

To reduce “triggered but no UI change” failure modes, this repo enforces a
single, auditable network boundary.

## Allowlist

Only modules under `frontend/src/api/**` may:

- call `fetch(...)`
- construct `new WebSocket(...)`
- construct `new EventSource(...)` (SSE)

All other frontend code (components, hooks, contexts, utils) MUST call the
functions exported from `@/lib/api` (re-export of `frontend/src/api/**`) or use
the realtime primitives that *delegate* socket construction to `frontend/src/api`.

## Enforcement

- `pnpm -C frontend run lint` fails if `fetch`/`WebSocket`/`EventSource` are used
  outside the allowlist.

## Patterns

- **HTTP**: `makeRequest(...)` + `handleApiResponse(...)` from `frontend/src/api/client.ts`
- **WS/SSE construction**: `createWebSocket(...)` / `createEventSource(...)` from
  `frontend/src/api/realtime.ts`

