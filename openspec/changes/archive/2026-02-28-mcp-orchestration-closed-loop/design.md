## Context

Vibe Kanban is operated by both humans (via the UI) and non-human clients (LLM agents and external orchestrators such as Zirvox). The current MCP server (`mcp_task_server`) exposes a compact “control plane” tool set (create/update/start/follow-up), but it does not expose the minimum “observability loop” required for reliable automation:

- Attempt/workspace status (latest session, latest process, coarse state, last activity)
- Incremental log tailing with cursor-based paging (prefer normalized logs)
- A lightweight “what changed” view (diff summary + changed file list), without requiring WebSocket streaming

We want this loop to be available through MCP with **very few tools** (LLM selection quality drops as tool count grows), while still allowing direct `/api` usage by deterministic clients when needed.

Constraints and assumptions:
- Same-host deployments for now; authentication/access control is out-of-scope.
- Tool naming stays `snake_case` and additive (no breaking renames).
- Prefer explicit IDs (`attempt_id`) over implicit cwd context.

## Goals / Non-Goals

**Goals:**
- Add a minimal “attempt closed loop” to MCP using exactly three tools:
  - `get_attempt_status`
  - `tail_attempt_logs`
  - `get_attempt_changes`
- Reuse existing backend primitives where possible:
  - Execution process log history v2 cursor semantics
  - Diff preview guardrails (blocked + reason)
- Keep response shapes predictable and documented (UUIDs, RFC3339 timestamps, stable enums).

**Non-Goals:**
- No auth boundary work and no token passthrough plumbing in MCP for this change.
- No event subscription tool and no long-lived streaming protocol via MCP (SSE/WS remains available via `/api` for advanced clients).
- No `task.search` and no new “task label/tag” data model.
- No protocol-level idempotency/request correlation beyond existing IDs.

## Decisions

### 1) Add 3 MCP tools (minimal surface)

We add three tools instead of many smaller ones to keep the MCP surface LLM-friendly:

- `get_attempt_status(attempt_id)`:
  - returns: attempt/workspace info, latest session info, latest “relevant” execution process info, coarse `state`, and `last_activity_at`.
- `tail_attempt_logs(attempt_id, channel, cursor, limit)`:
  - returns: tail-first history page for the latest relevant execution process.
  - defaults: `channel=normalized` with a modest `limit`.
- `get_attempt_changes(attempt_id, force)`:
  - returns: diff summary + changed files (no contents).
  - respects diff preview guardrails unless `force=true`.

Alternatives considered:
- **Expose all `/api` endpoints to agents**: too much surface; LLMs make poor choices and require more prompt guidance.
- **Single “attempt.inspect” mega-tool**: reduces tool count further but creates very large, mixed payloads (status + logs + diffs) which is harder to paginate and harder for LLMs to handle reliably.

### 2) Resolve “latest process” consistently (attempt → session → execution process)

Logs and status need a stable definition of “latest relevant execution process”.

Decision:
- “Relevant” run reasons for attempt automation are `codingagent`, `setupscript`, and `cleanupscript`.
- `devserver` processes are excluded from the attempt closed-loop status/log tail (they are long-running and better handled by dedicated UI flows).

Implementation sketch:
- Load workspace by `attempt_id` (workspace UUID).
- Resolve latest session by workspace (`Session::find_latest_by_workspace_id`).
- Resolve latest relevant execution process across sessions for that workspace:
  - Prefer `ExecutionProcess::find_latest_by_workspace_and_run_reason(..., CodingAgent)`
  - Fallback to latest across `{SetupScript, CleanupScript}` if no coding-agent process exists (optional; spec decides exact behavior).

This provides a single `execution_process_id` that downstream tools can reference.

### 3) Prefer small backend aggregation helpers (optional endpoints)

To avoid duplicating lookup logic in multiple MCP tools (and to enable Zirvox to call `/api` directly when it wants deterministic behavior), we prefer implementing small backend aggregation helpers/endpoints for:

- Attempt status resolution (workspace + latest session + latest relevant execution process)
- Attempt changes snapshot (diff summary + changed files with guardrails)

MCP tools can then call these endpoints and return their payloads with MCP-friendly schemas.

Alternative:
- Implement everything by chaining existing endpoints from MCP only (more round-trips, more MCP-only logic, harder to reuse outside MCP).

### 4) Diff changes snapshot omits contents and follows guardrails

`get_attempt_changes` returns summary + file list (paths prefixed with repo name for multi-repo worktrees). It does not return full hunks/patch text by default.

Guard behavior:
- If summary computation fails or exceeds configured thresholds and `force=false`, return:
  - `blocked=true`
  - `blocked_reason` (`summary_failed` | `threshold_exceeded`)
  - summary only; empty file list

## Risks / Trade-offs

- **[Polling load]** `tail_attempt_logs` can be called frequently → mitigate with defaults (normalized + modest limit), cursor paging, and clear usage docs.
- **[Ambiguous “failure reason”]** Process status alone may not explain failure → keep failure summary coarse (status + exit_code + optional short hint) and direct clients to logs.
- **[Diff cost]** Even file-list generation can be expensive → enforce guardrails and omit contents; allow `force=true` only when explicitly requested.
- **[Tool creep]** Pressure to add more tools (events, search, patches) → keep this change strictly scoped to the closed loop; evaluate follow-on changes separately.

## Migration Plan

- Add new backend helpers/endpoints (if used) in an additive way; do not modify existing routes’ shapes.
- Extend MCP server with the three new tools and full field descriptions.
- Regenerate TypeScript types only if new `/api` DTOs are intended for frontend reuse.
- Rollback is a simple revert: changes are additive and do not require data migrations.

## Open Questions

- Should `get_attempt_status`’s `state` include `idle` explicitly when no relevant process exists?
- Should `tail_attempt_logs` support `channel=raw|normalized|both`, or default to `normalized` only to reduce payload size?
