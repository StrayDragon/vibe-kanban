## ADDED Requirements

### Requirement: Preserve workflow drafts on refresh
The workflow view MUST preserve unsaved TaskGroup draft edits when refreshed TaskGroup data arrives, and SHALL only replace the draft after the user saves or discards changes.

#### Scenario: Refresh while editing preserves draft
- **WHEN** the workflow view receives updated TaskGroup data while the user has unsaved edits
- **THEN** the UI preserves the local draft and does not overwrite unsaved changes

