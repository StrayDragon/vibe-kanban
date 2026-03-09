# frontend-i18n-a11y-baseline Specification

## Purpose
TBD - created by archiving change kanban-reliability-and-e2e. Update Purpose after archive.
## Requirements
### Requirement: Touched UI surfaces honor the selected language consistently
On the task/project management surfaces touched by this change, user-visible strings SHALL be rendered through the i18n system and SHALL update when the language setting changes.

#### Scenario: Language change updates visible labels without reload
- **WHEN** a user changes the application language
- **THEN** visible labels and tooltips on the affected task/project surfaces update without requiring a full page reload

#### Scenario: No mixed-language UI on a single surface
- **WHEN** a task/project surface is rendered with a selected language
- **THEN** primary UI labels on that surface are consistently in that language (no unintended hard-coded fallback mix)

### Requirement: Icon-only controls have accessible names
All icon-only interactive controls on the affected surfaces SHALL have accessible names (for example via `aria-label`) so they are discoverable by keyboard and assistive technologies.

#### Scenario: Icon-only button is discoverable by role and name
- **WHEN** a user or test queries for a button by role and accessible name
- **THEN** the icon-only control can be found and activated

### Requirement: Form controls have correct label association
Form controls on the affected surfaces SHALL have correct label association (label-to-control linkage) so assistive technologies can announce intent.

#### Scenario: Checkbox/toggle is accessible by label
- **WHEN** a user navigates a settings control using assistive tech conventions (label-based navigation)
- **THEN** the control is reachable and correctly announced by its label

