## 1. Log entry storage
- [x] 1.1 Add `execution_process_log_entries` table with indexed entries and metadata.
- [x] 1.2 Add DB models and queries for tail paging by index + cursor.
- [x] 1.3 Add entry upsert for replacement updates (e.g., tool approval state).
- [x] 1.4 Dual-write entries alongside existing JSONL logs during migration.

## 2. Backend streaming lifecycle
- [x] 2.1 Add MsgStore helpers for bounded history (bytes + entries) and monotonic index tracking.
- [x] 2.2 Update raw/normalized log WebSocket handlers to emit entry-indexed events and close after Finished.
- [x] 2.3 Ensure MsgStore references are released promptly after completion.

## 3. Log history API
- [x] 3.1 Add v2 HTTP endpoints for raw + normalized history with limit + cursor.
- [x] 3.2 Implement history retrieval for running processes from MsgStore tail.
- [x] 3.3 Implement history retrieval for completed processes from entry store with fallback to JSONL parse/backfill.

## 4. Frontend lazy-load UX
- [x] 4.1 Replace JSON patch stream consumer with entry-indexed v2 stream handler.
- [x] 4.2 Update useConversationHistory to load tail first and page older entries on demand.
- [x] 4.3 Add UI controls (Load earlier history + spinner) and preserve scroll position on prepend.
- [x] 4.4 Bound raw log buffers and show a truncation indicator in ProcessLogsViewer.
- [x] 4.5 Switch frontend paths to v2 endpoints.

## 5. Config and defaults
- [x] 5.1 Add env-based defaults for history byte/entry limits and page size.
- [x] 5.2 Document new environment variables and default behavior.

## 6. Migration / backfill
- [x] 6.1 Implement on-demand backfill from JSONL logs into entry table when history requested.
- [x] 6.2 Add optional background backfill for legacy logs.
- [x] 6.3 Emit console log progress during startup backfill.
- [x] 6.4 Remove legacy WS endpoints after tests pass.

## 7. Tests
- [x] 7.1 Add unit tests for MsgStore tail/index tracking and Finished termination.
- [x] 7.2 Add DB tests for history paging and cursor stability.
- [x] 7.3 Add frontend tests for lazy history loading and truncation indicator (as feasible).
