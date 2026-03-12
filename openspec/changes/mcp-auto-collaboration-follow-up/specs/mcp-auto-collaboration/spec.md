## Definitions

- **Auto-managed task**: a milestone node task inside a milestone with `automation_mode=auto`, where the task has a non-empty `milestone_node_id` and is not the milestone entry task itself.

## ADDED Requirements

### Requirement: MCP review handoff payload
The system SHALL expose a concise MCP-readable handoff payload for review-ready auto-managed tasks.

#### Scenario: Review-ready task returns a concise handoff payload
- **WHEN** an auto-managed task reaches a review-required state
- **THEN** an MCP caller can read a payload containing task identity, latest summary, diff summary, validation outcome, and recommended next actions
- **AND** the caller SHALL NOT need to scrape raw execution logs to decide between approve, rework, or take-over

#### Scenario: Non-review task reports handoff as unavailable
- **WHEN** a caller requests a handoff payload for a task or attempt that is not in a review-ready auto-managed state
- **THEN** the response succeeds with a structured unavailable or not-applicable result
- **AND** the caller can distinguish that state from a transport failure

#### Scenario: Missing handoff ingredients degrade gracefully
- **WHEN** one of the handoff ingredients is unavailable
- **THEN** the payload still returns successfully with structured unavailable markers
- **AND** the caller can distinguish unavailable data from empty results

### Requirement: Control transfer is explicitly reasoned
The system SHALL persist and expose structured reasons when control shifts between human-driven and auto-managed execution.

#### Scenario: Human takes over a managed task
- **WHEN** a human pauses or takes over an auto-managed task
- **THEN** the task exposes a persisted reason describing the transfer
- **AND** MCP reads reflect that reason without requiring log inspection

#### Scenario: Automation resumes after human intervention
- **WHEN** a task returns from human control to auto-managed execution
- **THEN** the task exposes a persisted resume reason
- **AND** MCP reads can distinguish resumed automation from a fresh first-run

### Requirement: Auto-managed executor policy is machine-readable
The system SHALL expose a machine-readable project policy result for requested auto-managed executor/profile variants.

#### Scenario: Requested profile is allowed
- **WHEN** an MCP caller requests an allowed executor/profile variant for auto-managed work
- **THEN** the effective selection is persisted and inspectable

#### Scenario: Requested profile is rejected by policy
- **WHEN** an MCP caller requests a disallowed executor/profile variant
- **THEN** the task remains persisted without silent escalation
- **AND** MCP reads expose a structured policy rejection reason
