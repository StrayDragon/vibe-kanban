# Cleanup Inventory (2026-03-13): Inconsistent Docs/Links/QA Drift

This report inventories current drift sources (docs, URLs, legacy compatibility) and defines the link policy to enforce going forward.

## External link policy (product surfaces)

### Allowed

- Only upstream GitHub links under: `https://github.com/BloopAI/vibe-kanban...`

### Disallowed (examples found in repo)

- `vibekanban.com` (including `www.vibekanban.com`)
- `api.vibekanban.com`
- `review.fast`

### Guardrails

- External link guardrail: `scripts/check-external-links.js` (added in this change)
- Docs internal-link guardrail: `scripts/check-doc-links.js` (added in this change)

## Current disallowed references (to remove/replace)

> Note: paths below reflect the state before applying the cleanup tasks. Each item includes the planned remediation in this change.

### Frontend/UI

- `frontend/src/components/layout/Navbar.tsx`
  - Has external “Docs” link → remove Docs item (no Help/docs surface until stable).
- `frontend/src/components/dialogs/global/DisclaimerDialog.tsx`
  - Links to external docs → remove external URL and keep guidance inline.
- `frontend/src/components/dialogs/global/ReleaseNotesDialog.tsx`
  - Loads external release notes URL → remove dialog and clear the config flag without loading anything.

### E2E

- `e2e/external-links.spec.ts`
  - Assumes external Docs URL exists → update test to assert “Docs is absent” and “Support opens upstream GitHub issues”.

### Backend/templates/generation

- `crates/server/src/routes/task_attempts/pr.rs`
  - Embeds external note URL → replace with a non-link note or remove the note entirely.
- `crates/server/src/bin/generate_types.rs` (via generated `shared/types.ts`)
  - Generated comment includes external note URL → update generator so generated types do not embed external URLs.

### MCP metadata

- `crates/executors/default_mcp.json`
- `crates/executors-core/default_mcp.json`
  - `meta.*.url` points to external hosted docs → replace with upstream GitHub link or remove.

### Git defaults

- `crates/repos/src/git/mod.rs`
- `crates/repos/tests/git_workflow.rs`
  - Uses an upstream-domain noreply email → replace with a local/no-external-domain default (and update tests).

### Unsupported remote-service tooling

- `crates/review/**`
  - Hard-coded remote API and external terms URL → remove `crates/review` from the workspace and delete the crate directory.

## Dead internal doc references (to fix)

- `README.md` references `docs/operations.md` (missing) → remove or replace with existing docs entry points.
- `docs/fake-agent.md` references `docs/operations.md` (missing) → remove the reference.

## Legacy/compat inventory

### UI/UX compatibility (remove in this change)

- `frontend/src/hooks/utils/useLayoutMode.ts`
  - Legacy URL redirect for bookmarked links → remove and upgrade callers to canonical routes.

### Data-level compatibility (inventory only; do not remove in this change)

- Workspace layout migration:
  - `crates/repos/src/workspace_manager.rs` includes a “legacy worktree layout” migration path.
- Legacy log persistence fallback:
  - `crates/execution/src/container/mod.rs` supports `legacy_jsonl` and related cleanup/fallback behavior.

### Follow-up deletion plan (data-level legacy)

We will only remove data-level compatibility in a dedicated follow-up change after we can demonstrate it is safe:

- Add minimal usage evidence:
  - log/metric counters for how often legacy paths trigger (workspace migration invoked; legacy JSONL fallback selected).
- Define deprecation window:
  - keep compatibility for N releases or N weeks after instrumentation lands (choose during follow-up proposal).
- Removal criteria:
  - legacy path usage stays at ~0 for the full window
  - migration test coverage exists for the “new layout only” expectation
  - documentation clearly states the minimum supported data layout/log mode

