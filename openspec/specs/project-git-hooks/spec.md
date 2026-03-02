# project-git-hooks Specification

## Purpose
TBD - created by archiving change settings-editor-none-and-config-cleanup. Update Purpose after archive.
## Requirements
### Requirement: Project overrides global git hook skipping
The system SHALL compute an effective git hook skipping setting using project override precedence.

The effective value SHALL be computed as:
- If a project override value is explicitly set, it takes precedence.
- Otherwise, the global default is used.

#### Scenario: Project override disables hook skipping
- **WHEN** the global `git_no_verify` setting is enabled
- **AND** the project override value is `false`
- **THEN** the effective setting is disabled

#### Scenario: Project override enables hook skipping
- **WHEN** the global `git_no_verify` setting is disabled
- **AND** the project override value is `true`
- **THEN** the effective setting is enabled

#### Scenario: Project inherits the global default
- **WHEN** the project override value is `null` (inherit)
- **THEN** the effective setting equals the global `git_no_verify` value

### Requirement: Git operations use the effective setting
When the effective hook skipping setting is enabled, the system SHALL pass `--no-verify` to git commit and merge operations initiated by Vibe Kanban.

#### Scenario: Merge uses --no-verify when enabled
- **WHEN** a merge is executed for a workspace whose project’s effective setting is enabled
- **THEN** the git merge invocation includes `--no-verify`

#### Scenario: Commit uses --no-verify when enabled
- **WHEN** a commit is executed for a workspace whose project’s effective setting is enabled
- **THEN** the git commit invocation includes `--no-verify`

### Requirement: Settings UI communicates override precedence
The Settings UI SHALL communicate that a project-specific configuration overrides the global default for git hook skipping.

#### Scenario: Global helper mentions project override
- **WHEN** viewing the global git hook skipping setting
- **THEN** the helper text indicates that projects can override this value

#### Scenario: Project settings exposes inherit/enabled/disabled
- **WHEN** viewing a project’s settings
- **THEN** the UI provides a control with at least: inherit, enabled, and disabled options for hook skipping

