# Change: Tighten database startup configuration

## Why
The current DBService assumes a local SQLite file and deletes the database when migration metadata is missing. We need explicit startup configuration, safe fallback to the project default SQLite path when `DATABASE_URL` is missing, fail-fast behavior on misconfiguration, and non-destructive migrations while keeping SQLite compatibility for now.

## What Changes
- Use `DATABASE_URL` when present, otherwise fall back to the project default SQLite path.
- Fail fast on invalid `DATABASE_URL` and reject non-SQLite URLs until additional backends are supported.
- Enable SQLite foreign key enforcement on every connection and keep existing pragmas.
- Remove the sqlite_master migration sentinel deletion logic.
- Continue running migrations on startup without destructive cleanup.

## Impact
- Affected specs: connect-database (new)
- Affected code: crates/db/src/lib.rs (plus tests if added)
