# codex-dynamic-tools Specification

## ADDED Requirements

### Requirement: Dynamic Tools registration for Codex threads
The system SHALL be able to register a set of VK-provided Dynamic Tools for Codex threads at thread start.

#### Scenario: Tools are registered for a new thread
- **WHEN** a Codex thread is started with Dynamic Tools enabled
- **THEN** the system registers the configured Dynamic Tool specs on `thread/start`
- **AND** each tool includes a strict JSON input schema

#### Scenario: Tools are not registered when disabled
- **WHEN** a Codex thread is started with Dynamic Tools disabled
- **THEN** the system does not register any VK Dynamic Tool specs

### Requirement: Dynamic Tool call execution
The system SHALL execute Codex app-server Dynamic Tool call requests and return tool outputs.

#### Scenario: Known tool succeeds
- **WHEN** the Codex app-server requests execution of a known VK Dynamic Tool with valid arguments
- **THEN** the system executes the tool
- **AND** returns a `success: true` response containing at least one text content item

#### Scenario: Unknown tool is rejected
- **WHEN** the Codex app-server requests execution of an unknown tool name
- **THEN** the system returns `success: false`
- **AND** returns a text content item that explains the tool is unsupported

#### Scenario: Invalid tool arguments are rejected
- **WHEN** the Codex app-server requests execution of a known tool with arguments that fail the tool’s JSON schema validation
- **THEN** the system returns `success: false`
- **AND** returns a text content item describing the validation error

### Requirement: Minimum supported VK Dynamic Tools (read-only)
When Dynamic Tools are enabled, the system SHALL provide at least the following read-only tools:
- `vk.get_attempt_status`
- `vk.tail_attempt_logs`
- `vk.get_attempt_changes`

#### Scenario: Attempt status tool returns structured status
- **WHEN** `vk.get_attempt_status` is called with a valid `attempt_id`
- **THEN** the tool returns a text content item containing a human-readable summary of the attempt status

#### Scenario: Tail logs tool returns recent logs
- **WHEN** `vk.tail_attempt_logs` is called with a valid `attempt_id`
- **THEN** the tool returns a text content item containing recent log output suitable for troubleshooting

#### Scenario: Attempt changes tool returns a concise diff summary
- **WHEN** `vk.get_attempt_changes` is called with a valid `attempt_id`
- **THEN** the tool returns a text content item containing a concise summary of changed files and/or patch information

### Requirement: Approval gating for mutating tools
If a VK Dynamic Tool can mutate VK state (task/attempt metadata, files, or execution), the system SHALL require explicit user approval before executing it.

#### Scenario: Mutating tool requires approval
- **WHEN** a mutating VK Dynamic Tool is requested
- **THEN** the system requests user approval before executing the tool
- **AND** executes the tool only if the user approves

### Requirement: Dynamic Tool activity is visible in logs
The system SHALL record Dynamic Tool call activity in a user-visible log stream.

#### Scenario: Tool call is logged
- **WHEN** a VK Dynamic Tool is executed
- **THEN** the system emits a log entry that includes the tool name, arguments summary, and success/failure outcome
