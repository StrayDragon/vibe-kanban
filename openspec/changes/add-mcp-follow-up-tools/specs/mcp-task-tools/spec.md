## ADDED Requirements
### Requirement: MCP follow-up tool
The system SHALL expose an MCP tool that starts a follow-up execution for an existing session using a prompt. The tool SHALL accept either session_id or workspace_id, and MAY accept an optional variant.

#### Scenario: Send follow-up by workspace id
- **WHEN** a client calls the tool with workspace_id and a prompt
- **THEN** the server resolves the latest session for the workspace and triggers a follow-up execution

#### Scenario: Send follow-up by session id
- **WHEN** a client calls the tool with session_id and a prompt
- **THEN** the server triggers a follow-up execution for that session

#### Scenario: Workspace has no sessions
- **WHEN** a client calls the tool with workspace_id and no session exists
- **THEN** the tool returns an explicit error

### Requirement: MCP queue follow-up tool
The system SHALL expose an MCP tool that queues a follow-up message for a session. The tool SHALL accept either session_id or workspace_id, and MAY accept an optional variant.

#### Scenario: Queue follow-up by workspace id
- **WHEN** a client calls the tool with workspace_id and a message
- **THEN** the server queues the follow-up message for the latest session and returns queue status

#### Scenario: Queue follow-up by session id
- **WHEN** a client calls the tool with session_id and a message
- **THEN** the server queues the follow-up message for that session and returns queue status

#### Scenario: Workspace has no sessions
- **WHEN** a client calls the tool with workspace_id and no session exists
- **THEN** the tool returns an explicit error
