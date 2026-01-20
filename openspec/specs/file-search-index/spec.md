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

