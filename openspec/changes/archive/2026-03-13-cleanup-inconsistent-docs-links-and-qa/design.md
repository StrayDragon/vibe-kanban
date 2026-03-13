## Context

This fork has diverged significantly from upstream, but several repo surfaces still carry upstream-facing assumptions:

- UI navigation links and onboarding dialogs contain hard-coded external docs URLs.
- E2E tests currently assume those external URLs exist and stub them for CI.
- Some generated copy/templates embed upstream URLs.
- An auxiliary Rust crate (`crates/review`) is hard-coded to upstream remote services and “upload code to hosted servers” messaging.
- README and docs contain dead references (for example referencing `docs/operations.md` which does not exist today).

Our desired posture is local-first and self-contained:

- Product UX should not depend on outbound network access for documentation or release notes.
- External links should be rare and explicit; per current policy we only keep upstream GitHub links.
- CI should prevent regressions (reintroducing upstream URLs or dead doc references).

## Goals / Non-Goals

**Goals:**
- Keep only an explicit allowlist of external links (upstream GitHub).
- Remove the external Release Notes flow and any hosted-service assumptions.
- Remove legacy UI compatibility redirects and upgrade callers to canonical routes.
- Remove the `crates/review` workspace crate (unsupported in this fork).
- Add deterministic repo QA guardrails (including a link-policy check).
- Inventory data-level legacy compatibility and defer deletion until we can prove it is safe.

**Non-Goals:**
- Adding an in-app Help page or developer operations docs in this change (defer until the product stabilizes).
- Changing core task/attempt orchestration behavior.
- Removing data compatibility paths (worktree layout migrations, legacy log persistence) in this change.
- Introducing new hosted infrastructure or new public docs hosting.
- Broad restructuring of the OpenSpec archive content.

## Decisions

### 1) No Docs/Help entry yet (remove external Docs links)

**Decision:** Remove the navbar “Docs” external URL from the UI for now. We will consider adding an in-app Help page and/or developer operations docs later once the product stabilizes.

**Why:** We do not want an unstable or misleading docs surface during rapid development, and we also do not want any UX dependency on external docs hosting.

**Alternatives considered:**
- Replace with an in-app Help route/modal now (rejected for this change: explicitly deferred).
- Keep an external docs link and make it configurable (rejected: still depends on outbound network and tends to drift).

### 2) External link policy (allowlist + guardrail)

**Decision:** Enforce a strict allowlist policy for external links used by product surfaces and tooling. Only URLs under `https://github.com/BloopAI/vibe-kanban` are allowed. Add a deterministic repo check (run in CI and locally) that fails if disallowed domains appear.

**Why:** Prevents reintroducing upstream hosted domains and keeps the fork’s docs and UX consistent.

**Alternatives considered:**
- “Best effort” manual review only (rejected: drift is frequent and hard to detect).
- Ban all external links (rejected: Apache-2.0 license text and upstream GitHub links are legitimate).

### 3) Release notes: disable external fetch, remove iframe UX

**Decision:** Remove the Release Notes dialog that loads an external iframe. On startup, if `config.show_release_notes` is set, clear it without showing external content.

**Why:** The current implementation is an upstream-hosted page and violates the “no external dependency for UX” goal.

**Alternatives considered:**
- Replace iframe with local static release notes content bundled with the frontend (deferred: requires ongoing content maintenance; can be added later if desired).

### 4) Remove `crates/review` from the Rust workspace

**Decision:** Remove `crates/review` from `Cargo.toml` workspace membership and delete any references to it in docs/scripts/CI.

**Why:** It embeds upstream remote-service defaults and messaging (“upload code to our servers”), which is incompatible with this fork’s local-first posture.

**Alternatives considered:**
- Keep the crate but require explicit `REVIEW_API_URL` configuration (rejected for this change: still a confusing surface and not part of supported workflows).

### 5) Data-level legacy compatibility: inventory now, delete later

**Decision:** Do not delete data compatibility logic in this change. Produce an inventory report that separates:
- UI/UX compat (safe to remove now)
- data/FS/log persistence compat (requires usage evidence and a dedicated change)

**Why:** Avoids accidental data loss or making existing `~/.vibe-kanban` state unreadable without a measured migration plan.

## Risks / Trade-offs

- **Breaking old bookmarks** → Remove legacy redirects; mitigate by ensuring all in-app navigation uses canonical routes and documenting the change.
- **Less change visibility (no release notes)** → Accept short-term; optionally add an in-app static “What’s new” later.
- **Guardrail false positives** → Keep the check allowlist-based and scope it to product surfaces (not the Apache license text or archived OpenSpec changes).
- **Workspace churn from removing `crates/review`** → Ensure no other crates depend on it and update any scripts referencing it.

## Migration Plan

- No DB migrations expected.
- Frontend config remains backward-compatible by *ignoring* the legacy `show_release_notes` trigger after clearing it once.
- Rollback is a normal git revert; no data migrations means no special rollback steps.

## Open Questions

- None.
