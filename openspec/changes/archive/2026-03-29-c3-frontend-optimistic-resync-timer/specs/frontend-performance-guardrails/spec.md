# frontend-performance-guardrails Specification (Delta)

## ADDED Requirements

### Requirement: Optimistic resync checks are scheduled, not polled
When the frontend maintains optimistic task state that may require a resync, it SHALL schedule the next optimistic-stale check using a one-shot timer based on the earliest eligible resync time, and SHALL NOT rely on a fixed high-frequency polling loop.

#### Scenario: Optimistic state triggers one-shot scheduling
- **WHEN** optimistic task state exists and a resync is not yet eligible due to time gates
- **THEN** the frontend schedules a single next check at the earliest eligible time
- **AND** it does not wake on a fixed 250ms polling loop while waiting
