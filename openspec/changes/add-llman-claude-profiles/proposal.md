# Change: Import llman Claude Code env groups as profiles

## Why
Users manage multiple Claude Code accounts via llman groups and need to select them in Vibe Kanban without manually copying environment variables.

## What Changes
- Add a configurable llman config path (default `~/.config/llman/claude-code.toml`).
- Add a manual "Import from llman" action that creates or updates Claude Code profile variants named `LLMAN_<GROUP>`.
- Imported variants are persisted in profiles and only change when the user re-imports.

## Impact
- Affected specs: llman-profile-import
- Affected code: executor profile loading/merging, config versioning, server config update hooks, frontend settings UI, shared types
