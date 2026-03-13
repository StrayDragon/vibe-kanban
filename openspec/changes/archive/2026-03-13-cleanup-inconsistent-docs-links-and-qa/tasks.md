## 1. Inventory and policy baseline

- [x] 1.1 Produce a cleanup inventory report (external links, dead doc refs, legacy/compat surfaces) and commit it under `docs/` or `openspec/reports/` (verify: report includes file paths + brief remediation notes).
- [x] 1.2 Define the external link allowlist policy (only `https://github.com/BloopAI/vibe-kanban...`) and the disallowed-domain list for product surfaces (verify: policy is written down in the inventory report and referenced by the guardrail script).

## 2. Repo QA guardrails (no drift)

- [x] 2.1 Add a deterministic external-link guardrail script that scans product surfaces and prints file:line matches (verify: introducing a known-bad domain causes non-zero exit and prints matches).
- [x] 2.2 Add a docs-link guardrail that catches dead internal file references like missing `docs/*.md` (verify: temporarily referencing a missing file makes the check fail with a clear message).
- [x] 2.3 Add `pnpm run qa` to run the CI-equivalent checks (and include the new guardrails) (verify: `pnpm run qa` matches `.github/workflows/test.yml` coverage and passes on main).
- [x] 2.4 Wire the new guardrails into CI (verify: GitHub Actions runs them and the job fails on violations).

## 3. In-app Help and navigation (Docs/Support)

- [x] 3.1 Remove the navbar “Docs” external link (defer Help/docs until the product stabilizes); keep “Support” pointing to `https://github.com/BloopAI/vibe-kanban/issues` and opening in a new tab (verify: manual click + `e2e/external-links.spec.ts`).
- [x] 3.2 Update `e2e/external-links.spec.ts` to assert the new behavior (no Docs item; Support opens upstream GitHub) (verify: `pnpm run e2e:test` passes).

## 4. Onboarding dialogs (no external hosted docs/release notes)

- [x] 4.1 Remove external docs URLs from the onboarding safety notice dialog and keep guidance inline (verify: dialog renders no external docs link).
- [x] 4.2 Remove the Release Notes dialog implementation that loads external hosted content; ensure startup does not attempt external navigation/iframe (verify: when `config.show_release_notes=true`, the app clears it and continues without external fetch).
- [x] 4.3 Update any Rust-side config defaults/tests that assumed release notes behavior (verify: `cargo test --workspace` passes).

## 5. Docs and README cleanup (remove upstream messaging from README)

- [x] 5.1 Remove dead internal references to missing docs files (for example README and `docs/fake-agent.md` referencing `docs/operations.md`) (verify: docs-link guardrail passes).
- [x] 5.2 Remove upstream/fork messaging from `README.md` (NOTICE remains for attribution) and keep README focused on this fork’s supported workflow (verify: README contains no “Upstream project/fork network” section; still contains License pointer).
- [x] 5.3 Sweep repo docs for misleading/outdated text that assumes upstream-hosted docs/services and replace with local-first wording (verify: external-link guardrail passes).

## 6. Remove legacy/compat surfaces (upgrade to current behavior)

- [x] 6.1 Remove the legacy URL redirect compatibility in `frontend/src/hooks/utils/useLayoutMode.ts` and update any callers to canonical routes (verify: `pnpm run check` + basic manual navigation of tasks routes).
- [x] 6.2 Inventory data-level legacy compatibility (workspace layout migrations, legacy log persistence) and record a follow-up deletion plan with safety criteria (verify: inventory report explicitly separates UI compat vs data compat and states “no deletion in this change”).

## 7. Remove upstream-hosted defaults from tooling/workspace

- [x] 7.1 Remove `crates/review` from the Rust workspace (and delete the crate directory if it is unused) (verify: `cargo test --workspace` passes and no crate depends on it).
- [x] 7.2 Remove/replace upstream-hosted URLs embedded in templates/generators (for example PR auto-description notes) and regenerate shared types via `pnpm run generate-types` (verify: `rg vibekanban\\.com` is clean for product surfaces; `pnpm run generate-types:check` passes).
- [x] 7.3 Remove/replace upstream-hosted MCP metadata URLs in `crates/*/default_mcp.json` (verify: external-link guardrail passes and MCP settings UI still renders).

## 8. Final verification

- [x] 8.1 Run the full local QA suite: `pnpm run qa` + `pnpm run e2e` (verify: all checks pass).
- [x] 8.2 Manual smoke: onboarding flow (disclaimer, no release notes), navbar docs/help, and support link behavior (verify: no external navigation except upstream GitHub issues).
