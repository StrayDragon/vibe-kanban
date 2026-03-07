## ADDED Requirements

### Requirement: Workspace removal honors configured cleanup hooks
Workspace removal flows SHALL honor configured project cleanup hooks.

#### Scenario: Remove-worktree action runs before-cleanup hook first
- **WHEN** a user confirms worktree removal for a project with a configured `before_cleanup` hook
- **THEN** the hook executes before workspace deletion begins
- **AND** the outcome follows the project's configured failure policy
