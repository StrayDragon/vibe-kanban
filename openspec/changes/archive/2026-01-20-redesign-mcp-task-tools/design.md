## Context
MCP task tools are currently inconsistent in naming and shape, and do not return attempt/workspace/session identifiers from task listings. Agents cannot schedule follow-ups without additional out-of-band context. There is also confusion about the relationship between "attempt" and "workspace" across UI and MCP.

## Goals / Non-Goals
- Goals:
  - Provide a minimal, coherent MCP tool set with consistent naming and field schemas.
  - Make follow-up scheduling possible using only MCP responses.
  - Clarify attempt vs workspace terminology for agents.
  - Ensure every schema field has a description to guide agents.
- Non-Goals:
  - Change underlying DB schema or HTTP API semantics beyond what MCP needs.
  - Remove existing HTTP endpoints unrelated to MCP.

## Decisions
- **Terminology**: MCP will use the term "attempt" for task execution workspaces. The MCP schema will expose `attempt_id` as the canonical identifier (backed by workspace_id internally).
- **Minimal list surface**: Only `list_projects`, `list_repos`, `list_tasks`, and `list_task_attempts` are list tools. Other tools are get/create/update/delete/start/follow_up.
- **Latest summaries**: `list_tasks` returns latest attempt/session summary fields to enable direct follow-up routing without extra calls.
- **Latest attempt selection**: Latest attempt is the most recently created workspace (ORDER BY workspace.created_at DESC, attempt_id ASC).
- **Data source**: `list_task_attempts` is backed by `/api/task-attempts/with-latest-session` to return workspace + latest session in one call.
- **Schema documentation**: Every request and response field must include a `schemars` description with format hints (UUID, RFC3339, enum values).

## Risks / Trade-offs
- Breaking MCP clients if tool names or fields change; may require temporary aliases or migration guidance.
- Additional queries to compute latest attempt summaries for tasks; may need caching if performance regresses.

## Migration Plan
- Implement MCP tools with new schemas.
- Do not keep legacy tool names or field aliases; MCP is authoritative.
- Update ARCH.md and MCP tool documentation.

## Open Questions
- None.
