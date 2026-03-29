## ADDED Requirements

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
