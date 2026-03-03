# workspace-management Specification (Incremental)

## ADDED Requirements

### Requirement: Worktree ensure SHALL tolerate missing worktree metadata directory
The system SHALL treat a missing `<repo>/.git/worktrees/` directory as “no worktrees registered” and SHALL continue worktree ensure and creation flows without failing.

#### Scenario: First worktree creation when `.git/worktrees/` is absent
- **WHEN** the system ensures a worktree for a repo that has no `.git/worktrees/` directory
- **THEN** the ensure operation succeeds and the worktree is created at the expected filesystem path

### Requirement: Ensure SHALL recreate missing attempt branch from target branch
If a workspace’s attempt branch does not exist in a repo, the system SHALL recreate the attempt branch from that repo’s configured `target_branch` before creating/ensuring the worktree.

#### Scenario: Attempt branch is missing during ensure
- **WHEN** the system ensures a workspace whose attempt branch does not exist in the repo
- **AND** the workspace repo has a configured `target_branch` that exists
- **THEN** the attempt branch is created at the same commit as `target_branch`
- **AND** the worktree is created/ensured successfully

### Requirement: Worktree ensure SHALL be idempotent for healthy worktrees
The system SHALL treat repeated worktree ensure calls as a no-op when the worktree path exists and is registered in git.

#### Scenario: Worktree already exists and is registered
- **WHEN** the system ensures a worktree that is already present and registered
- **THEN** the operation succeeds without recreating the worktree
