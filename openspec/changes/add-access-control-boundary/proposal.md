# Change: Add access-control boundary (HTTP/SSE/WS)

## Why
- `/api` (including SSE + WebSocket streams) is currently unauthenticated, which makes LAN/public deployment risky.
- We need a minimal, configurable boundary without introducing accounts, OAuth, or RBAC.

## What Changes
- Add `accessControl` config with:
  - `mode`: `disabled | token`
  - `token`: shared secret string
  - `allowLocalhostBypass`: defaults to `true`
- Protect **only** `/api/**` (including `/api/events` and WS upgrades) when `mode=token`.
- Keep `/health` and static assets public.
- Redact `accessControl.token` from UserSystemInfo / config responses.
- Frontend: optional token stored in `localStorage` (key `vk_api_token`) and automatically injected into:
  - HTTP requests via `Authorization: Bearer <token>` (fallback `X-API-Token`)
  - SSE/WS URLs via `?token=<token>` when headers are unavailable

## Impact
- New spec: `access-control-boundary`.
- Code areas: `crates/services` config schema/versions, `crates/server` HTTP middleware + stream auth, frontend API client + SSE/WS helpers, tests.
- Compatibility: default mode is `disabled` so behavior is unchanged unless users enable token mode.

