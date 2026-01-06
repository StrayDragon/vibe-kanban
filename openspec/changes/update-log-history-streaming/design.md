## Context
The current log pipeline keeps a large in-memory history per execution and keeps WebSocket streams open after the Finished signal is emitted. When multiple tasks or projects run in parallel, this causes server memory growth. The frontend loads full history into memory and appends raw logs indefinitely, even though users mainly care about the most recent entries.

## Goals / Non-Goals
- Goals:
  - Bound server memory used by log history per execution and per connection.
  - Default the UI to a "latest entries" view with on-demand loading of older history.
  - Close log streams promptly after execution finishes to release server resources.
  - Preserve access to older history without requiring full in-memory retention.
- Non-Goals:
  - Full redesign of the logging storage model or migrations to a new database schema.
  - Changing executor behavior or the raw log format.

## Decisions
- Decision: Tail-first history API
  - Add an HTTP endpoint that returns the latest N normalized entries with a cursor for older history.
  - The API is best-effort for older history; it can fall back to rebuilding from stored raw logs when needed.
- Decision: Live updates remain WebSocket-based
  - The WebSocket stream sends live patches only and closes after Finished is observed.
  - The client reconnects only when needed and does not keep historical streams open.
- Decision: Enforced memory budgets
  - In-memory log history is bounded by byte and entry limits with eviction of oldest data.
  - Limits are configurable via environment variables with safe defaults.
- Decision: Frontend windowed cache
  - The attempt conversation list keeps a bounded window (default latest 10) and can request older pages.
  - Raw log viewers keep a bounded line buffer and surface a truncation indicator.

## Risks / Trade-offs
- Rebuilding normalized history from raw logs can be CPU-heavy for very large outputs.
- Bounded history may hide context unless the user explicitly loads older pages.
- Paging adds UI complexity and requires careful scroll position management.

## Migration Plan
- Add new endpoints and keep existing WebSocket endpoints operational.
- Ship UI changes behind the new API while preserving graceful fallback for older servers.
- Use defaults that keep current behavior for live logs while reducing memory retention.

## Open Questions
- Final default limits (e.g., latest 10 vs 50) and page size.
- Whether to store normalized patches in the DB for faster paging in future iterations.
- Whether limits should be moved into user config instead of environment variables.
