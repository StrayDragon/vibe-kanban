# Settings Table Layout

## ADDED Requirements

### Requirement: Config information is presented in a table layout
The Settings page SHALL present the Config section's read-only configuration status entries in a table layout with aligned columns.

#### Scenario: Config table renders with aligned columns
- **WHEN** the user opens the Settings page and the config status has been loaded successfully
- **THEN** the Config section shows entries in rows with an item label column, a value column, and an actions column

### Requirement: Copy actions copy full underlying values
The system SHALL provide a copy action for each copyable value in the Config and Projects sections, and SHALL copy the full underlying string (not a truncated display value) to the clipboard.

#### Scenario: Copy config path
- **WHEN** the user triggers the copy action for the `config.yaml` path
- **THEN** the clipboard contains the full `config.yaml` path value returned by the config status API response

#### Scenario: Copy project id
- **WHEN** the user triggers the copy action for a project id
- **THEN** the clipboard contains the full project id string for that row

### Requirement: Long values remain usable on narrow viewports
The Settings page SHALL render long path and command values without overlapping interactive controls, and SHALL remain usable on narrow viewports.

#### Scenario: Long path remains usable
- **WHEN** a config path or command value is longer than the viewport width
- **THEN** the user can still read or scroll the full value without clipping or overlapping the actions column

### Requirement: Configured projects are displayed in a table layout
The Settings page SHALL display the configured projects list in a table layout that includes each project's name and id.

#### Scenario: Projects table renders with name and id
- **WHEN** the projects list has been loaded successfully and contains at least one project
- **THEN** each project appears as a table row with both the project name and project id visible

#### Scenario: Empty projects state remains visible
- **WHEN** the projects list has been loaded successfully and contains zero projects
- **THEN** the Settings page shows an empty-state message in the configured projects area
