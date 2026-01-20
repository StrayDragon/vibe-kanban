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
- Decision: Provide a single follow_up tool with action = send|queue|cancel.
  - Rationale: minimize tool surface while supporting immediate follow-up, queued follow-up, and cancellation.

## Risks / Trade-offs
- Latest-session resolution could target an unintended session if multiple are active.
  - Mitigation: document behavior and return an explicit error if no session exists.

## Migration Plan
- None (new MCP tools only).

## Open Questions
- Should we require explicit session_id when multiple sessions exist for a workspace?
