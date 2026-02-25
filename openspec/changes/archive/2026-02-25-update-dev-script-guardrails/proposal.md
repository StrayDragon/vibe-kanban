# Change: Update dev-script execution guardrails

## Why
Project-level dev scripts can currently be updated and executed through API paths that eventually run shell commands, creating a high-risk command execution surface.

## What Changes
- Add explicit safety boundaries for project dev-script configuration and execution.
- Replace shell-string execution with structured command invocation where feasible.
- Constrain dev-script execution to workspace-scoped directories and validated commands.
- Add execution audit events for dev-script runs.

## Impact
- Affected specs: `dev-script-guardrails`
- Affected code: `crates/server/src/routes/projects.rs`, `crates/server/src/routes/task_attempts/handlers.rs`, `crates/executors/src/actions/script.rs`, related DTO/config files
- Out of scope: changing global auth defaults.
