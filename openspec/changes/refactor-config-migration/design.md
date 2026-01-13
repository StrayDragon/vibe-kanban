## Context
The config system currently keeps one struct per version and migrates forward on load. For personal users, this adds heavy maintenance and a lot of code churn for minor schema changes.

## Goals / Non-Goals
Goals:
- Provide a single latest schema for config load/save.
- Keep config files usable out of the box with safe defaults.
- Minimize breakage by tolerating simple field/enum renames via serde aliases.
- Reduce code size and migration complexity.
Non-Goals:
- Full compatibility with all historical versions.
- Maintaining per-version struct definitions.

## Decisions
- Decision: Remove the versioned config module chain and keep only the latest Config schema.
- Decision: Use serde defaults/Option fields for missing data and alias for cheap renames.
- Decision: Add a small normalization step after deserialization to enforce invariants.
- Decision: Keep config_version as metadata but do not gate loading on it; saving always writes the latest value.

## Risks / Trade-offs
- Older configs with structural differences may lose data; mitigate by adding aliases only for the most recent renames.
- Unknown fields will be ignored on load and dropped on save.

## Migration Plan
- Delete versioned config modules and wire the config loader directly to the latest schema.
- Add defaults and aliases, and introduce normalize() for invariants.
- Update tests to verify defaults, alias handling, and fallback to defaults.
- Regenerate shared types if the schema moved or changed.

## Open Questions
- Which historical field renames should be preserved as aliases (if any)?
