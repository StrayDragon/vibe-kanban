## Context

The frontend depends on a multi-layer toolchain (Vite, TS, ESLint, Vitest, Tailwind/PostCSS, state libraries). Over time, version skew and lagging majors can:

- reintroduce known bugs and security issues via transitive deps,
- make upgrades harder (bigger jumps later),
- reduce developer ergonomics and increase “works on my machine” variance.

This phase modernizes the tooling stack deliberately, with verification gates and rollback plans, and separates low-risk upgrades from high-risk ecosystem majors.

## Goals / Non-Goals

**Goals:**

- Upgrade to newer stable versions of the frontend toolchain while keeping the product behavior stable.
- Remove “version skew” where it adds confusion (e.g., align Vite usage so the repo has a clear, single story).
- Keep the security baseline intact (no reintroduced HIGH/MODERATE production advisories).
- Keep upgrades incremental and verifiable.

**Non-Goals:**

- Backend protocol changes (Phase 3).
- Large frontend architecture re-org (Phase 2).
- Mandatory migration to the newest possible majors if the ecosystem is not ready (e.g., forcing React 19 / Router 7 if key deps are incompatible).

## Decisions

### 1) Two-lane upgrade strategy (safe lane vs. high-risk lane)

**Decision:** Split upgrades into:

- **Safe lane:** patch/minor upgrades and widely-compatible majors (e.g., Vitest major, Zustand major) where ecosystem support is mature and migration cost is bounded.
- **High-risk lane:** ecosystem majors that often require coordinated upgrades (React 19, React Router 7, Tailwind 4). These require explicit evaluation, compatibility checks, and rollback steps.

**Rationale:** Avoids blocking the whole modernization effort on a single ecosystem-wide breaking change.

### 2) Upgrade order prioritizes build correctness and observability

**Decision:** Upgrade in this order:

1) build/test tooling (Vite, Vitest, TS-related tooling)
2) lint/tooling (ESLint, plugins)
3) state libraries (Zustand)
4) styling toolchain (Tailwind/PostCSS) last, due to UI drift risk

**Rationale:** Build/test correctness gates provide fast feedback before higher-churn UI toolchain changes.

### 3) Verification gates are non-negotiable

**Decision:** Every upgrade step must pass:

- `pnpm -C frontend run check`
- `pnpm -C frontend run lint`
- `pnpm -C frontend run build`
- `pnpm run e2e:just-run`

**Rationale:** Tooling upgrades are notorious for subtle regressions; consistent gates reduce risk.

## Risks / Trade-offs

- **[Toolchain major introduces runtime differences]** → Mitigation: keep changes small; run e2e; validate “just run” flow; use rollback-friendly commits.
- **[Tailwind major causes visual drift]** → Mitigation: isolate Tailwind upgrade; add visual spot-check checklist; defer if too disruptive.
- **[React ecosystem majors require coordinated upgrades]** → Mitigation: treat as high-risk lane; do a compatibility spike before committing to the upgrade.

## Migration Plan

1) Establish current baseline versions and verification results.
2) Apply safe-lane upgrades in small batches (1–3 packages at a time) with full verification.
3) For each high-risk lane candidate, run a dedicated compatibility spike:
   - identify required peer upgrades
   - estimate migration effort
   - decide “go/no-go” based on e2e + maintenance benefits
4) Document the final toolchain versions and upgrade procedure.

Rollback strategy: revert the smallest failing batch; keep the lockfile diff scoped per step.

Config versioning: no user config migrations are expected.

## Baseline & Targets (2026-03-14)

Baseline and upgrade candidates were captured via `pnpm -C frontend outdated --format json`.

### Safe-lane targets (planned)

- **Build:** `vite` `5.4.19` → `8.0.0`; `@vitejs/plugin-react` `4.5.2` → `6.0.1`
- **Test:** `vitest` `1.6.1` → `4.1.0`; `jsdom` `24.1.3` → `28.1.0`
- **Lint:** `eslint` `8.57.1` → `10.0.3`; `@typescript-eslint/*` `6.21.0` → `8.57.0`; `eslint-plugin-react-hooks` `4.6.2` → `7.0.1`; `eslint-plugin-check-file` `2.8.0` → `3.3.1`
- **Core tooling:** `typescript` `5.9.2` → `5.9.3`; `postcss` `8.5.6` → `8.5.8`; `autoprefixer` `10.4.21` → `10.4.27`
- **State:** `zustand` `4.5.7` → `5.0.11`

### High-risk lane candidates (spike only)

- **Styling:** `tailwindcss` `3.4.17` → `4.2.1`
- **React ecosystem:** `react`/`react-dom` `18.3.1` → `19.2.4`; `react-router-dom` `6.30.3` → `7.13.1`

## Open Questions

- Do we want to standardize on a single “supported Node version” beyond `>=18` for more reproducible builds?
- Should Tailwind 4 be part of Phase 4 or split into a dedicated follow-up change if UI drift is significant?
