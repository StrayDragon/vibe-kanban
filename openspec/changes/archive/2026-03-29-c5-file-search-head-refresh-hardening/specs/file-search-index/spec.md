## ADDED Requirements

### Requirement: Background HEAD validation resolves OID without spawning git in common layouts
The system SHALL resolve the repository `HEAD` OID for background validation by reading Git metadata files without requiring Git command execution in common repository layouts, including worktree `gitdir/commondir` and `packed-refs`.

#### Scenario: Worktree repository resolves HEAD OID
- **WHEN** a repository is a Git worktree where `.git` is a `gitdir:` pointer and the worktree uses `commondir`
- **THEN** the system resolves the current `HEAD` OID correctly for background validation

#### Scenario: Packed refs repository resolves HEAD OID
- **WHEN** a repository stores refs in `packed-refs` (no loose ref file for the current branch)
- **THEN** the system resolves the current `HEAD` OID correctly for background validation

### Requirement: Truncated index rebuilds are throttled
The system SHALL enforce a minimum interval between index rebuilds for repositories whose file search index is truncated, even when `HEAD` changes frequently.

#### Scenario: Frequent HEAD changes do not trigger frequent rebuilds on truncated index
- **WHEN** a repository has a cached file search index marked as truncated
- **AND** the repository `HEAD` changes multiple times within the configured minimum rebuild interval
- **THEN** the system does not enqueue index rebuilds more frequently than the configured minimum rebuild interval

#### Scenario: Rebuild can occur after the minimum interval
- **WHEN** a repository has a cached file search index marked as truncated
- **AND** the repository `HEAD` has changed
- **AND** the configured minimum rebuild interval has elapsed since the last rebuild
- **THEN** the system enqueues an index rebuild
