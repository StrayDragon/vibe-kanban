## REMOVED Requirements

### Requirement: Settings UI communicates override precedence
**Reason**: VK’s canonical configuration workflow is YAML + schema validation rather than bespoke per-setting UI. Maintaining dedicated UI copy for override precedence is no longer required.
**Migration**: Use the YAML schema/docs to communicate precedence and configure `git_no_verify` via `config.yaml` project overrides.

## ADDED Requirements

### Requirement: Config schema communicates override precedence
The system SHALL communicate project-override precedence for git hook skipping via YAML schema descriptions and/or documented configuration guidance.

#### Scenario: Schema mentions precedence
- **WHEN** an operator inspects the YAML schema for the project git hook skipping setting
- **THEN** the schema description indicates that a project override takes precedence over the global default
