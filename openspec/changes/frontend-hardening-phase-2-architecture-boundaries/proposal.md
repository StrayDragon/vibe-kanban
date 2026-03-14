## Why

Frontend reliability issues (e.g., “triggered but no UI change”) are amplified by unclear module boundaries and mixed data-access patterns (React Query + multiple WS/SSE streams + ad-hoc `fetch`). This makes consistency bugs harder to prevent and harder to debug.

This phase creates a clear, enforceable frontend architecture so the UI and backend can coordinate on realtime data flows with fewer hidden failure modes.

## What Changes

- Introduce strict boundaries for network access and server-state synchronization:
  - Centralize HTTP/WS/SSE access behind a single API layer.
  - Consolidate query keys and invalidation rules.
- Reorganize frontend modules into a predictable structure (feature/domain slices + shared primitives), with incremental migration (no “big bang”).
- Add guardrails (lint rules / forbidden imports) to prevent regressions back to ad-hoc `fetch` and duplicated caches.
- Improve mutation UX invariants so user-triggered actions always produce a visible UI update (optimistic overlays/toasts/resync patterns), consistently.

## Capabilities

### New Capabilities

- `frontend-architecture-boundaries`: Define and enforce module boundaries for data access, realtime streams, and server-state caching so UI updates are consistent and observable.

### Modified Capabilities

- (none)

## Impact

- Frontend code organization and conventions:
  - `frontend/src/api/*` and any existing re-export layers
  - `frontend/src/hooks/*`, `frontend/src/contexts/*`, `frontend/src/stores/*`
  - mutation patterns and UI feedback (toast/optimistic/resync)
- Tooling guardrails:
  - ESLint rules / restricted imports to prevent direct `fetch` in components/hooks.

## Goals

- All network calls (HTTP + WS/SSE setup) are routed through a defined API boundary; components/hooks no longer call `fetch` directly.
- Query key usage is consistent (single source of truth) and invalidations are centralized.
- For key user mutations (create task, send follow-up, etc.), UI SHALL show a visible state change even if streams are delayed (loading/optimistic/toast/resync).
- Refactor is incremental with tight verification; no broad dependency upgrades required in this phase.

## Non-goals

- Protocol-level changes to backend realtime streams (seq/resume/invalidation hints) (Phase 3).
- Major dependency/tooling upgrades (React 19, Tailwind 4, Vite 8) (Phase 4).
- Security baseline remediation for production deps (Phase 1).

## Risks

- Large cross-cutting refactor could introduce regressions if migrated too broadly at once.
- Over-constraining architecture could slow down iteration if guardrails are too rigid.
- Incorrectly centralizing data can cause stale caches if invalidation rules are incomplete.

## Verification

- Type/lint/build:
  - `pnpm -C frontend run check`
  - `pnpm -C frontend run lint`
  - `pnpm -C frontend run build`
- Behavioral:
  - Extend existing e2e tests to assert “mutation → visible UI change” invariants.
  - Validate realtime stream recovery paths still resync after disconnects.
