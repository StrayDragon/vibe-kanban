## ADDED Requirements

### Requirement: Activity feeds expose orchestration transition events
The system SHALL publish structured orchestration transition events through existing activity/feed surfaces.

#### Scenario: Review-ready transition is published
- **WHEN** an auto-managed task enters a review-required state
- **THEN** the activity/feed surfaces include a structured transition event describing that state change

#### Scenario: Human take-over transition is published
- **WHEN** a human pauses or takes over an auto-managed task
- **THEN** the activity/feed surfaces include a structured transition event with the persisted transfer reason
