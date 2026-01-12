## 1. Implementation
- [x] 1.1 Add MCP request/response types for follow-up and queue tools (session_id/workspace_id, prompt/message, optional variant).
- [x] 1.2 Add a helper to resolve workspace_id to the latest session_id and return a clear error when none exists.
- [x] 1.3 Implement MCP tool: send_follow_up (calls /api/sessions/{session_id}/follow-up).
- [x] 1.4 Implement MCP tool: queue_follow_up (calls /api/sessions/{session_id}/queue).
- [x] 1.5 Update MCP server info/instructions to mention the new tools.
- [x] 1.6 Add tests or lightweight QA notes covering follow-up and queue flows.

## 2. QA Notes
- [ ] 2.1 Use MCP send_follow_up with session_id and verify a new execution process starts.
- [ ] 2.2 Use MCP queue_follow_up with workspace_id and verify queue status is returned.
