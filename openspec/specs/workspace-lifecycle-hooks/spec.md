# workspace-lifecycle-hooks Specification

## Purpose
TBD - created by archiving change add-workspace-lifecycle-hooks. Update Purpose after archive.
## Requirements
### Requirement: Projects may define guarded after-prepare hooks
The system SHALL allow a project to define an optional guarded `after_prepare` workspace hook.

#### Scenario: Hooks are disabled by default
- **WHEN** a project has not configured an `after_prepare` hook
- **THEN** workspace preparation continues with VK's existing behavior
- **AND** no hook outcome is recorded for that phase

#### Scenario: After-prepare hook runs after workspace materialization
- **WHEN** a workspace is created or prepared for an attempt in a project with `after_prepare` enabled
- **THEN** the hook runs after the workspace exists and VK has written its generated workspace files

#### Scenario: Required after-prepare hook blocks task start
- **WHEN** a project's `after_prepare` hook is configured with a blocking failure policy
- **AND** the hook fails
- **THEN** the coding-agent execution SHALL NOT start
- **AND** the task exposes a structured diagnostic describing the hook failure

### Requirement: Projects may define guarded before-cleanup hooks
The system SHALL allow a project to define an optional guarded `before_cleanup` workspace hook.

#### Scenario: Before-cleanup hook runs before explicit worktree removal
- **WHEN** a user removes a worktree for a project with `before_cleanup` enabled
- **THEN** the hook runs before workspace files are deleted

#### Scenario: Best-effort before-cleanup hook does not block removal
- **WHEN** a project's `before_cleanup` hook uses a warning-only failure policy
- **AND** the hook fails
- **THEN** workspace cleanup still proceeds
- **AND** the failure is recorded for operator inspection

### Requirement: Hook outcomes are inspectable
The system SHALL expose the latest workspace hook outcome without requiring raw log inspection.

#### Scenario: Workspace surfaces show latest hook outcome
- **WHEN** a client reads workspace or attempt detail for a hooked project
- **THEN** the response includes the latest completed hook phase and its outcome

