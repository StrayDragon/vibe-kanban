## Why

This repo has diverged heavily from upstream, but some docs, UI copy, hard-coded URLs, and auxiliary tooling still reference upstream branding/services (for example `vibekanban.com`, `api.vibekanban.com`, and `review.fast`) or describe workflows that no longer match our fork. This creates user confusion, broken links, and inconsistent expectations during onboarding and QA.

We want a local-first, self-contained product surface: remove external docs entry points, avoid any hidden “phone home” behavior, and add CI guardrails that prevent these inconsistencies from creeping back in. (In-app Help and developer-facing operations docs are explicitly deferred until the product stabilizes.)

## What Changes

- Remove or replace misleading docs/product copy that assumes upstream infrastructure or public hosted docs.
- Remove external docs entry points from the UI and keep only the upstream GitHub links we explicitly allow (for example upstream issues).
- Remove the external Release Notes flow (iframe to upstream site) and any other hard-coded external docs URLs.
- **BREAKING**: remove legacy/compat UI redirects that only exist for old bookmarked URLs (upgrade callers/routes to the current canonical paths).
- **BREAKING**: remove the `review` CLI crate from the Rust workspace (it is hard-coded to upstream remote services and is not part of our fork’s supported local-first workflow).
- Add repo-level QA guardrails:
  - a single command to run the “CI-equivalent” checks locally
  - a deterministic check that fails CI if disallowed external links reappear (allowlist-based)
- Produce a cleanup inventory report that:
  - lists remaining “legacy/compat” surfaces
  - separates UI/UX compatibility from data compatibility
  - documents a follow-up deletion plan for data-level legacy only after usage is proven low/zero

## Goals

- No hard-coded references to `vibekanban.com`, `api.vibekanban.com`, or `review.fast` remain in product surfaces and tooling.
- The app does not rely on external docs sites for core UX flows.
- CI reliably prevents reintroducing disallowed external links and drifted docs references.
- Legacy UI compatibility is removed decisively (upgrade to the current behavior “in one step”).
- Data-level compatibility removals are explicitly *not* done blindly; we first inventory and define safe criteria for removal.

## Non-goals

- Large refactors of core task/attempt/runtime orchestration.
- Rebranding to a new public domain or adding new external documentation hosting.
- Removing data-layer legacy (for example workspace layout migrations or legacy log persistence fallbacks) in this change.

## Risks

- Removing legacy routes/redirects may break old bookmarks; mitigate by updating internal navigation to canonical paths and documenting the change.
- Removing Release Notes may reduce visibility into changes; mitigate by keeping release notes in-repo (for example `CHANGELOG.md`) and explicitly deferring any in-app “What’s new” until later.
- Removing the `review` crate may break workflows for anyone relying on it; mitigate by documenting removal and ensuring no supported workflow depends on it.
- Overly strict link guardrails could block legitimate references; mitigate with an explicit allowlist (upstream GitHub links) and clear failure messages.

## Verification

- Run the existing CI checks locally:
  - `pnpm run lint`
  - `pnpm run check`
  - `cargo test --workspace`
  - `pnpm run e2e:test`
- Verify link policy:
  - the repo-level link guardrail passes
  - the UI has no “Docs” entry (Help/docs deferred)
  - “Support” still opens the upstream GitHub issues in a new tab
- Smoke test the onboarding flow:
  - Disclaimer shows no external docs link
  - Release notes are not shown / do not attempt external fetch

## Capabilities

### New Capabilities
- `docs-and-link-hygiene`: remove external documentation entry points and enforce a strict external link allowlist for UI, docs, and tooling.
- `qa-baseline`: a repeatable “CI-equivalent” QA command and deterministic guardrails that fail on drift.

### Modified Capabilities
- None.

## Impact

- **Frontend:** navigation menu links, onboarding dialogs (disclaimer/release notes), and e2e tests that validate external link behavior.
- **Backend:** generated PR templates / copy and any server-side templates that embed external URLs.
- **Workspace:** Rust workspace membership (removal of `crates/review`).
- **Docs:** remove dead references and keep fork messaging out of README (NOTICE keeps required attribution).
- **CI:** add a stable link-policy check so drift is caught before merge.
