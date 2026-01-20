# Change: Redesign database schema + SeaORM data layer

## Why
The current SQLx implementation relies on large amounts of handwritten SQL and SQLite-specific behavior, which makes PostgreSQL compatibility brittle. We want a clean, cross-database schema with a consistent identifier strategy and an async ORM to reduce manual SQL.

## What Changes
- Adopt SeaORM as the primary data access layer for core records (projects, tasks, workspaces, sessions, execution processes, logs, tags, images).
- Redesign the schema around `id` (auto-increment primary key) + `uuid` (unique external identifier).
- Move all FK relationships to `id` and keep public/API lookups on `uuid`.
- Replace SQLx query macros in model modules with SeaORM entities and query DSL.
- Introduce SeaORM migrations as the canonical schema source with a new baseline (data loss acceptable).
- Keep SQLite and PostgreSQL compatibility via ORM-managed schema and query patterns.
- Replace SQLite update hooks with a service-layer event dispatcher.
- Update Cargo dependency setup and build configuration to avoid cross-worktree target-dir conflicts when adding new crates.

## Impact
- Affected specs: persist-metadata (new)
- Affected code: crates/db, crates/services, crates/server, crates/local-deployment, crates/deployment, .cargo/config.toml, crates/db/migrations
