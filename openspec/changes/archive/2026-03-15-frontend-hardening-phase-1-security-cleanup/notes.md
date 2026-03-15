## Baseline (2026-03-14)

### Frontend audit (`pnpm -C frontend audit --prod`)

Summary: `high:7 moderate:4 low:4`

HIGH / MODERATE items targeted by this phase:

- HIGH `@remix-run/router` (patched `>=1.23.2`)
- MODERATE `react-router` (patched `>=6.30.2`)
- HIGH/MODERATE `devalue` (remove from prod graph or patch `>=5.6.4`)
- HIGH `glob` (patched `>=10.5.0`)
- HIGH `minimatch` (patched `>=9.0.7`)
- MODERATE `lodash` (patched `>=4.17.23`)
- MODERATE `lodash-es` (patched `>=4.17.23`)

Initial remediation plan (expected):

- Upgrade `react-router-dom` to ensure `react-router >=6.30.2` and `@remix-run/router >=1.23.2`.
- Upgrade `@tanstack/react-form` to a version that removes the `devalue` dependency.
- Upgrade `@git-diff-view/*` to bring in `diff >=8.0.3` (removes low advisory).
- Upgrade `lodash` (direct dep) to `>=4.17.23`.
- Remove unused prod dependency `@rjsf/shadcn` (removes `lodash-es` from prod audit graph).
- If remaining advisories are purely transitive, add minimal `pnpm.overrides` (e.g., `glob`, `minimatch`).

### Unused scan (`pnpm -C frontend dlx knip --reporter compact`)

Key actionable findings:

- Unused files (2):
  - `frontend/src/components/tasks/WorkspaceHookSummaryCard.tsx`
  - `frontend/src/hooks/task-attempts/useTaskAttemptStatus.ts`
- Unused dependencies:
  - `frontend/package.json`: `@rjsf/shadcn`, `wa-sqlite`
- Unused devDependencies:
  - `frontend/package.json`: `@vitest/coverage-v8`, `eslint-plugin-prettier`
  - `package.json`: `vite`
- Unresolved import:
  - `frontend/src/types/virtual-executor-schemas.d.ts`: `@/shared/types` (should use existing `shared/*` alias)

## Verification gates

- `pnpm -C frontend run check`
- `pnpm -C frontend run lint`
- `pnpm -C frontend run build`
- `pnpm -C frontend audit --prod`
- `pnpm -C frontend dlx knip --reporter compact`
- `pnpm run e2e:just-run`
