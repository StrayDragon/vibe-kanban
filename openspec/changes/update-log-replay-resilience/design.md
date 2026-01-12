## Context
- MsgStore enforces `VK_LOG_HISTORY_MAX_BYTES`/`VK_LOG_HISTORY_MAX_ENTRIES` and evicts the oldest entries when limits are exceeded.
- For running processes, `/raw-logs/v2` and `/normalized-logs/v2` history pages are served from MsgStore only, even if older entries have been persisted to `execution_process_log_entries`.
- The UI currently treats `has_more` as "history truncated," which is ambiguous when older entries are missing vs simply not loaded yet.

## Goals / Non-Goals
- Goals: restore full replay by falling back to persistent log entries during eviction, compute accurate `has_more`, and surface a clear partial-history hint.
- Non-Goals: change log streaming protocols or rewrite persistence/storage pipelines.

## Decisions
- Add a `history_truncated` boolean to `LogHistoryPage` to signal incomplete history.
- Expose MsgStore metadata (min index + eviction flag) for raw and normalized history.
- Running-process history logic:
  - If no eviction, serve pages from MsgStore as today.
  - If eviction detected, serve tail pages from MsgStore but compute `has_more` using DB (`has_older` before the earliest returned index).
  - If a client requests a cursor older than the earliest in-memory index, fetch the page from DB.
- Set `history_truncated` when eviction occurred and DB has no older entries (or the log entry table is unavailable).

## Risks / Trade-offs
- Additional DB queries when eviction is detected (bounded to `has_older`/paged fetch).
- `history_truncated` may be conservative if DB writes lag; mitigate by using the earliest available index for checks.

## Migration Plan
- Add the new field in the API response and regenerate shared types.
- Update UI consumers to display a partial-history hint when `history_truncated` is true.

## Open Questions
- Should partial-history hints appear in both raw log views and normalized conversation history, or only in raw logs?
