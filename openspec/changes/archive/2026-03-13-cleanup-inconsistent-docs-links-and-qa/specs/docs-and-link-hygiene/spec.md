## ADDED Requirements

### Requirement: Main navigation contains no external Docs link
The UI SHALL NOT provide a “Docs” navigation entry that opens an external documentation website.

#### Scenario: Docs is not present in the main navigation menu
- **WHEN** a user opens the main navigation menu
- **THEN** the menu does not contain a “Docs” item

### Requirement: Support link is limited to upstream GitHub and opens in a new tab
The UI SHALL provide a “Support” link that points to the upstream GitHub issues page and opens in a new tab without navigating the app.

#### Scenario: Clicking Support opens upstream issues without navigating
- **WHEN** a user activates “Support” from the main navigation menu
- **THEN** a new tab opens to `https://github.com/BloopAI/vibe-kanban/issues`
- **AND** the current app tab remains on the same route it was on before the click

### Requirement: Onboarding safety notice contains no external docs URL
The safety notice shown during onboarding SHALL present guidance inline and SHALL NOT include external documentation URLs.

#### Scenario: Disclaimer dialog shows safety guidance without external links
- **WHEN** the app displays the onboarding safety notice dialog
- **THEN** the dialog body includes safety guidance text
- **AND** the dialog does not render a link to an external docs website

### Requirement: Release notes do not load external hosted content
If the app is configured to show release notes on startup, it SHALL NOT attempt to load release notes from an external hosted website.

#### Scenario: Startup with release-notes flag does not navigate externally
- **WHEN** the app starts with `config.show_release_notes=true`
- **THEN** the app does not open an external URL or embed external iframe content for release notes
- **AND** the app clears `config.show_release_notes` so the same external fetch is not retried on the next startup
