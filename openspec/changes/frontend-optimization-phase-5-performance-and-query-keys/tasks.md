## 1. Query Key Factory Coverage

- [x] 1.1 Add missing query-key factories under `frontend/src/query-keys/**` (archived kanbans, user-system/config, project lifecycle hook outcomes, repo script lookups)
- [x] 1.2 Replace inline `queryKey: [...]` in `frontend/src/pages/ProjectArchives.tsx` and `frontend/src/pages/ProjectArchiveDetail.tsx` with imported key factories
- [x] 1.3 Replace inline `queryKey: [...]` / invalidations in `frontend/src/pages/settings/ProjectSettings/ProjectSettings.tsx` with imported key factories
- [x] 1.4 Replace inline `queryKey: [...]` / invalidations in `frontend/src/components/ConfigProvider.tsx` with imported key factories
- [x] 1.5 Replace mismatched `attemptWithSession` query key usage in `frontend/src/components/NormalizedConversation/NextActionCard.tsx` with `taskAttemptKeys.*` and verify invalidation targets match
- [x] 1.6 Replace inline `queryKey: [...]` in `frontend/src/components/tasks/TaskFollowUpSection.tsx` with a stable key factory (ensure repo-id list is deterministic)
- [x] 1.7 Replace inline invalidation keys in `frontend/src/components/dialogs/tasks/RemoveWorktreeDialog.tsx` with `taskAttemptKeys.*`

## 2. Guardrails: Enforce No Inline `queryKey` Arrays Across `frontend/src`

- [x] 2.1 Update `frontend/eslint.config.mjs` so the inline `queryKey: [...]` restriction applies to pages/components (not only `src/hooks/**`) while preserving modal/network boundary overrides
- [x] 2.2 Run `pnpm -C frontend run lint` and confirm no inline `queryKey` arrays remain under `frontend/src/**/*.{ts,tsx}`

## 3. Performance: Route-Level Code Splitting

- [x] 3.1 Convert page imports in `frontend/src/app/AppRouter.tsx` to `React.lazy` + `Suspense` with a consistent fallback loader
- [x] 3.2 Validate `pnpm -C frontend run build` emits code-split chunks (more than one JS file under `frontend/dist/assets/`)
- [x] 3.3 Run a minimal UI smoke: navigate to `/tasks`, `/projects/:id/archives`, `/settings/projects`, and confirm routes render and dialogs still open

## 4. Regression Tests: Mutation → Visible UI Change

- [x] 4.1 Add/extend Playwright coverage so “create task” is immediately visible without manual refresh
- [x] 4.2 Add/extend Playwright coverage so “send follow-up message” is immediately visible and reconciles after resync/invalidation
- [x] 4.3 Run `pnpm run e2e:just-run` (at least 2 seeds) and confirm stability

## 5. Final QA

- [x] 5.1 Run `pnpm run qa` on a clean working tree and confirm all gates pass
