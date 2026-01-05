## Context
Users manage multiple Claude Code accounts in llman using TOML groups. Vibe Kanban needs to surface those groups as selectable Claude Code configurations without manual env duplication.

## Goals / Non-Goals
- Goals:
  - Read llman group envs from a configurable path with a sane default.
  - Expose each group as a Claude Code profile variant users can select.
  - Allow users to import llman groups on demand without automatic syncing.
- Non-Goals:
  - Editing or writing llman config files.
  - Supporting llman groups for non-Claude executors in this change.

## Decisions
- Config key: add `llman_claude_code_path` (string, optional) to app config; when unset, default to `~/.config/llman/claude-code.toml` resolved via `dirs::config_dir()`.
- Parsing: read TOML, extract `[groups.<name>]` tables, and keep only string values. Non-string values are ignored with a warning.
- Mapping: create Claude Code variants named `LLMAN_<GROUP>` where `<GROUP>` is canonicalized to SCREAMING_SNAKE_CASE.
- Import behavior: manual import creates or updates `LLMAN_` variants and persists them to `profiles.json`. No automatic syncing occurs on reload.
- Update rules: during import, `cmd.env` is replaced with the group's env map; other fields are preserved.

## Risks / Trade-offs
- Import overwrites env values for `LLMAN_` variants on demand; users should copy/rename if they want a custom env map.
- Invalid TOML or unexpected types could silently drop groups; mitigate with warnings in logs and clear UI messaging.

## Migration Plan
- Bump config version and backfill `llman_claude_code_path` to `None` (default path resolution) for existing configs.
- Use the saved path for manual imports; no automatic profile changes on startup.

## Open Questions
- None.
