## Why

Users still report cases where core mutations (send follow-up message, create task, etc.) complete successfully but the UI does not visibly change until a manual refresh. In practice this is usually caused by inconsistent React Query keys and invalidation targets (ad-hoc `queryKey` arrays and mismatched naming across pages/components/hooks), which breaks the “mutations must cause visible UI change” contract.

In parallel, the production frontend build currently bundles most routes into a single large JS chunk, which increases startup cost and makes regressions harder to spot. This phase focuses on tightening the React Query boundary (keys + invalidation) and applying low-risk route-level code splitting to improve performance without major framework upgrades.

## What Changes

- Centralize remaining ad-hoc React Query keys:
  - Introduce missing domain key factories (e.g., archived kanbans, user system/config, project lifecycle hook outcomes, repo script lookups).
  - Replace all inline `queryKey: [...]` arrays and invalidation calls across `frontend/src/**` with imports from key factories.
  - Fix inconsistent key naming that causes invalidation misses (for example `attemptWithSession` vs `taskAttemptWithSession`).
- Strengthen guardrails so regressions cannot reappear:
  - Extend the existing ESLint restriction to prevent inline `queryKey` arrays outside `frontend/src/hooks/**` (pages/components must also comply).
  - Keep an explicit escape hatch: if a one-off key is required, it must be introduced via a domain key factory module rather than inline arrays.
- Improve mutation UX reliability:
  - Ensure the “immediate visible UI change” requirement holds for the key user flows (create task, send follow-up).
  - Add/extend e2e coverage to catch “mutation succeeds but UI doesn’t change” regressions.
- Reduce initial bundle weight via low-risk code splitting:
  - Route-level lazy-loading for heavier pages/settings sections to avoid bundling everything into the entry chunk.

## Capabilities

### New Capabilities

- `frontend-performance-guardrails`: Route-level code splitting and lightweight build-time checks to prevent large-bundle regressions.

### Modified Capabilities

- `frontend-architecture-boundaries`: Expand query-key guardrails beyond hooks so pages/components cannot introduce ad-hoc keys; align invalidation with domain key factories to preserve mutation-to-UI consistency.

## Impact

- Frontend:
  - Query keys: `frontend/src/query-keys/**` and call sites across `frontend/src/pages/**` and `frontend/src/components/**`
  - Guardrails: `frontend/eslint.config.mjs`
  - Router/perf: `frontend/src/app/AppRouter.tsx` (route-level lazy imports) and any shared loading UI
- Tests:
  - Playwright e2e: `e2e/*.spec.ts` (add/adjust scenarios for create-task + follow-up visibility)

## Goals

- No user-triggered mutation completes without an immediate visible UI change for the covered flows (create task, send follow-up).
- A single source of truth for React Query keys per domain, used consistently for `useQuery` and invalidation.
- Lint prevents reintroducing inline `queryKey: [...]` arrays anywhere under `frontend/src/**`.
- Production build uses route-level code splitting to reduce the entry chunk size without major dependency upgrades.
- `pnpm run qa` and `pnpm run e2e:just-run` remain stable and deterministic.

## Non-goals

- Major framework/library upgrades (React 19, React Router 7, Tailwind 4).
- Backend protocol changes or new server endpoints (unless a clear frontend-blocking bug is discovered).
- Replacing React Query/Zustand or redesigning the UI.

## Risks

- Broadening the lint rule can create churn; the migration must land atomically with the guardrail expansion.
- Query-key renames can temporarily create cache misses (acceptable; data should refetch deterministically).
- Route-level code splitting can surface missing loading states; ensure consistent fallbacks and no layout flicker.

## Verification

- `pnpm -C frontend run lint`
- `pnpm -C frontend run check`
- `pnpm -C frontend run build` (confirm multiple chunks and reduced entry chunk)
- `pnpm run e2e:just-run`
- `pnpm run qa`
