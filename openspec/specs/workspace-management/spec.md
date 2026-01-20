# workspace-management Specification

## Purpose
TBD - created by archiving change add-remove-worktree-action. Update Purpose after archive.
## Requirements
### Requirement: Manual worktree removal action
The system SHALL provide a "Remove worktree" action in both task and attempt views for attempts that have an existing worktree/container reference.

#### Scenario: Attempt view action uses current attempt
- **WHEN** a task attempt is being viewed and has a container_ref
- **THEN** the Attempt actions menu offers "Remove worktree" for that attempt

#### Scenario: Task view action requires an eligible attempt
- **WHEN** a task has one or more attempts with a container_ref
- **THEN** the Task actions menu offers "Remove worktree"

#### Scenario: Task view selects an attempt
- **WHEN** the user selects "Remove worktree" from the task view
- **THEN** the system prompts for an attempt selection (unless only one eligible attempt), defaulting to the latest attempt and only listing attempts with a container_ref

#### Scenario: Action hidden when no worktree exists
- **WHEN** no attempts have a container_ref
- **THEN** the action is not shown

#### Scenario: Action disabled when processes are running
- **WHEN** the attempt has running processes or a dev server
- **THEN** the action is disabled with an explanation

### Requirement: Confirmation before removal
The system SHALL require confirmation before removing worktrees and SHALL warn that uncommitted changes will be lost.

#### Scenario: Confirmation dialog warns about data loss
- **WHEN** the user selects "Remove worktree"
- **THEN** the dialog warns about deleting the worktree directory and losing uncommitted changes

### Requirement: Cleanup execution
The system SHALL stop attempt-related processes, remove worktree directories and git metadata, remove the workspace directory, and clear container_ref while preserving task/attempt records and branches.

#### Scenario: Successful removal
- **WHEN** the user confirms removal
- **THEN** the workspace is cleaned and the attempt remains available for recreation

#### Scenario: Removal proceeds with uncommitted changes
- **WHEN** uncommitted changes exist and the user confirms removal
- **THEN** the cleanup proceeds and changes are discarded

