## Definitions

- **Auto-managed task**: a milestone node task inside a milestone with `automation_mode=auto`, where the task has a non-empty `milestone_node_id` and is not the milestone entry task itself.

## ADDED Requirements

### Requirement: MCP task tools expose a focused review handoff reader
The MCP task tool set SHALL include a focused read surface for review-ready auto-managed outcomes.

#### Scenario: Dedicated review handoff tool is discoverable
- **WHEN** an MCP client lists available tools
- **THEN** the tool list includes a read-only handoff tool for review-ready auto-managed tasks
- **AND** the tool publishes an output schema for machine parsing

#### Scenario: Handoff tool accepts task or attempt context
- **WHEN** a client provides either a task identifier or an attempt identifier for a review-ready outcome
- **THEN** the handoff tool resolves the latest relevant review state
- **AND** the response identifies which task and attempt the handoff payload describes
