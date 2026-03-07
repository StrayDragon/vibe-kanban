## ADDED Requirements

### Requirement: Auto-managed starts honor required workspace preparation hooks
Auto-managed task dispatch SHALL honor required workspace preparation hooks before starting coding-agent execution.

#### Scenario: Scheduler defers dispatch on required hook failure
- **WHEN** an auto-managed task belongs to a project whose blocking `after_prepare` hook fails
- **THEN** the scheduler SHALL NOT continue into coding-agent execution
- **AND** the task SHALL expose a structured non-dispatch reason for the hook failure
