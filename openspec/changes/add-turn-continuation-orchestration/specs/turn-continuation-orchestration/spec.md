## ADDED Requirements

### Requirement: Auto-managed tasks may continue across bounded turns
The system SHALL allow bounded same-session continuation for eligible auto-managed tasks after a normal coding-agent completion.

#### Scenario: Continuation remains disabled until an effective budget is configured
- **WHEN** an auto-managed task's effective continuation budget is `0` (project default is `0` and task override is unset or `0`)
- **THEN** the system SHALL NOT schedule a continuation turn after normal completion
- **AND** current single-turn behavior remains unchanged

#### Scenario: Eligible task continues in the same session
- **WHEN** an eligible auto-managed task completes a coding-agent turn normally and still has remaining actionable work
- **THEN** the system starts a continuation turn in the same workspace
- **AND** the continuation uses the same coding-agent session when one exists

#### Scenario: Manual task never enters continuation
- **WHEN** a task is effectively manual
- **THEN** the system SHALL NOT schedule a continuation turn
- **AND** manual task behavior remains unchanged by continuation policy

### Requirement: Continuation budget inheritance and task overrides
The system SHALL derive an effective continuation turn budget from project defaults and optional per-task overrides.

#### Scenario: Task inherits the project default budget
- **WHEN** an auto-managed task has no task-level continuation override configured
- **THEN** the system SHALL use the project default continuation budget for eligibility and scheduling

#### Scenario: Task override disables continuation even when the project enables it
- **WHEN** the project default continuation budget is non-zero
- **AND** an auto-managed task sets its continuation override to `0`
- **THEN** the system SHALL NOT schedule a continuation turn for that task

#### Scenario: Task override enables continuation even when the project default is disabled
- **WHEN** the project default continuation budget is `0`
- **AND** an auto-managed task sets a positive continuation override
- **THEN** the system MAY schedule continuation turns for that task up to the override budget

#### Scenario: Task override takes precedence over the project default
- **WHEN** the project default continuation budget is non-zero
- **AND** an auto-managed task sets a positive continuation override
- **THEN** the system SHALL use the task override as the effective continuation budget

### Requirement: Continuation respects explicit budgets
The system SHALL enforce explicit continuation budgets for auto-managed work.

#### Scenario: Continuation stops at budget limit
- **WHEN** an auto-managed task reaches its effective continuation turn budget
- **THEN** the system SHALL NOT start another continuation turn
- **AND** the task exposes a structured stop reason describing budget exhaustion

### Requirement: Continuation prompts incorporate the previous turn outcome
The system SHALL provide continuation turns with prompts that keep context and momentum without restarting from scratch.

#### Scenario: Continuation prompt includes the previous turn summary and remaining budget
- **WHEN** the system schedules a continuation turn for an auto-managed task
- **THEN** the follow-up prompt includes a summary of the previous turn's outcome and the remaining continuation budget
- **AND** the prompt explicitly instructs the agent to continue from the current workspace state

### Requirement: Continuation yields to human gates
The system SHALL stop continuation when human review or approval is required.

#### Scenario: Review handoff blocks further continuation
- **WHEN** an auto-managed task enters a review-required state
- **THEN** the system SHALL NOT schedule another continuation turn

#### Scenario: Pending approval blocks further continuation
- **WHEN** a task has a pending approval
- **THEN** the system SHALL NOT schedule another continuation turn
