## Why

Frontend stability and security issues are currently amplified by (a) production dependency vulnerabilities and (b) dead/unused code paths and modules that obscure the true runtime behavior. This increases the chance of regressions (e.g., “triggered but no UI change”), slows down debugging, and exposes avoidable security risk.

This phase focuses on measurable hardening (dependency security baseline + dead code pruning) without taking on broad architectural refactors.

## What Changes

- Reduce production dependency risk by upgrading or overriding vulnerable transitive deps until `pnpm -C frontend audit --prod` has **no HIGH/MODERATE** findings.
- Remove clearly-unused frontend files and dependencies, and fix unresolved imports/types so build and tooling reflect real runtime state.
- Add lightweight local guardrails so dead-code and vulnerability drift is detected early (developer workflow; no CI requirement).
- Document a repeatable “update policy” for frontend dependencies (what is safe to update, when to pin/override, and how to verify).

## Capabilities

### New Capabilities

- `frontend-security-cleanup`: Establish a security baseline and cleanup rules for the frontend dependency graph and codebase (audit/knip gates, safe upgrade patterns, and verification steps).

### Modified Capabilities

- (none)

## Impact

- Frontend package graph and build tooling:
  - `frontend/package.json`, `pnpm-lock.yaml`
  - Vite and type declarations (e.g., `frontend/vite.config.ts`, `frontend/src/types/*.d.ts`)
- Developer workflow (local-only):
  - New/updated scripts for `audit` and “unused code” checks (e.g., Knip) and guidance for dependency updates.

## Goals

- `pnpm -C frontend audit --prod` reports **0 HIGH / 0 MODERATE** vulnerabilities.
- Remove unused dependencies and files identified by static analysis (Knip), and resolve any toolchain import/path issues uncovered by the scan.
- Keep changes incremental and low-risk: no product behavior changes beyond improved reliability and safety.

## Non-goals

- Major frontend architecture re-org (feature slices, new data layer, etc.). That is Phase 2.
- Realtime protocol evolution (seq/resume/invalidation hints). That is Phase 3.
- Framework/tooling major jumps (React 19, Tailwind 4, Vite 8). That is Phase 4.

## Risks

- Dependency upgrades may introduce subtle runtime regressions (especially UI, routing, diff rendering, form behavior).
- Overriding transitive dependencies can create version skew if not carefully verified.
- Removing “unused” exports/files can break dynamic imports or “side-effect only” modules if detection is wrong.

## Verification

- Security:
  - `pnpm -C frontend audit --prod` (must have 0 HIGH/MODERATE)
- Dead code / unused deps:
  - `pnpm -C frontend dlx knip --reporter compact` (must not report unused files/dependencies relevant to this phase)
- Frontend correctness smoke:
  - `pnpm -C frontend run check`
  - `pnpm -C frontend run lint`
  - `pnpm -C frontend run build`
