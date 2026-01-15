## Context
The database layer currently assumes a local SQLite file and contains logic that deletes the file when migration metadata is missing. Production needs PostgreSQL, and local development should remain lightweight.

## Goals / Non-Goals
- Goals:
  - Prefer `DATABASE_URL` when set, otherwise fall back to the project default SQLite path.
  - Fail fast on invalid configuration.
  - Enforce SQLite foreign keys per connection.
  - Preserve existing SQLite pragmas for performance.
  - Remove destructive startup behavior.
- Non-Goals:
  - Schema changes or timestamp type migrations.
  - Query-level refactors (joins, batching).
  - Runtime config reloading or hot switching.

## Decisions
- Use `DATABASE_URL` when set; if absent, connect to the local SQLite file under the asset directory.
- Reject non-SQLite backends until additional database support is implemented.
- Use `ConnectOptions::after_connect` to execute `PRAGMA foreign_keys = ON` for SQLite.
- Keep `map_sqlx_sqlite_opts` for WAL, synchronous NORMAL, and busy timeout settings.
- Always run `Migrator::up` and never delete the database file based on migration metadata.

## Risks / Trade-offs
- SQLite foreign key enforcement must be set per connection; missing hooks would leave constraints disabled.
- Runtime switching relies on a correctly formatted `DATABASE_URL` in production.

## Migration Plan
- Deploy with `DATABASE_URL` configured for PostgreSQL in production.
- Existing SQLite databases remain intact; migrations run on startup.

## Open Questions
- Do we want to support an explicit env var for the SQLite file path beyond `DATABASE_URL`?
