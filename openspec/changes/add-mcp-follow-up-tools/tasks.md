## 1. Implementation
- [x] 1.1 Add MCP request/response types for unified follow_up tool (session_id/workspace_id, action, prompt, optional variant).
- [x] 1.2 Add a helper to resolve workspace_id to the latest session_id and return a clear error when none exists.
- [x] 1.3 Implement MCP tool: follow_up with actions send, queue, and cancel.
- [x] 1.4 Update MCP server info/instructions to mention the new tool.
- [x] 1.5 Add tests or lightweight QA notes covering follow-up send/queue/cancel flows.

## 2. QA Notes
- [ ] 2.1 Use MCP follow_up action=send with session_id and verify a new execution process starts.
- [ ] 2.2 Use MCP follow_up action=queue with workspace_id and verify queue status is returned.
- [ ] 2.3 Use MCP follow_up action=cancel and verify queue status clears.
