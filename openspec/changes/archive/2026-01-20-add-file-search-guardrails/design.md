## Context
File search builds a full-repo FST index and registers filesystem watchers. On very large repos this can spike CPU/IO and keep watchers alive indefinitely when TTLs are disabled.

## Goals / Non-Goals
- Goals:
  - Cap file search indexing with a configurable limit.
  - Surface when indexes are partial so users understand incomplete results.
  - Avoid watcher registration for repos that exceed the index cap.
- Non-Goals:
  - Replace the existing search algorithm or caching strategy.
  - Introduce new external services.

## Decisions
- Add a `VK_FILE_SEARCH_MAX_FILES` budget with a sane default (tunable via env).
- Track `index_truncated` on cached repo entries and propagate to search responses.
- Skip watcher registration when `index_truncated` is true, relying on TTL-based cache invalidation.
- Emit a warning when truncation occurs so operators can tune the limit.

## Risks / Trade-offs
- Search results for large repos may be incomplete; mitigate by surfacing a clear partial-index hint.
- Skipping watchers means changes appear only after cache refresh; acceptable for large repos.

## Migration Plan
- Add the new config and metadata fields.
- Regenerate shared types and update UI to show partial results messaging.

## Open Questions
- What default limit should we use for `VK_FILE_SEARCH_MAX_FILES` (e.g., 200k)?
