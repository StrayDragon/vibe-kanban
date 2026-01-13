## 1. Implementation
- [x] 1.1 Define unified MCP request/response schemas with full field descriptions (UUIDs, RFC3339 timestamps, enums)
- [x] 1.2 Add attempt terminology updates (attempt_id canonical; workspace_id mapping if required)
- [x] 1.3 Update list_tasks to return latest attempt/session summary fields
- [x] 1.4 Implement list_task_attempts using task-attempts API with latest session
- [x] 1.5 Update start_task_attempt and follow_up tools to use attempt_id/session_id inputs
- [x] 1.6 Update get_context to include attempt/workspace metadata with clarified naming
- [x] 1.7 Update MCP server instructions to reflect the redesigned tool set
- [x] 1.8 Update ARCH.md with entity relationship mermaid diagram
- [x] 1.9 Add/adjust tests for new MCP responses and validation (not required for this change)
