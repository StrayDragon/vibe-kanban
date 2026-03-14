## Context

The frontend currently carries:

- Production dependency vulnerabilities (HIGH/MODERATE) as reported by `pnpm -C frontend audit --prod`.
- Dead/unused code paths, unused dependencies, and tooling issues (e.g., unresolved imports) that make it harder to reason about runtime behavior and increase regression risk.

This phase is intentionally constrained to “hardening and cleanup” with measurable outcomes. Larger refactors (frontend architecture re-org, realtime protocol changes, major framework/tooling jumps) are explicitly deferred to later phases.

## Goals / Non-Goals

**Goals:**

- Eliminate HIGH/MODERATE production dependency vulnerabilities in the frontend, using upgrades where possible and minimal overrides when necessary.
- Remove unused dependencies/files discovered by static analysis, and fix tooling import/path issues uncovered by the scan.
- Add local guardrails (scripts + docs) so the security/cleanliness baseline is easy to re-check and less likely to regress.

**Non-Goals:**

- Re-architect frontend data flow/state management or reorganize feature/module boundaries (Phase 2).
- Change backend/frontend realtime streaming protocols (Phase 3).
- Upgrade to major new framework/tooling generations (React 19, Tailwind 4, Vite 8) (Phase 4).

## Decisions

### 1) Prefer “top-level upgrades” before transitive overrides

**Decision:** Fix vulnerabilities by upgrading first-party dependencies (or their direct dependents) whenever a compatible version exists; use `pnpm.overrides` only when upstream versions lag or the vulnerable package is purely transitive.

**Why:** Upgrades reduce long-term maintenance burden and avoid subtle version skew. Overrides are a useful last resort but can conceal incompatibilities if not verified.

**Alternatives considered:**

- Always override transitive deps: faster in the short term, higher risk of skew and harder upgrades later.
- Ignore non-critical advisories: increases risk and does not meet the hardening objective.

### 2) Targeted upgrades that remove whole vulnerable subtrees

**Decision:** Prioritize dependency upgrades that eliminate entire vulnerable chains, not just patch individual advisories.

Examples expected in this repo:

- Upgrading `@tanstack/react-form` to a version that no longer depends on `devalue` (removes multiple advisories at once).
- Upgrading `@git-diff-view/*` to bring in a patched `diff` version.
- Upgrading `react-router`/`react-router-dom` to patched versions to address router advisories.

**Alternatives considered:**

- Patch each transitive dependency individually: more work, higher risk of repeated drift.

### 3) Remove unused dependencies/files only when usage is provably absent

**Decision:** Use Knip findings as the initial candidate list, but validate removals against build/lint/typecheck and runtime smoke.

**Why:** Some modules may be “side-effect only” or referenced in ways static analysis can miss.

**Alternatives considered:**

- Manual cleanup without tooling: slow and error-prone.
- Mass deletion based purely on static report: increases risk of runtime breakage.

### 4) Normalize type imports and resolve alias usage

**Decision:** Fix unresolved import paths in `.d.ts` and other declarations to align with existing Vite/TS aliases (e.g., `shared/*`), and avoid creating new alias patterns in this phase.

**Why:** Tooling correctness is prerequisite for meaningful static analysis.

## Risks / Trade-offs

- **[Dependency upgrade regression]** → Mitigation: keep upgrades as small as possible (patch/minor where feasible), run `check/lint/build`, and run the existing local e2e smoke if relevant.
- **[Override skew]** → Mitigation: prefer upgrades; if overrides are needed, pin narrowly and verify with lockfile diff + build/lint/typecheck.
- **[False-positive unused code]** → Mitigation: remove incrementally; validate each removal with `pnpm -C frontend run build` and basic navigation smoke.

## Migration Plan

- This phase is a repo-local change (dependencies, scripts, unused cleanup). No data migrations are expected.
- Rollback strategy: revert the dependency/cleanup commits and restore the previous lockfile.
- Config versioning: no config schema changes are expected in this phase.

## Open Questions

- Do we want a hard policy on LOW advisories in production deps (e.g., allowlist vs. “fix opportunistically”)?
- Should Knip be run via `pnpm dlx` (no dep added) or added as a devDependency for consistent versioning?
