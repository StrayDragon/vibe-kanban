# TODO

## Security
- Consider reducing the exposure of absolute local paths returned by `GET /api/config/status` when access control is disabled (especially `secret_env_path`). Options: omit sensitive paths, return only relative paths, or require a token for this endpoint.
- Review other endpoints/tools that may trigger local side effects or process execution (preflight checks, setup helpers, etc.) and ensure they are either token-gated or constrained via allowlists.

## Config Reload Consistency
- Make config reload fully atomic across: runtime `Config`, `public_config`, `ExecutorConfigs` cache, and `config_status` (single snapshot swap or versioned state).
- Reduce TOCTOU risk when loading multiple files (`config.yaml`, `projects.yaml`, `projects.d/*`, `secret.env`). Consider a load "generation" check and/or reloading until a stable read is observed.

## Projects Source of Truth
- Decide the long-term semantics of `sync_config_projects_to_db()`:
  - Full reconcile (update + delete) with explicit rules, or
  - File-only projects (stop syncing to DB entirely), keeping DB as runtime/task state only.

## Maintainability
- Split `crates/server/src/legacy_migrations.rs` into smaller modules and define a removal plan for deprecated migration commands.
- Audit remaining user-facing guidance strings/docs to ensure they consistently reference `projects.yaml` / `projects.d/*.yaml` and the new config layout (no stale `config.yaml`-only instructions).
