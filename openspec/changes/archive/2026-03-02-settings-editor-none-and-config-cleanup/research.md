# Research: PR settings + Remote SSH host

This document summarizes current usage and a recommendation for whether to keep, move, or remove:

- “Pull Requests” settings (auto-generate PR title/body + custom prompt)
- “Remote SSH Host” editor setting (remote URL generation)

## 1) Pull Requests settings (“Configure PR creation behavior”)

### Current behavior

- Global Settings surface:
  - `Config.pr_auto_description_enabled` (default state for auto-generation)
  - `Config.pr_auto_description_prompt` (custom prompt template; falls back to a default prompt)
- Create PR flow:
  - Frontend: `frontend/src/components/dialogs/tasks/CreatePRDialog.tsx`
    - Shows a checkbox for auto-generation; initialized from `config.pr_auto_description_enabled`.
    - When enabled, PR title/body inputs are disabled and the backend follow-up uses the prompt template.
  - Backend: `crates/server/src/routes/task_attempts/pr.rs`
    - `POST /api/task-attempts/{id}/pr` accepts `auto_generate_description: bool`.
    - When enabled, starts a coding-agent follow-up that runs `gh pr edit` using the prompt template.

### Users / value

- Helps keep PR title/body high-quality with low effort.
- Especially useful when users frequently create PRs from attempts and want consistent formatting.

### Maintenance cost / complexity

- Adds a Settings card and two config fields.
- Requires GitHub CLI (`gh`) and a working coding-agent executor; error handling already exists.

### Recommendation

**Keep**, but consider **moving** the Settings entry out of General Settings:

- Proposed location: `Settings → Integrations → GitHub` (or an “Advanced” section).
- Rationale: it’s GitHub/PR-specific and depends on `gh` + agent tooling; it can distract users who never use PR features.

**Do not remove** without replacing:

- Removing would eliminate custom prompt support and the global default toggle.
- If removal is desired later, replace with a per-PR dialog-only configuration (and drop global config fields).

## 2) Editor: “Remote SSH Host (Optional)”

### Current behavior

- Global Settings surface:
  - `EditorConfig.remote_ssh_host` / `remote_ssh_user`
  - Only shown for editor types that support remote URLs (VS Code, Cursor, Windsurf).
- Backend URL generation:
  - `crates/services/src/services/config/editor/mod.rs` constructs a `vscode://...` / `cursor://...` / `windsurf://...` remote URL when the host is set.
  - Used by:
    - `POST /api/task-attempts/{id}/open-editor`
    - `POST /api/projects/{id}/open-editor`

### Users / value

- Valuable for users running Vibe Kanban on a remote machine (or in a remote container) while wanting “Open in Editor” to open a local IDE connected via SSH.

### Maintenance cost / complexity

- Two optional settings + some backend URL formatting logic.
- Confusion risk is moderate, but the UI is already conditional (only appears for relevant editor types).

### Recommendation

**Keep**, but consider **moving** under an “Advanced” subsection within the Editor card:

- Rationale: most local users do not need it, but remote setups benefit significantly.
- If we remove it, “Open in Editor” for remote deployments regresses to either (a) spawning a local command that won’t work, or (b) requiring users to use a custom command or manual navigation.

## If removal is desired (future change)

- PR settings removal:
  - Remove `pr_auto_description_*` fields from config + Settings UI.
  - Keep the Create PR dialog checkbox, default it to `false` (or infer from last-used state).
  - Provide migration note: custom prompts will be lost.
- Remote SSH removal:
  - Remove `remote_ssh_*` fields from config + Settings UI.
  - Remove URL-generation path; “Open in Editor” becomes local-only spawn.
  - Provide migration note: remote deployments lose the click-to-open convenience.

