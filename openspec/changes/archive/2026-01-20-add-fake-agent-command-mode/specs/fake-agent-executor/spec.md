## ADDED Requirements
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
