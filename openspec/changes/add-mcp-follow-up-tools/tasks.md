## 1. Implementation
- [ ] 1.1 Add MCP request/response types for follow-up and queue tools (session_id/workspace_id, prompt/message, optional variant).
- [ ] 1.2 Add a helper to resolve workspace_id to the latest session_id and return a clear error when none exists.
- [ ] 1.3 Implement MCP tool: send_follow_up (calls /api/sessions/{session_id}/follow-up).
- [ ] 1.4 Implement MCP tool: queue_follow_up (calls /api/sessions/{session_id}/queue).
- [ ] 1.5 Update MCP server info/instructions to mention the new tools.
- [ ] 1.6 Add tests or lightweight QA notes covering follow-up and queue flows.
