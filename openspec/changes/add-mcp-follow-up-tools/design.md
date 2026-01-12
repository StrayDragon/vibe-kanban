## Context
MCP currently exposes task and workspace management tools but cannot send follow-up prompts to an existing session. The existing HTTP API supports follow-up execution and queued follow-up messages via session-scoped endpoints.

## Goals / Non-Goals
- Goals:
  - Allow MCP clients to send immediate follow-up prompts.
  - Allow MCP clients to queue follow-up prompts while an execution is running.
  - Minimize scope by reusing existing session endpoints and error handling.
- Non-Goals:
  - Building new scheduling logic or UI.
  - Changing executor behavior or log formats.

## Decisions
- Decision: MCP tools accept either session_id or workspace_id.
  - Rationale: session_id is required by the API, but workspace_id is more discoverable from MCP context.
- Decision: Resolve workspace_id to the latest session by created_at.
  - Rationale: consistent with current session listing semantics and avoids additional API surface.
- Decision: Keep tools limited to send_follow_up and queue_follow_up.
  - Rationale: match the requested two capabilities; add cancel/status in a follow-up change if needed.

## Risks / Trade-offs
- Latest-session resolution could target an unintended session if multiple are active.
  - Mitigation: document behavior and return an explicit error if no session exists.

## Migration Plan
- None (new MCP tools only).

## Open Questions
- Should we expose optional cancel/status tools for queued messages in a follow-up change?
- Should we require explicit session_id when multiple sessions exist for a workspace?
