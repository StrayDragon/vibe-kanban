# diff-preview-guardrails Specification

## Purpose
TBD - created by archiving change add-diff-preview-guardrails. Update Purpose after archive.
## Requirements
### Requirement: Diff Preview Guard Presets
The system SHALL provide diff preview guard presets (Safe, Balanced, Relaxed, Off) and default to Balanced when no selection exists.

#### Scenario: Default preset applied
- **WHEN** the user has no configured preset
- **THEN** the system uses the Balanced thresholds for diff preview guard evaluation

#### Scenario: User-selected preset applied
- **WHEN** the user selects a preset in settings
- **THEN** the system applies the corresponding thresholds for diff preview guard evaluation

### Requirement: Guarded Diff Preview
When diff summary exceeds the active preset thresholds and the request is not forced, the system SHALL block diff preview rendering and return a summary with a blocked indicator.

#### Scenario: Preview blocked for large diff
- **WHEN** the diff summary exceeds thresholds
- **AND** the request is not forced
- **THEN** the system returns a blocked indicator and summary without streaming diff contents

### Requirement: Forced Diff Preview Override
The system SHALL allow users to force loading a diff preview after receiving a blocked response, subject to existing hard byte caps.

#### Scenario: Forced preview allowed
- **WHEN** a user requests a forced diff preview
- **THEN** the system attempts to stream full diff contents while applying existing per-file and cumulative byte caps

### Requirement: Summary Without Full Content Reads
The system SHALL compute diff summary without loading full file contents into application memory.

#### Scenario: Summary computation uses lightweight sources
- **WHEN** the system computes diff summary
- **THEN** it avoids loading full file contents into application memory and relies on lightweight metadata or diff stats

### Requirement: Worktree diff preview reuses a single prepared index per request
When computing diff preview for worktree changes, the system SHALL stage the worktree into a temporary index at most once per repository per request and SHALL reuse that prepared index to derive diff status and diff statistics.

#### Scenario: Summary and file listing reuse one staging pass
- **WHEN** the system computes a worktree diff summary and a changed-file listing for the same repository within a single request
- **THEN** it stages the worktree into a temporary index at most once for that request
- **AND** it derives both status (name/status) and stats (numstat) from that same prepared index

#### Scenario: Patch generation does not repeat staging after guard evaluation
- **WHEN** the system generates a patch for selected changed paths after passing diff preview guard evaluation
- **THEN** it does not perform an additional staging pass for that repository within the same request

