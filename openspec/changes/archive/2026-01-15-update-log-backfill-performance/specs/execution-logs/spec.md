## ADDED Requirements
### Requirement: Non-blocking legacy log backfill
The system SHALL run legacy JSONL backfill asynchronously and MUST NOT block service readiness.

#### Scenario: Startup without blocking
- **WHEN** the service starts with legacy logs present
- **THEN** health and API endpoints respond while backfill continues in the background

### Requirement: Bounded backfill completion tracking
The system MUST track backfill completion per execution/channel in a bounded cache with TTL.

#### Scenario: Cache bounded
- **WHEN** many executions are backfilled
- **THEN** completion tracking does not grow beyond configured limits and older entries expire
