## ADDED Requirements

### Requirement: Codex can run in plan-only mode

The system SHALL provide a Codex plan-only mode that can be selected via an
executor profile variant.

#### Scenario: Select Codex plan-only mode

- **WHEN** a user selects the CODEX `PLAN` profile variant for an attempt
- **THEN** the attempt runs Codex in plan-only mode

### Requirement: Plan-only mode does not mutate the workspace

In plan-only mode, the system SHALL prevent mutation of the workspace. This
includes:

- no filesystem writes (including patch application)
- no command execution that can change state

#### Scenario: Mutation tool calls are denied

- **WHEN** Codex attempts a mutation tool call while in plan-only mode
- **THEN** the system denies the request and no mutation occurs

### Requirement: Plan-only mode produces a structured plan output

In plan-only mode, Codex SHALL produce a structured plan that is surfaced to
the UI as Todo/Plan entries.

#### Scenario: Plan appears in UI

- **WHEN** Codex completes a plan-only run
- **THEN** the UI displays the latest plan steps in the Todo panel

