## ADDED Requirements

### Requirement: LLMAN Config Path
The system SHALL load llman Claude Code groups from a configurable config path, defaulting to `~/.config/llman/claude-code.toml` when unset.

#### Scenario: Default path resolution
- **WHEN** the llman path is unset and the default file exists
- **THEN** the system reads groups from the default path

#### Scenario: Override path resolution
- **WHEN** the llman path is set in the app config
- **THEN** the system reads groups from the override path

### Requirement: LLMAN Group Import
The system SHALL provide a manual import that creates or updates Claude Code profile variants for each `[groups.<name>]` entry, mapping each group to `LLMAN_<GROUP>` and setting `cmd.env` to the group's string key/value pairs.

#### Scenario: Group becomes variant
- **WHEN** the user runs the llman import and the config includes a `groups.minimax` table
- **THEN** the Claude Code executor includes a `LLMAN_MINIMAX` variant with matching env values

#### Scenario: Non-string values ignored
- **WHEN** a group contains non-string values
- **THEN** those entries are skipped and the remaining string entries are used

### Requirement: Import Update Rules
The system SHALL update the `cmd.env` map for existing `LLMAN_` variants on import, preserving other configuration fields.

#### Scenario: Existing variant env updated
- **WHEN** a `LLMAN_MINIMAX` variant already exists and the user re-imports
- **THEN** its `cmd.env` matches the llman group values and other fields remain unchanged

### Requirement: Imported Variants Are Persisted
The system SHALL persist imported `LLMAN_` variants in profile overrides.

#### Scenario: Saving profiles includes imported variants
- **WHEN** profiles are saved after a llman import
- **THEN** the saved overrides include the `LLMAN_` variants

### Requirement: Manual Re-Import Refreshes Variants
The system SHALL refresh imported variants only when the user triggers a re-import.

#### Scenario: Manual refresh
- **WHEN** the llman path is updated and the user re-imports
- **THEN** the available `LLMAN_` variants reflect the groups at the new path

### Requirement: Re-Import Does Not Prune Variants
The system SHALL NOT delete existing `LLMAN_` variants that are missing from the llman config during re-import.

#### Scenario: Removed group retained
- **WHEN** a previously imported `LLMAN_FOO` variant exists but `groups.foo` is removed from llman
- **THEN** the `LLMAN_FOO` variant remains after re-import
