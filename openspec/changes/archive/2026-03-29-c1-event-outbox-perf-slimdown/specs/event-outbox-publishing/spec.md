# event-outbox-publishing Specification (Delta)

## ADDED Requirements

### Requirement: Unpublished outbox entries are fetched in FIFO order with bounded limit
The system SHALL fetch unpublished outbox entries in ascending `created_at` order and SHALL respect a caller-provided batch limit.

#### Scenario: Fetch respects limit and ordering
- **WHEN** the outbox contains multiple unpublished entries with different `created_at` timestamps
- **AND** the caller fetches unpublished entries with a batch limit `N`
- **THEN** the system returns at most `N` entries
- **AND** the returned entries are ordered by `created_at` ascending

### Requirement: Marking published removes an entry from the unpublished set
The system SHALL mark an outbox entry as published by setting `published_at` to a timestamp.

#### Scenario: Mark published hides entry from unpublished fetch
- **WHEN** an unpublished outbox entry is marked as published
- **THEN** subsequent unpublished fetches do not return that entry

### Requirement: Dispatch failures are recorded and the entry remains unpublished
When an outbox entry fails to dispatch, the system SHALL increment `attempts` and SHALL record `last_error`, while keeping the entry unpublished.

#### Scenario: Mark failed increments attempts and keeps entry unpublished
- **WHEN** an unpublished outbox entry is marked as failed with an error message
- **THEN** the entry remains returned by unpublished fetches
- **AND** the entry’s `attempts` increases by 1
- **AND** the entry’s `last_error` equals the provided error message
