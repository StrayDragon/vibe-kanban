# Change: Add file search indexing guardrails

## Why
Large repositories can trigger heavy CPU/IO during file index builds and watcher registration. With TTLs set to 0, watchers can also linger indefinitely. We need explicit guardrails and visibility into partial indexing so performance remains predictable for single-user workflows.

## What Changes
- Add a configurable cap for file search indexing to limit total files indexed per repo.
- Track and expose whether a repo index is truncated so clients can communicate partial results.
- Skip watcher registration for repositories that exceed the index cap and log a warning.
- Document the new guardrails and defaults.

## Impact
- Affected specs: file-search-index (new)
- Affected code: crates/services/src/services/file_search_cache.rs, crates/services/src/services/cache_budget.rs, crates/services/src/services/project.rs, frontend search UI/types, shared types generation
