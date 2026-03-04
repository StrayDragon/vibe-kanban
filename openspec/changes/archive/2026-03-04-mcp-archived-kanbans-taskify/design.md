## Context

The `archived-kanbans` feature introduced an MCP tool surface to let external orchestrators/agents archive and restore batches of tasks. Some of these operations can be slow (bulk DB updates, task-group expansion checks, and event outbox enqueueing).

The MCP server already supports MCP Tasks (`tasks/*`) and uses `execution.taskSupport=optional` on other heavy tools (e.g. attempt change/patch/file and attempt lifecycle tools) to encourage async execution.

At the same time, `delete_archived_kanban` is irreversible and too risky to expose to “other agents” by default.

## Goals / Non-Goals

**Goals:**
- Make `archive_project_kanban` and `restore_archived_kanban` task-capable via `execution.taskSupport=optional`.
- Remove `delete_archived_kanban` from MCP to reduce accidental data loss risk.
- Keep HTTP APIs and the frontend behavior unchanged.

**Non-Goals:**
- Add progress/percent reporting for these tasks.
- Make mutating MCP tasks resumable across server restarts.
- Add new “delete with confirmation” MCP variants in this change.

## Decisions

1) Declare archive/restore as task-capable via `execution.taskSupport=optional`
- **Why:** aligns with existing MCP conventions in `crates/server/src/mcp/task_server.rs` and enables `tasks/create` + polling/cancel.
- **Alternative:** keep sync-only calls; rejected due to orchestrator timeouts and poor UX for long-running operations.

2) Remove `delete_archived_kanban` from MCP (breaking)
- **Why:** safe-by-default for other agents; deletion remains available via HTTP/UI for explicit human-driven action.
- **Alternatives considered:**
  - Env-gated exposure: still risky when enabled and easy to misconfigure.
  - Add confirm tokens / expected counts: mitigates but does not eliminate the core “irreversible action exposed to agents” risk.

3) Keep current task resume policy (read-only only)
- **Why:** the MCP task runtime currently treats `read_only_hint=true` tools as resumable; archive/restore are mutating so they should not auto-resume after restart.

## Risks / Trade-offs

- **Breaking MCP deletion** → orchestrators that used `delete_archived_kanban` must migrate to HTTP/UI deletion.
- **Clients ignoring taskSupport metadata** → no functional regression; tools still work synchronously.

## Migration Plan

- Update orchestrators:
  - For archive/restore: optionally switch to MCP Tasks mode; sync calls continue to work.
  - For deletion: switch to HTTP `DELETE /api/archived-kanbans/:id` (or UI).
- Rollback: re-add the MCP tool and restore previous tool descriptions/tests.

## Open Questions

- Should we later introduce a “delete archived kanban” MCP tool behind an explicit env flag plus a typed confirmation payload?
- Do we want lightweight phase-based `status_message` updates for long deletes (if MCP deletion is ever reintroduced)?
