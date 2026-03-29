# git-remote-status-ttl Specification

## ADDED Requirements

### Requirement: Branch status remote refresh is TTL-gated and minimal
The system SHALL refresh remote tracking refs for branch status comparisons at most once per `repo + remote + branch` within a configurable TTL, and SHALL fetch only the required remote branch ref when refreshing.

#### Scenario: First remote status check refreshes required branch
- **WHEN** a client requests branch status that requires comparing a local branch against a remote-tracking branch
- **AND** no successful refresh has occurred within the configured TTL
- **THEN** the system performs a `git fetch` for the required remote branch ref
- **AND** the system computes ahead/behind using the refreshed remote-tracking ref

#### Scenario: Repeated checks within TTL do not refetch
- **WHEN** a client requests branch status that requires the same `repo + remote + branch` remote comparison
- **AND** a successful refresh has occurred within the configured TTL
- **THEN** the system SHALL NOT perform another remote fetch for that comparison
- **AND** the system computes ahead/behind using the existing remote-tracking refs

#### Scenario: Concurrent refresh does not block responses
- **WHEN** a remote refresh for a given `repo + remote + branch` is already in progress
- **THEN** additional branch status requests for that comparison return without waiting for the refresh to complete
- **AND** the system uses existing refs to compute the best-effort result

### Requirement: Remote refresh failures degrade gracefully
The system SHALL degrade gracefully when remote refresh fails and SHALL avoid repeated failure amplification.

#### Scenario: Fetch fails during branch status
- **WHEN** the system attempts to refresh remote tracking refs for branch status
- **AND** the fetch fails (for example due to network or auth errors)
- **THEN** the system still returns branch status with local-only fields populated when possible
- **AND** remote-ahead/behind fields MAY be absent (`null`) for that repo

#### Scenario: Failure cooldown prevents retry storms
- **WHEN** remote refresh attempts for a given `repo + remote + branch` are failing
- **THEN** the system enforces a short cooldown window before attempting another refresh for that comparison

