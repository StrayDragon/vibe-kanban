## Why

Even after security cleanup and architectural guardrails, the frontend can still inherit latent bugs and security risk from older build/test/tooling stacks and lagging dependency majors. Modernizing to newer stable versions reduces exposure to known issues, improves developer ergonomics, and makes future maintenance easier.

This phase focuses on controlled upgrades with clear verification gates and rollback.

## What Changes

- Upgrade frontend tooling to newer stable versions with minimal behavioral change:
  - Build: Vite (align workspace/tooling versions; remove version skew)
  - Test: Vitest + related tooling
  - Styling/build chain: Tailwind/PostCSS/autoprefixer (as feasible)
  - State/tooling libs: Zustand (major), key UI primitives (Radix minor/patch)
- Prefer “safe lane” upgrades first (patch/minor or widely-compatible majors), and isolate high-risk majors (e.g., React 19 / React Router 7) behind explicit evaluation and rollback steps.
- Ensure dependency upgrades do not reintroduce security advisories and do not regress e2e flows.

## Capabilities

### New Capabilities

- `frontend-tooling-modernization`: A verified, up-to-date frontend toolchain (build/test/style/state) with a repeatable upgrade/verification process.

### Modified Capabilities

- (none)

## Impact

- Frontend build/test/config:
  - `frontend/vite.config.ts`, `frontend/tsconfig*.json`, Tailwind/PostCSS configs
  - `frontend/package.json`, `pnpm-lock.yaml`
- Developer workflows:
  - `pnpm run check`, `pnpm run lint`, `pnpm run e2e:just-run`

## Goals

- Frontend continues to build, typecheck, lint, and pass e2e after upgrades.
- Tooling versions are aligned and documented; “version skew” between workspace and frontend is removed where possible.
- Security baseline remains healthy (no reintroduced HIGH/MODERATE advisories in production dependencies).

## Non-goals

- Major product behavior changes or UI redesign.
- Backend protocol work (Phase 3).
- Large architectural re-org (Phase 2).

## Risks

- Tooling majors can introduce subtle bundling/runtime differences (Vite/Tailwind).
- React ecosystem majors (React 19, Router 7) may require coordinated upgrades across multiple libraries.
- Upgrades can increase churn in lockfile and make bisects harder without phased commits.

## Verification

- `pnpm -C frontend run check`
- `pnpm -C frontend run lint`
- `pnpm -C frontend run build`
- `pnpm run e2e:just-run`
- `pnpm -C frontend audit --prod` (no HIGH/MODERATE)
