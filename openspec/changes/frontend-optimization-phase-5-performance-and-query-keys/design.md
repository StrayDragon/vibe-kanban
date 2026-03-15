## Context

The frontend relies on React Query for server-state caching and on a mix of hooks/pages/components to declare queries and invalidate caches after mutations. Today, a small number of modules still declare ad-hoc `queryKey: [...]` arrays (and some keys are inconsistently named), which makes invalidation brittle and is a common root cause of “mutation succeeds but UI doesn’t change” reports.

In addition, the router currently imports all route modules synchronously from `frontend/src/app/AppRouter.tsx`, producing a single large entry chunk in `frontend/dist/assets/index-*.js`. This is a straightforward opportunity for route-level lazy loading with low functional risk.

Constraints:
- Keep changes incremental; avoid major framework upgrades.
- Do not change backend APIs unless a frontend-blocking defect is discovered.
- Keep the existing verification gates (`pnpm run qa`, `pnpm run e2e:just-run`) stable.

## Goals / Non-Goals

**Goals:**
- Eliminate inline `queryKey: [...]` arrays across `frontend/src/**` by migrating to domain key factories.
- Ensure invalidation targets match query declaration keys (no mismatched naming).
- Extend the lint guardrail so pages/components cannot reintroduce ad-hoc query keys.
- Reduce initial bundle size via route-level code splitting with predictable loading fallbacks.
- Add/extend e2e coverage to detect “no visible UI change after mutation” regressions.

**Non-Goals:**
- React/Router/Tailwind major upgrades.
- Replacing React Query/Zustand or redesigning the UI.
- Large cross-cutting refactors unrelated to query keys / routing / UI-update correctness.

## Decisions

### 1) Canonical key factories live in `frontend/src/query-keys/**`

Create/extend key factory modules per domain (kebab/camel naming consistent with existing `*Keys.ts` files) and migrate call sites to import them.

Rationale:
- Makes keys discoverable and avoids “almost the same key” drift.
- Allows invalidation rules to be expressed in the same module (future-proofing).

Alternatives considered:
- Keep key factories colocated in hooks (status quo). Rejected because pages/components currently bypass the guardrail and key drift continues.
- Enforce only via conventions without lint. Rejected because regressions have already occurred.

### 2) Guardrails apply to all `frontend/src/**/*.{ts,tsx}`, not just hooks

Update `frontend/eslint.config.mjs` so `no-restricted-syntax` also blocks inline `Property[key.name="queryKey"] > ArrayExpression` in pages/components, and ensure overrides (dialogs/API boundary) still carry the query-key restriction.

Rationale:
- The remaining violations are outside `src/hooks/**`; limiting enforcement to hooks is insufficient.
- ESLint is already used for similar “API boundary” and modal guardrails.

### 3) Fix key mismatches by choosing one canonical string per domain key

When migrating, prefer existing canonical keys in `frontend/src/query-keys/**` (e.g., `taskAttemptKeys.attemptWithSession`) and delete/stop using divergent ad-hoc names (e.g., `attemptWithSession`).

Rationale:
- This directly targets the “invalidate misses” class of bugs.

### 4) Route-level code splitting via `React.lazy` + `Suspense` in `AppRouter`

Convert top-level page imports (and optionally settings subroutes) to lazy imports. Provide a consistent `Suspense` fallback that matches the current loading UX (reuse `Loader`).

Rationale:
- Low-risk: routing boundaries already exist and pages are standalone.
- High impact: transforms the single large entry chunk into multiple smaller chunks.

Alternatives considered:
- Rollup `manualChunks` only. Rejected as a primary strategy because it is less explicit than route-lazy loading and harder to reason about long-term.
- Introduce a bundle-analyzer dependency first. Optional; defer unless needed for targeted follow-ups.

### 5) Verification is primarily e2e + lint, not runtime instrumentation

Add/extend Playwright tests for the “mutation must cause visible UI change” flows. Avoid adding runtime perf instrumentation unless a specific regression requires it.

Rationale:
- The reported failures are correctness/consistency issues first; e2e is the best regression net.

## Risks / Trade-offs

- [Lint churn] Broadening the rule may cause a burst of edits. → Mitigation: migrate all existing call sites in the same PR as the rule expansion.
- [Cache miss after key rename] Some keys may change their string identifiers. → Mitigation: accept refetch on next render; ensure queries have correct `enabled` guards and deterministic `queryFn`.
- [Lazy-loading UX regressions] Missing fallbacks can feel “blank”. → Mitigation: wrap route elements with a shared fallback; keep layout/root providers eager.
- [Over-splitting] Too many tiny chunks can increase request overhead. → Mitigation: start with route-level splitting only; avoid micro-splitting until measured.

## Migration Plan

1. Add missing key factories in `frontend/src/query-keys/**`.
2. Migrate all remaining inline `queryKey: [...]` and invalidation call sites under `frontend/src/**` to use the factories.
3. Expand ESLint guardrails to enforce the rule across `frontend/src/**/*.{ts,tsx}` and update overrides accordingly.
4. Add/extend e2e specs covering create-task and follow-up visibility (no manual refresh).
5. Implement route-level lazy loading in `frontend/src/app/AppRouter.tsx` with consistent `Suspense` fallbacks.
6. Re-run `pnpm run qa` and `pnpm run e2e:just-run` for final validation.

Rollback:
- Query-key changes are local to the frontend; rollback is a git revert.
- Lazy-loading can be reverted by restoring static imports if a critical route breaks.

## Open Questions

- Do we want a hard “bundle budget” check (numeric threshold) in QA, or keep it as an informational guardrail initially?
- Which settings subroutes are safe to lazy-load without impacting onboarding/modals (likely yes, but confirm in e2e)?
