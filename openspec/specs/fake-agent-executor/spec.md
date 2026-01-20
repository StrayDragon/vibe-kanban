# fake-agent-executor Specification

## Purpose
TBD - created by archiving change add-fake-agent-executor. Update Purpose after archive.
## Requirements
### Requirement: Fake agent availability
The system SHALL provide a Fake agent executor that can be selected in executor profiles and used in all runtime environments.

#### Scenario: Select Fake agent
- **WHEN** a user selects the Fake agent executor in settings
- **THEN** new task attempts run using the Fake agent executor

### Requirement: Codex-compatible streaming
The Fake agent SHALL emit Codex-compatible JSONL events, including a session identifier and streaming assistant message deltas, so the existing log normalization and UI streams are exercised.

#### Scenario: Stream assistant output
- **WHEN** a Fake agent run starts
- **THEN** the log stream emits a session configuration event and streaming assistant deltas until completion

### Requirement: Deterministic simulation controls
The Fake agent SHALL support a configurable seed and timing parameters so repeated runs can generate deterministic event sequences.

#### Scenario: Repeatable output
- **WHEN** a Fake agent run is executed twice with the same seed and config
- **THEN** the emitted event sequence is identical in order and content

### Requirement: Safety by default
The Fake agent MUST NOT execute real filesystem or network operations and MUST only simulate tool events.

#### Scenario: Safe execution
- **WHEN** the Fake agent emits tool events
- **THEN** no external command or file modification is performed

### Requirement: Command-mode trigger
The Fake agent SHALL interpret prompts that start with a configured command prefix (default `help`, `?`, or `(`) as command-mode input and SHALL ignore prefixes that appear after other text.

#### Scenario: Prefix at start triggers command mode
- **WHEN** the prompt begins with `help`, `?`, or `(`
- **THEN** the Fake agent uses command-mode parsing

#### Scenario: Prefix mid-text is ignored
- **WHEN** the prompt contains `help` after other text
- **THEN** the Fake agent uses the default random simulation

### Requirement: Command sequencing
The Fake agent SHALL accept multiple commands in a single prompt (newline or `;` separated) and emit corresponding events in the same order.

#### Scenario: Multi-command run
- **WHEN** the prompt starts with `help exec; mcp`
- **THEN** exec events are emitted before mcp events

### Requirement: Built-in command coverage
The Fake agent SHALL provide commands that emit current tool/event sequences (exec_command, apply_patch, mcp, web_search, reasoning, warning/error, message) to exercise UI and log normalization paths.

#### Scenario: Built-in exec command
- **WHEN** the `exec` command is issued
- **THEN** the Fake agent emits exec begin/output/end events (and approvals when enabled)

### Requirement: Arbitrary event emission
The Fake agent SHALL accept a command that emits arbitrary codex `EventMsg` JSON (and raw JSON-RPC notifications) so new event types can be tested without code changes.

#### Scenario: Emit new event type
- **WHEN** command `emit { ... }` is provided for a valid EventMsg
- **THEN** the Fake agent outputs the corresponding `codex/event` notification

### Requirement: Session configured injection
The Fake agent SHALL emit a SessionConfigured event at the start of command-mode runs when the command sequence does not include one.

#### Scenario: Missing session configured
- **WHEN** a command-mode run omits SessionConfigured
- **THEN** the Fake agent prepends a SessionConfigured event

