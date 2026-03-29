## ADDED Requirements

### Requirement: Worktree diff preview reuses a single prepared index per request
When computing diff preview for worktree changes, the system SHALL stage the worktree into a temporary index at most once per repository per request and SHALL reuse that prepared index to derive diff status and diff statistics.

#### Scenario: Summary and file listing reuse one staging pass
- **WHEN** the system computes a worktree diff summary and a changed-file listing for the same repository within a single request
- **THEN** it stages the worktree into a temporary index at most once for that request
- **AND** it derives both status (name/status) and stats (numstat) from that same prepared index

#### Scenario: Patch generation does not repeat staging after guard evaluation
- **WHEN** the system generates a patch for selected changed paths after passing diff preview guard evaluation
- **THEN** it does not perform an additional staging pass for that repository within the same request

