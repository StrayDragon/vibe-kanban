# Change: Stability hardening and ops checklist

## Why
Broadcast log streams can lag and currently fail without resynchronizing, which risks missing log entries and interrupted persistence. Users also discover missing CLI dependencies late (coding agent or GitHub CLI), and there is no centralized guidance for SQLite lock handling, backups, or routine operational checks.

## What Changes
- Add lagged resync behavior for entry-indexed log streams so live streams recover without closing.
- Introduce CLI dependency preflight checks for the selected coding agent and GitHub CLI (install/auth status).
- Document SQLite lock/backup practices plus daily/weekly/monthly ops checklists.

## Impact
- Affected specs: `execution-logs`, `cli-dependency-preflight` (new).
- Affected code: `crates/utils/src/msg_store.rs`, server routes for preflight, frontend dialogs/hooks for preflight status, new ops documentation.
