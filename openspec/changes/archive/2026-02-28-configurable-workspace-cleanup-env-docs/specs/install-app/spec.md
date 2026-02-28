## ADDED Requirements

### Requirement: Generated environment variable reference
The system SHALL provide a generated environment-variable reference document and SHALL validate it in CI to prevent documentation drift.

#### Scenario: Generated env doc exists
- **WHEN** the repository is checked out
- **THEN** `docs/env.gen.md` exists and documents supported environment-variable knobs

#### Scenario: CI validates the generated env doc
- **WHEN** CI runs
- **THEN** the `generate-env-docs:check` step fails if `docs/env.gen.md` is out of date

