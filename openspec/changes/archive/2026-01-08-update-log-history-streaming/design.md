## Context
The current log pipeline keeps a large in-memory history per execution and keeps WebSocket streams open after the Finished signal is emitted. When multiple tasks or projects run in parallel, this causes server memory growth. The frontend loads full history into memory and appends raw logs indefinitely, even though users mainly care about the most recent entries.

## Constraints
- Follow-up coding agent runs depend on `agent_session_id` stored in `coding_agent_turns`, which is extracted from log stream output. Any log storage changes MUST preserve this capture path.
- Execution logs are primarily for UI/history; agent runtimes (e.g., Codex) resume from their own session storage rather than replaying full log history.

## Background Summary (from investigation)
- Server memory growth comes from per-execution MsgStore history (100MB cap each) and history being cloned per stream subscriber.
- Completed-process history currently requires parsing full JSONL logs into memory, which causes large spikes for long runs.
- Frontend loads full history via WS and keeps growing arrays for conversation + raw logs; no tail-first UI today.
- Live streams can remain open after Finished; MsgStore cleanup is attempted but can fail if there are still strong Arcs.
- Tool approvals and MCP tool calls update existing entries (replace), so the new store/stream must support upserts.
- Migration plan chosen: run SQL migrations at startup, then perform a blocking backfill before serving requests; console logs only.
- New v2 endpoints are preferred for clarity, and legacy endpoints will be removed after tests pass.
- Entry indexes are per-channel (raw/normalized), independent and monotonic.

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
- Decision: Indexed persistent log entry store
  - Create a new table that stores log entries with a stable `entry_index` and typed payload (normalized entry or stdout/stderr line).
  - Write entries as they are produced (dual-write with existing JSONL log lines during migration).
- Decision: Tail-first history API
  - Add new v2 HTTP endpoints that return the latest N entries (raw or normalized) plus a cursor for older history.
  - History pages include stable entry indexes and `has_more` metadata.
  - Proposed paths:
    - `GET /api/execution-processes/:id/normalized-logs/v2`
    - `GET /api/execution-processes/:id/raw-logs/v2`
- Decision: Entry-indexed live stream protocol
  - Add new v2 WebSocket streams that emit explicit append/replace/finished events with entry indexes.
  - Remove legacy WS endpoints once tests confirm parity.
  - Proposed paths:
    - `WS /api/execution-processes/:id/normalized-logs/v2/ws`
    - `WS /api/execution-processes/:id/raw-logs/v2/ws`
  - WebSocket streams close after Finished is observed.
- Decision: Enforced memory budgets
  - In-memory log history is bounded by byte and entry limits with eviction of oldest data.
  - Limits are configurable via environment variables with safe defaults.
- Decision: Frontend windowed cache
  - The attempt conversation list keeps a bounded window (default latest 10) and can request older pages.
  - Raw log viewers keep a bounded line buffer and surface a truncation indicator.

## Risks / Trade-offs
- Storing indexed entries increases database write volume and storage usage.
- Paging adds UI complexity and requires careful scroll position management.
- Entry-index allocation must remain monotonic across restarts to avoid collisions.
- Dual-write and backfill add migration complexity and require careful cleanup.

## Migration Plan
- Phase 1: Add log entry tables, write paths, and history APIs (tail-first + cursor).
- Phase 2: Update WebSocket streams to emit entry-indexed events; update frontend consumers.
- Phase 3: Backfill legacy JSONL logs into entry tables on-demand or via background job.
- Phase 4: Run a startup backfill before serving requests, with console progress logs.
- Phase 5: Remove legacy JSON patch streaming and JSONL-only history paths after tests pass.

## Startup Backfill Logging (console only)
- Log start summary once:
  - `log-history backfill starting: processes=NN, total_bytes=NN, mode=startup`
- Log progress every N processes or every M bytes (whichever comes first):
  - `log-history backfill progress: processed=NN, entries=NN, bytes=NN, elapsed_ms=NN`
- Log per-process errors without dumping payloads:
  - `log-history backfill error: execution_id=..., error=...`
- Log completion summary:
  - `log-history backfill complete: processes=NN, entries=NN, bytes=NN, elapsed_ms=NN`

## Suggested Worktree Split (implementation planning)
- Worktree A: DB schema + entry models + backfill job + startup logging.
- Worktree B: v2 history API + v2 WS stream protocol + MsgStore index/bounds.
- Worktree C: frontend v2 consumers (conversation history + raw logs) + pagination UX.
- Worktree D: remove v1 endpoints + cleanup + tests.

## Open Questions
- Final default limits (e.g., latest 10 vs 50) and page size.
- Whether to store raw JSONL logs long-term once entry tables are reliable.
- Whether limits should be moved into user config instead of environment variables.
