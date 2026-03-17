## ADDED Requirements

### Requirement: Sessions have an optional human-friendly name

The system SHALL store an optional `name` for each session.

If a session name is absent, the system SHALL display a deterministic fallback
label derived from the session ID (e.g., a UUID prefix) so the session remains
distinguishable in UI.

#### Scenario: Session name is absent

- **WHEN** a session has `name = null`
- **THEN** the UI displays a fallback label derived from the session ID

### Requirement: Users can rename a session

The system SHALL allow the user to rename an existing session.

The system SHALL trim whitespace and treat an empty string as clearing the name
(`null`).

#### Scenario: Rename a session

- **WHEN** the user submits a new non-empty name for a session
- **THEN** subsequent reads of that session return the updated name

#### Scenario: Clear a session name

- **WHEN** the user sets the session name to an empty string (or `null`)
- **THEN** the stored session name becomes `null`

### Requirement: Sessions are auto-named when created without a name

When a session is created without an explicit name, the system SHALL set a
best-effort auto-generated name based on the creation context.

Auto-naming MUST NOT override an explicitly provided name.

#### Scenario: Auto-name on creation

- **WHEN** a session is created without an explicit name and the backend has
  enough context to generate one
- **THEN** the created session has a non-null name

