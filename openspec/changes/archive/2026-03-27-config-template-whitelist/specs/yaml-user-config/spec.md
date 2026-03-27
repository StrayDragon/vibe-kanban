# yaml-user-config Specification (Delta)

## MODIFIED Requirements

### Requirement: Template resolution is deterministic and uses secret.env precedence
The system SHALL resolve environment placeholders in whitelisted YAML string fields using the following precedence: `secret.env` > process/system environment.

Supported placeholders:
- `{{env.NAME}}`
- `{{env.NAME:-default}}`
- `{{secret.NAME}}`

Templates MUST NOT be allowed in arbitrary string fields. If a non-whitelisted field contains template syntax, config validation SHALL fail with an error that references the field path.

#### Scenario: secret.env overrides system env
- **WHEN** `secret.env` contains `OPENAI_API_KEY=from_secret`
- **AND** the process/system environment contains `OPENAI_API_KEY=from_system`
- **AND** a whitelisted field contains `{{env.OPENAI_API_KEY}}`
- **THEN** the resolved value is `from_secret`

#### Scenario: Default value is used when var is missing
- **WHEN** a whitelisted field contains `{{env.OPENAI_API_KEY:-fallback}}`
- **AND** neither `secret.env` nor the process/system environment defines `OPENAI_API_KEY`
- **THEN** the resolved value is `fallback`

#### Scenario: Missing var without default fails
- **WHEN** a whitelisted field contains `{{env.OPENAI_API_KEY}}`
- **AND** neither `secret.env` nor the process/system environment defines `OPENAI_API_KEY`
- **THEN** config validation fails with an error that references `OPENAI_API_KEY`
- **AND** the system records the error for status/diagnostics
- **AND** a reload attempt keeps the last known good configuration active

#### Scenario: Template in a non-whitelisted field is rejected
- **WHEN** a non-whitelisted YAML string field contains `{{secret.SOME_KEY}}`
- **THEN** config validation fails with an error that references that field path

