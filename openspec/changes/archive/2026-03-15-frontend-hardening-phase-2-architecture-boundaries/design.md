## Context

The frontend currently mixes multiple data sources and patterns:

- React Query for HTTP fetch + cache invalidation
- WebSocket JSON-Patch streams (`useJsonPatchWsStream`) for live entity maps (tasks, execution processes, projects, etc.)
- SSE event stream (`/api/events`) used for “invalidate by patch” behavior
- Ad-hoc `fetch` calls in hooks/utilities for specific endpoints (logs, image metadata, etc.)
- Local UI state via Zustand (some of it overlapping with server-state concerns)

This makes it difficult to guarantee consistent user-visible updates across flows (e.g., mutation succeeded but stream missed the event; different fetch calls handle auth/errors differently). It also makes frontend/backend coordination on realtime behavior harder because “where truth lives” varies by feature.

## Goals / Non-Goals

**Goals:**

- Establish enforceable boundaries:
  - A single API boundary for HTTP/WS/SSE creation and request helpers (token/auth/error normalization).
  - A single place to define query keys and invalidation rules.
- Make realtime + cache behavior observable and consistent:
  - A small set of primitives for JSON-Patch streams and resync behavior.
  - A consistent “mutation contract” that guarantees a visible UI effect (optimistic state + resync/invalidate + user feedback).
- Migrate incrementally without breaking product behavior.

**Non-Goals:**

- Backend protocol evolution (seq/resume/invalidation hints) (Phase 3).
- Large dependency/tooling upgrades (Phase 4).
- Broad UI redesign.

## Decisions

### 1) Define an explicit “network boundary”

**Decision:** Only modules under `frontend/src/api/**` (and a very small allowlist such as `frontend/src/utils/translation.ts` if needed) may call `fetch` directly or construct WS/SSE connections. All other code must use API functions.

**Rationale:** Centralizes token handling, error normalization, and request options. Prevents drift into inconsistent patterns.

**Alternatives considered:**

- Allow `fetch` in hooks: simpler short-term, preserves inconsistency and hidden auth/error differences.

### 2) Centralize “server-state boundary” and query keys

**Decision:** For each domain (tasks, attempts, execution processes, etc.), define a single key factory module (e.g., `*Keys`) and reuse it across hooks and invalidation logic.

**Rationale:** Prevents duplicated key shapes and accidental stale caches. Makes invalidation auditable.

**Alternatives considered:**

- Keep ad-hoc `queryKey: ['x', id]` scattered across code: hard to reason about and easy to break during refactors.

### 3) Introduce a small realtime primitives layer

**Decision:** Create/standardize primitives for:

- “Entity map via JSON-Patch stream” (snapshot + patch apply + dedupe + resync)
- “Append/replace log stream” (reconnect policies + finished semantics)
- “Invalidate by patch” (SSE patch to invalidations), with a path to upgrade to backend-provided invalidation hints in Phase 3

**Rationale:** Keeps realtime behavior consistent across features and makes Phase 3 protocol work a drop-in change (adapter swap), not a full refactor.

**Alternatives considered:**

- Keep per-feature stream logic: repeated bugs and harder backend coordination.

### 4) Standardize a “mutation UX contract”

**Decision:** Mutations that change server state MUST:

1) produce an immediate user-visible UI change (optimistic overlay and/or toast + loading state)
2) trigger a deterministic reconciliation step (invalidate/resync) to converge to canonical server state

**Rationale:** Guarantees “triggered → UI changed” even when streams are delayed/disconnected.

**Alternatives considered:**

- Rely on streams only: vulnerable to stream gaps, idle disconnects, and missed patches.

## Risks / Trade-offs

- **[Cross-cutting refactor regression]** → Mitigation: incremental migration by domain; keep adapters compatible; require `check/lint/build` and targeted e2e for each migrated flow.
- **[Guardrails too strict]** → Mitigation: start with a small allowlist and expand only when justified; document the boundary and provide clear patterns.
- **[Incomplete invalidation rules]** → Mitigation: centralize invalidation logic and add regression tests for key flows (“mutation → visible update”).

## Migration Plan

1) Introduce/confirm the API boundary module(s) and add lint restrictions for direct `fetch` outside the allowlist.
2) Add/standardize key factory modules per domain and update a small set of hooks to use them.
3) Consolidate stream primitives behind a single interface; migrate one stream consumer at a time.
4) For high-risk user flows (create task, send follow-up), enforce the mutation contract and add e2e assertions.
5) Repeat per domain until most of the app follows the new boundaries.

Rollback strategy: each migration step is a small PR; revert by domain if regressions occur.

Config versioning: no config schema changes are expected in this phase.

## Open Questions

- Which modules should be on the `fetch` allowlist (e.g., translation endpoint, image metadata), and can those be moved into `src/api/**` without large churn?
- Do we want a hard rule against using Zustand for server entities (allow only UI state + explicit optimistic overlays)?
