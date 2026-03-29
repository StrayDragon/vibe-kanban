# file-search-index Specification

## Purpose
TBD - created by archiving change add-file-search-guardrails. Update Purpose after archive.
## Requirements
### Requirement: File search index cap
The system SHALL cap file search indexing per repository at a configurable maximum and mark results as partial when the cap is exceeded.

#### Scenario: Index exceeds the cap
- **WHEN** a repository contains more files than the configured index cap
- **THEN** the system truncates the index and reports that results are partial

### Requirement: Watcher guardrail for large repos
The system SHALL avoid registering file watchers for repositories whose index is truncated.

#### Scenario: Truncated index skips watchers
- **WHEN** the index is truncated due to the configured cap
- **THEN** the system does not register filesystem watchers for that repository

### Requirement: File search cache hit avoids synchronous Git HEAD checks
The system SHALL return file search results from an existing cached index without synchronously invoking Git commands to validate `HEAD` on every request, and SHALL validate `HEAD` asynchronously with TTL gating to keep the index eventually consistent.

#### Scenario: Warm cache search returns without blocking on HEAD validation
- **WHEN** a repository already has a cached file search index
- **AND** a client issues file search requests (for example on each keystroke)
- **THEN** the system returns results using the cached index without waiting for a Git `HEAD` validation step

#### Scenario: HEAD change triggers background refresh
- **WHEN** the repository `HEAD` OID changes after an index has been cached
- **THEN** the system schedules a background refresh to rebuild the index
- **AND** subsequent searches eventually reflect the updated `HEAD`

#### Scenario: HEAD validation is TTL-gated and coalesced
- **WHEN** the system validates `HEAD` for a repository in the background
- **THEN** the system SHALL NOT schedule another `HEAD` validation for that repository within the configured TTL window
- **AND** concurrent requests MAY coalesce into a single pending validation task

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

