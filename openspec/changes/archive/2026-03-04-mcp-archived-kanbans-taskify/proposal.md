## Why

ArchivedKanban operations can be slow (bulk updates, task-group expansion, event enqueueing, and cleanup for destructive deletes). When driven via MCP by external orchestrators/agents, long-running calls are better executed via MCP Tasks (poll/cancel) rather than blocking a single tool call.

Separately, `delete_archived_kanban` is irreversible and high-risk to expose to “other agents” by default. We want to keep the MCP surface safe-by-default while still enabling the common reversible flows (archive + restore).

## What Changes

- Mark archived-kanban bulk tools as MCP task-capable (`execution.taskSupport=optional`):
  - `archive_project_kanban`
  - `restore_archived_kanban`
- **BREAKING**: remove `delete_archived_kanban` from the MCP tool set (HTTP/UI deletion remains available).
- Update tool descriptions and unit tests to reflect the new MCP surface.

## Capabilities

### New Capabilities
- (none)

### Modified Capabilities
- `archived-kanbans`: adjust the MCP tool contract (remove delete tool; require task-capable execution metadata for archive/restore).

## Impact

### Goals
- Allow orchestrators to run archive/restore asynchronously via MCP Tasks (poll + cancel).
- Reduce accidental data loss risk by removing MCP-level access to archived-kanban deletion.
- Keep HTTP APIs and the frontend behavior unchanged.

### Non-goals
- Adding progress reporting for these tasks (beyond MCP task states and the final payload).
- Changing the underlying archive/restore/delete HTTP semantics or data model.

### Risks
- Orchestrators that previously called `delete_archived_kanban` via MCP will break and must switch to HTTP/UI deletion.
- Some MCP clients may ignore `execution.taskSupport`; this change is additive for those clients.

### Verification
- `cargo test -p server`
- Validate via an MCP client that `tools/list`:
  - includes `archive_project_kanban` and `restore_archived_kanban` with `execution.taskSupport=optional`
  - does **not** include `delete_archived_kanban`
