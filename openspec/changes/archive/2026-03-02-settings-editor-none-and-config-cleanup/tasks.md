## 1. Backend: Disabled editor type

- [x] 1.1 Add `EditorType::None` to `crates/services/src/services/config/editor/mod.rs` and ensure `EditorConfig` treats it as “disabled” (no command resolution, no remote URL).
- [x] 1.2 Make `POST /api/task-attempts/{id}/open-editor` and `POST /api/projects/{id}/open-editor` reject requests when the effective editor type is `NONE` (verify with a curl request returning 400 and a clear error message).
- [x] 1.3 Update any server-side editor name/icon helpers (if any) to handle the new enum variant and re-run `cargo test --workspace` for compilation coverage.

## 2. Frontend: Hide “Open in …” affordances when disabled

- [x] 2.1 Add a single helper/hook (e.g., `isEditorIntegrationEnabled`) derived from `config.editor.editor_type` and use it as the source of truth.
- [x] 2.2 Gate the navbar IDE icon/button (`frontend/src/components/layout/Navbar.tsx`) behind “editor integration enabled” (verify: selecting “None” removes the icon).
- [x] 2.3 Gate all other “Open in …” entry points (actions dropdown, diff cards, next action cards, preview panel, and any fallback selection dialogs) behind the same helper (verify: ripgrep finds no un-gated “open editor” UI paths).
- [x] 2.4 Update `frontend/src/components/ide/IdeIcon.tsx` / `getIdeName` to handle `EditorType.NONE` and ensure `pnpm run check` passes.
- [x] 2.5 Update the Editor settings dropdown to include a localized “None / Do not use” label and suppress availability status when `NONE` is selected (verify in both `en` and `zh-Hans` locales).

## 3. Backend + DB: Project override for `--no-verify`

- [x] 3.1 Add a SeaORM migration to add a nullable `git_no_verify_override` column on `projects` (verify: `pnpm run prepare-db` succeeds and the column exists).
- [x] 3.2 Update SeaORM entities + `db::models::project::{Project, UpdateProject}` to expose `git_no_verify_override` (verify: Rust compiles and `/api/projects` includes the field).
- [x] 3.3 Update git operations that currently read `Config.git_no_verify` to use an effective value computed from project override precedence (verify: add a unit/integration test covering global=true + project=false → effective=false).

## 4. Frontend: Project override UI + copy updates

- [x] 4.1 Add a project-level control in Settings → Projects for hook skipping with 3 states: inherit / enabled / disabled, and persist it via the existing project update API (verify: reload the page and confirm the state is preserved).
- [x] 4.2 Update the global git hook helper text in General Settings to explain it is the default and can be overridden per project (verify in `en` and `zh-Hans`).

## 5. Research: PR settings + remote SSH host

- [x] 5.1 Audit “Pull Requests” settings usage end-to-end (settings → create PR dialog → backend) and write a short recommendation: keep / move to advanced / remove (include migration plan if removal is recommended).
- [x] 5.2 Audit “Remote SSH Host” usage and write a short recommendation: keep / move to advanced / remove (include who it’s for and what breaks if removed).

## 6. Final verification

- [x] 6.1 Regenerate shared TS types (`pnpm run generate-types`) after Rust changes (verify: no manual edits to `shared/types.ts`).
- [x] 6.2 Run `pnpm run check` and `pnpm run backend:check` and do a quick UI smoke test:
  - Editor = None hides all “Open in …” affordances
  - Project override for `--no-verify` changes behavior as expected
