# TODO

## Security
- Consider reducing the exposure of absolute local paths returned by `GET /api/config/status` when access control is disabled (especially `secret_env_path`). Options: omit sensitive paths, return only relative paths, or require a token for this endpoint.
- Review other endpoints/tools that may trigger local side effects or process execution (preflight checks, setup helpers, etc.) and ensure they are either token-gated or constrained via allowlists.

## Projects Source of Truth
- Decide the long-term semantics of `sync_config_projects_to_db()`:
  - Full reconcile (update + delete) with explicit rules, or
  - File-only projects (stop syncing to DB entirely), keeping DB as runtime/task state only.

## Maintainability
- Split `crates/server/src/legacy_migrations.rs` into smaller modules and define a removal plan for deprecated migration commands.
