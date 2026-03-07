## ADDED Requirements

### Requirement: Auto-managed tasks may continue across bounded turns
The system SHALL allow bounded same-session continuation for eligible auto-managed tasks after a normal coding-agent completion.

#### Scenario: Continuation remains disabled until a project opts in
- **WHEN** an auto-managed project has not enabled any continuation budget
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

### Requirement: Continuation respects explicit budgets
The system SHALL enforce explicit continuation budgets for auto-managed work.

#### Scenario: Continuation stops at budget limit
- **WHEN** an auto-managed task reaches its configured continuation turn budget
- **THEN** the system SHALL NOT start another continuation turn
- **AND** the task exposes a structured stop reason describing budget exhaustion

### Requirement: Continuation yields to human gates
The system SHALL stop continuation when human review or approval is required.

#### Scenario: Review handoff blocks further continuation
- **WHEN** an auto-managed task enters a review-required state
- **THEN** the system SHALL NOT schedule another continuation turn

#### Scenario: Pending approval blocks further continuation
- **WHEN** a task has a pending approval
- **THEN** the system SHALL NOT schedule another continuation turn
