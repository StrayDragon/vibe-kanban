# Change: Redesign MCP task tools and schemas

## Why
MCP clients cannot reliably obtain attempt/workspace and session identifiers from task listings, and current tool schemas are inconsistent and under-documented. This blocks follow-up automation and causes confusion around attempt vs workspace concepts.

## What Changes
- Redesign the MCP tool set with consistent naming, minimal list operations, and unified attempt terminology.
- Add richer, consistently documented response fields (IDs, timestamps, and attempt/session summaries).
- Expose `list_task_attempts` and include latest attempt/session summaries in `list_tasks`.
- Update MCP tool schema descriptions to document every field.
- Add an entity-relationship mermaid diagram to ARCH.md.

## Impact
- Affected specs: mcp-task-tools (new consolidated spec)
- Affected code: crates/server/src/mcp/task_server.rs, crates/db/src/models/task.rs (if new summary fields), crates/server/src/routes/task_attempts.rs (if new API shape), ARCH.md
- Potentially supersedes: openspec/changes/add-mcp-follow-up-tools, openspec/changes/add-mcp-task-attempts-tool
