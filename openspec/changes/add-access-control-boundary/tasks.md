## 0. Scope & Constraints
- Scope: Configurable token boundary for `/api/**` (HTTP + `/api/events` SSE + relevant WS upgrades); frontend token injection; route-level tests.
- Non-goals: user accounts, OAuth/RBAC, multi-tenant model; protecting static assets; changing success response shapes.

## 1. Config surface
- [ ] 1.1 Add `AccessControl` to the latest config schema with defaults (`mode=disabled`, `allowLocalhostBypass=true`).
- [ ] 1.2 Ensure config serialization/deserialization is backward compatible and does not leak secrets by default.
- [ ] 1.3 Redact `accessControl.token` in any “system info” API responses used by the frontend.

## 2. Backend: HTTP middleware boundary
- [ ] 2.1 Implement `/api/**` auth middleware in `crates/server/src/http/` (skip `/health` + static assets).
- [ ] 2.2 Token sources: `Authorization: Bearer <token>` and `X-API-Token: <token>`.
- [ ] 2.3 Implement `allowLocalhostBypass` (localhost may omit token when enabled).
- [ ] 2.4 Unauthenticated requests return `401` with an `ApiResponse` error payload.
- [ ] 2.5 Add structured logs for auth failures (path, peer addr, reason) without logging token values.

## 3. Backend: SSE + WebSocket auth
- [ ] 3.1 For `/api/events` SSE, accept `?token=<token>` (since EventSource cannot set headers) and enforce the same boundary as `/api/**`.
- [ ] 3.2 For WS upgrades, accept `?token=<token>` and reject with `401` when missing/invalid.

## 4. Frontend: token injection
- [ ] 4.1 Implement token persistence helper (localStorage key `vk_api_token`).
- [ ] 4.2 Inject token into HTTP in `frontend/src/api/client.ts` (Authorization header).
- [ ] 4.3 Inject token into SSE URL in `frontend/src/contexts/EventStreamContext.tsx`.
- [ ] 4.4 Inject token into WS URLs used by log streams / patch streams (`frontend/src/utils/streamLogEntries.ts`, `frontend/src/hooks/useJsonPatchWsStream.ts`).

## 5. Tests
- [ ] 5.1 Backend tests: `/api/info` auth (401/200) and `/health` remains public.
- [ ] 5.2 Backend tests: SSE `/api/events` token required in token mode.
- [ ] 5.3 Backend tests: WS token required in token mode (missing token rejected).
- [ ] 5.4 Frontend tests: API client injects Authorization header when token present.
- [ ] 5.5 Frontend tests: SSE/WS URL includes `?token=` when token present.

## 6. Verification
- [ ] 6.1 `cargo test --workspace`
- [ ] 6.2 `pnpm -C frontend run test`
- [ ] 6.3 `pnpm -C frontend run check`
- [ ] 6.4 `pnpm -C frontend run lint`

## Acceptance Criteria
- `mode=disabled`: `/api/**`, SSE, and WS are accessible without token.
- `mode=token`: `/api/**`, SSE, and WS require a valid token for non-localhost callers and respond with `401` on failure.
- `allowLocalhostBypass=true`: localhost may access `/api/**` without token; non-localhost still requires token.
- Token value is never returned to the client via system-info/config endpoints.

