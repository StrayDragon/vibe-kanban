# Frontend Dependency Update Policy

This document describes how to update frontend dependencies safely in Vibe Kanban, with a focus on preventing security regressions and avoiding “silent” UI breakages.

## Goals

- Keep `pnpm -C frontend audit --prod` free of HIGH/MODERATE vulnerabilities (preferably 0 total).
- Remove unused code and dependencies before they become “latent” risk.
- Make updates repeatable and verifiable with local checks (CI not required).

## Daily/Weekly Checks

- Security (production graph): `pnpm run frontend:audit:prod`
- Unused code/deps scan: `pnpm run frontend:knip`
- Combined: `pnpm run guard:frontend-deps`

## Update Workflow (Recommended)

1) **Baseline**

- `pnpm -C frontend outdated`
- `pnpm -C frontend audit --prod`
- `pnpm -C frontend dlx knip --reporter compact`

2) **Prefer upgrades over overrides**

- First upgrade direct dependencies that pull in the vulnerable packages.
- Only add `pnpm.overrides` when the vulnerable package is purely transitive and upstream hasn’t released a fix yet.

3) **If you must use `pnpm.overrides`, keep it minimal**

- Target specific vulnerable versions (avoid forcing major upgrades across the tree).
- Re-run verification after every override change.
- Remove overrides when upstream dependencies naturally move to patched versions.

4) **Verification gates (non-negotiable)**

- `pnpm -C frontend run check`
- `pnpm -C frontend run lint`
- `pnpm -C frontend run build`
- `pnpm run e2e:just-run`

## Notes

- Overrides live in the workspace root `package.json` under `pnpm.overrides`.
- The frontend is a built SPA; runtime dependencies must be present at build time, so correctness depends on the build gates above.
