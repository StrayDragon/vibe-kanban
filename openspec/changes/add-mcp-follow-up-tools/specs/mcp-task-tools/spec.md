## ADDED Requirements
### Requirement: MCP follow-up tool
The system SHALL expose an MCP tool that manages follow-up actions for a session. The tool SHALL accept either session_id or workspace_id and an action of send, queue, or cancel. The tool SHALL require a prompt for send and queue actions and MAY accept an optional variant.

#### Scenario: Send follow-up by workspace id
- **WHEN** a client calls the tool with workspace_id, action=send, and a prompt
- **THEN** the server resolves the latest session for the workspace and triggers a follow-up execution

#### Scenario: Send follow-up by session id
- **WHEN** a client calls the tool with session_id, action=send, and a prompt
- **THEN** the server triggers a follow-up execution for that session

#### Scenario: Queue follow-up by workspace id
- **WHEN** a client calls the tool with workspace_id, action=queue, and a prompt
- **THEN** the server queues the follow-up message for the latest session and returns queue status

#### Scenario: Queue follow-up by session id
- **WHEN** a client calls the tool with session_id, action=queue, and a prompt
- **THEN** the server queues the follow-up message for that session and returns queue status

#### Scenario: Cancel queued follow-up
- **WHEN** a client calls the tool with action=cancel for a session
- **THEN** the server cancels the queued follow-up message and returns queue status

#### Scenario: Workspace has no sessions
- **WHEN** a client calls the tool with workspace_id and no session exists
- **THEN** the tool returns an explicit error
