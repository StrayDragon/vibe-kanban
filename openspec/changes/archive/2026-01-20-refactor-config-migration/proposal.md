# Change: Simplify configuration loading to a single schema

## Why
The current config system maintains a full struct per version (v1-v9) with chained migrations. This is heavy for personal users, increases maintenance cost, and makes small schema changes expensive. We want a single latest schema with robust defaults and minimal breakage.

## What Changes
- Collapse config loading to a single latest schema and remove per-version structs/migrations.
- Ensure all config fields have safe defaults or are optional so fresh installs are usable out of the box.
- Add best-effort tolerance for renamed fields and enum variants using serde aliases where cheap.
- Normalize values after load to keep invariants (for example, ensure defaults are applied consistently).
- Persist only the latest schema when saving config.

## Impact
- Affected specs: config-management (new)
- Affected code: crates/services/src/services/config/mod.rs, crates/services/src/services/config/versions/, crates/services/src/services/config/editor.rs, crates/server/src/bin/generate_types.rs
