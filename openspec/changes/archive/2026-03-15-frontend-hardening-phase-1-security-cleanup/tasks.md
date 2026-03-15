## 1. Baseline & Guardrails

- [x] 1.1 Capture current frontend audit output (`pnpm -C frontend audit --prod`) and record the HIGH/MODERATE items targeted by this phase.
- [x] 1.2 Run unused-code scan (`pnpm -C frontend dlx knip --reporter compact`) and record unused files/deps targeted by this phase.
- [x] 1.3 Add/adjust local scripts (no CI required) for `audit` and `knip` so the checks are easy to rerun and documented.

## 2. Dependency Remediation (Security)

- [x] 2.1 Upgrade router deps to patched versions (verify via `pnpm -C frontend audit --prod` that `react-router` / `@remix-run/router` advisories are cleared).
- [x] 2.2 Upgrade form deps to eliminate `devalue` in production (verify `pnpm -C frontend audit --prod` no longer reports `devalue` advisories).
- [x] 2.3 Upgrade diff viewer deps to pull in patched `diff` (verify `pnpm -C frontend audit --prod` no longer reports `diff` advisory).
- [x] 2.4 Upgrade `lodash`/`lodash-es` to patched versions (verify via `pnpm -C frontend audit --prod`).
- [x] 2.5 If any remaining HIGH/MODERATE advisories are purely transitive, add minimal `pnpm.overrides` and re-verify with `pnpm -C frontend audit --prod`.

## 3. Dead Code Pruning

- [x] 3.1 Remove unused frontend dependencies identified by the scan (verify `pnpm -C frontend dlx knip --reporter compact` no longer reports them).
- [x] 3.2 Remove unused frontend source files identified by the scan (verify `pnpm -C frontend run build` and route smoke still work).
- [x] 3.3 Fix unresolved imports/types discovered by tooling (verify `pnpm -C frontend run check` and `pnpm -C frontend run build`).

## 4. Verification & Documentation

- [x] 4.1 Run frontend validation: `pnpm -C frontend run check`, `pnpm -C frontend run lint`, `pnpm -C frontend run build`.
- [x] 4.2 Re-run security + unused checks: `pnpm -C frontend audit --prod` (0 HIGH/MODERATE) and `pnpm -C frontend dlx knip --reporter compact` (no targeted unused items).
- [x] 4.3 Run a local UI smoke (and e2e if available): `pnpm run e2e:just-run` (or document why skipped).
- [x] 4.4 Update docs/notes for the dependency update policy (what to upgrade first, when to override, and required verification commands).
