# Change: Add MCP follow-up and queue tools

## Why
MCP users can start tasks but cannot send follow-up prompts or queue messages for active sessions, which blocks multi-agent scheduling and self-dispatch workflows.

## What Changes
- Add an MCP tool to send an immediate follow-up prompt for a session/workspace.
- Add an MCP tool to queue a follow-up prompt to run when the current execution finishes.
- Resolve workspace_id to the latest session_id when session_id is not supplied.
- Return explicit errors when no session exists or when the prompt/message is empty.

## Impact
- Affected specs: mcp-task-tools (new)
- Affected code: crates/server/src/mcp/task_server.rs, crates/server/src/routes/sessions/mod.rs (reuse), crates/server/src/routes/sessions/queue.rs (reuse)
