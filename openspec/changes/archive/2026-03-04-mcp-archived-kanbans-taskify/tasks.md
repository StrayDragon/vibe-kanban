## 1. MCP Tool Surface Changes

- [x] 1.1 Mark `archive_project_kanban` and `restore_archived_kanban` as task-capable by adding `execution(task_support = "optional")` in `crates/server/src/mcp/task_server.rs` (verify: unit test asserts `execution.taskSupport=optional`).
- [x] 1.2 Remove the MCP tool `delete_archived_kanban` from `crates/server/src/mcp/task_server.rs` (verify: `tool_router_does_not_expose_delete_archived_kanban` test).
- [x] 1.3 Update the archived-kanban tool descriptions (`Next:` suggestions) to avoid referencing the removed tool (verify: compile + `cargo test -p server`).

## 2. Tests & Verification

- [x] 2.1 Update MCP tool router tests to reflect the new tool set (verify: `cargo test -p server`).

## 3. Specs & Docs Alignment

- [x] 3.1 Add an incremental spec for the archived-kanban MCP contract at `openspec/changes/mcp-archived-kanbans-taskify/specs/archived-kanbans/spec.md` (verify: `openspec status --change mcp-archived-kanbans-taskify`).
- [x] 3.2 Update the original `archived-kanbans` change docs to remove MCP deletion references (verify: `rg delete_archived_kanban openspec/changes/archived-kanbans` returns no matches).
