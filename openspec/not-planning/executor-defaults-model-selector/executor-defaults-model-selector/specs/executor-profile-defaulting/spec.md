## ADDED Requirements

### Requirement: Default variant is represented as null

The system SHALL treat the DEFAULT configuration for an executor profile as
`variant = null` (missing).

The system SHALL normalize `variant = "DEFAULT"` (case-insensitive, trimmed) to
`variant = null` at persistence boundaries so the stored representation is
canonical.

#### Scenario: Load legacy config with DEFAULT variant string

- **WHEN** the config file contains `executor_profile.variant = "DEFAULT"`
- **THEN** the normalized in-memory config has `executor_profile.variant = null`

#### Scenario: Save config persists canonical representation

- **WHEN** the user saves a config where the selected configuration is DEFAULT
- **THEN** the written config stores `executor_profile.variant = null`

### Requirement: New attempt default profile resolution is consistent

When creating a new attempt, the UI SHALL resolve a default
`executor_profile_id` using the following precedence order:

1. Milestone node override (locked)
2. User selection in the dialog
3. Last used coding-agent executor profile for this task/attempt
4. User system default executor profile

#### Scenario: Milestone node profile is locked

- **WHEN** the task is a milestone node with `executor_profile_id` set
- **THEN** the dialog selects that profile and disables editing

#### Scenario: Last used profile is used when available

- **WHEN** there is a previous attempt with a known last used coding-agent
  `executor_profile_id`
- **THEN** the dialog preselects that exact profile (including variant)

#### Scenario: Falls back to system default

- **WHEN** no last used coding-agent profile is available
- **THEN** the dialog preselects the user system `executor_profile`

### Requirement: Attempt summaries expose last used coding-agent profile

The system SHALL provide the last used coding-agent `executor_profile_id`
(executor + variant) for an attempt when returning attempt summaries used by the
attempt creation UI.

#### Scenario: Attempt list includes last used profile

- **WHEN** the client fetches attempts for a task with session metadata
- **THEN** each attempt includes the last used coding-agent
  `executor_profile_id`, or `null` if no coding-agent process has run yet

