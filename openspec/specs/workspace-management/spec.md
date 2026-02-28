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

### Requirement: Configurable expired workspace cleanup (local deployment)
The system SHALL periodically clean up expired workspaces that have no running processes, and SHALL allow operators to tune the expiry TTL and cleanup interval via environment variables for local deployments.

#### Scenario: Defaults are used when env vars are unset
- **WHEN** `VK_WORKSPACE_EXPIRED_TTL_SECS` and `VK_WORKSPACE_CLEANUP_INTERVAL_SECS` are unset
- **THEN** the system uses the built-in defaults for the expiry TTL and cleanup interval

#### Scenario: TTL can be reduced by operators
- **WHEN** `VK_WORKSPACE_EXPIRED_TTL_SECS` is set to a lower value
- **THEN** workspaces older than that TTL and without running processes are eligible for cleanup

#### Scenario: TTL-based cleanup can be disabled
- **WHEN** `DISABLE_WORKSPACE_EXPIRED_CLEANUP` is set
- **THEN** the system SHALL NOT perform TTL-based expired workspace cleanup

