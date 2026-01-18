# Module: Frontend Data Layer

## Goals
- Centralize token injection for API/SSE/WS.
- Keep cache invalidation rules consistent and predictable.

## In Scope
- makeRequest wrapper in frontend/src/lib/api.ts.
- EventSource setup in frontend/src/contexts/EventStreamContext.tsx.
- WebSocket URLs for diff/log streams.

## Out of Scope / Right Boundary
- Global state management rewrite.
- Switching data libraries (React Query remains).

## Design Summary
- Read token from localStorage key vk_api_token (or env override).
- Attach Authorization header to fetch requests.
- Append token query param to SSE/WS URLs when present.

## Testing
- Unit tests for token injection helpers.
- Manual checks: API request works with/without token depending on mode.
