## 1. Boundaries & Guardrails

- [ ] 1.1 Define the API boundary allowlist (which files may call `fetch` / create WS/SSE) and document it in the repo.
- [ ] 1.2 Add ESLint restrictions that fail on `fetch` usage outside the allowlist (verify `pnpm -C frontend run lint`).
- [ ] 1.3 Add guardrails for ad-hoc React Query keys (lint rule or guard script) and verify it detects an intentionally introduced violation.

## 2. API Layer Consolidation

- [ ] 2.1 Consolidate ad-hoc `fetch` call sites into `frontend/src/api/**` and reuse shared request helpers (verify `pnpm -C frontend run check` and `pnpm -C frontend run build`).
- [ ] 2.2 Standardize error handling and token/query injection across all API modules (verify error surfaces via a small manual smoke or unit tests where applicable).

## 3. Server-State Keys & Invalidation

- [ ] 3.1 Introduce/standardize domain key factories (e.g., tasks/attempts/execution-processes) and migrate a small set of hooks to use them (verify `pnpm -C frontend run check`).
- [ ] 3.2 Centralize invalidation rules so event-driven invalidation and mutation invalidation use the same key sources (verify by running key flows and observing cache updates).

## 4. Realtime Primitives & Migration

- [ ] 4.1 Introduce a small realtime primitives layer (entity-map JSON-Patch stream + log stream wrapper) without changing behavior (verify existing e2e still passes).
- [ ] 4.2 Migrate one high-value flow end-to-end to the new boundaries (e.g., “create task” + “send follow-up”) and ensure mutation contract guarantees visible UI updates (verify via e2e).

## 5. Verification

- [ ] 5.1 Run frontend validation: `pnpm -C frontend run check`, `pnpm -C frontend run lint`, `pnpm -C frontend run build`.
- [ ] 5.2 Extend/verify e2e assertions for “mutation → visible UI change” invariants: `pnpm run e2e:just-run`.
