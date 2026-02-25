# transactional-create-start Specification

## Purpose
TBD - created by archiving change add-transactional-create-start. Update Purpose after archive.
## Requirements
### Requirement: Transactional task and attempt creation
The system SHALL use database transactions for task/attempt create-and-start flows that write `task`, `workspace`, and `workspace_repo` records.

#### Scenario: Create failure rolls back
- **WHEN** any write fails during a create/start flow
- **THEN** the system does not leave partial `task`, `workspace`, or `workspace_repo` records

#### Scenario: WorkspaceRepo failure rolls back
- **WHEN** `workspace_repo` creation fails
- **THEN** `task` and `workspace` records are not persisted

### Requirement: Cleanup on start failure
When `start_workspace` fails after a successful commit, the system SHALL clean up any created `workspace` and `workspace_repo` records before returning an error.

#### Scenario: Start failure cleans workspace records
- **WHEN** `start_workspace` fails after commit
- **THEN** the created `workspace` and `workspace_repo` records are removed

#### Scenario: Start failure returns error
- **WHEN** `start_workspace` fails after commit and cleanup succeeds
- **THEN** the API returns an error and the database contains no leftover workspace records from that flow

