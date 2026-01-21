# tag-management Specification

## Purpose
TBD - created by archiving change add-tag-bulk-import. Update Purpose after archive.
## Requirements
### Requirement: Bulk tag import from Markdown
The system SHALL allow users to import multiple tags from a Markdown file by parsing headings and confirming a preview before applying changes.

#### Scenario: Parse headings into tag entries
- **WHEN** a user uploads a Markdown file containing headings from `#` to `######` formatted as `@tag_name`
- **THEN** the system extracts tag names without the leading `@` and uses the text between that heading and the next heading (or end-of-file) as the tag content in the preview

#### Scenario: Deduplicate imported tag names
- **WHEN** the uploaded file contains multiple headings that map to the same tag name
- **THEN** the preview shows a single entry per tag name using the last occurrence in the file

#### Scenario: Confirm import
- **WHEN** the user confirms the preview
- **THEN** the system creates tags that do not already exist and reports a successful import

#### Scenario: Confirm updates for duplicate names
- **WHEN** any imported tag name already exists
- **THEN** the system requires a second confirmation before updating existing tag content

#### Scenario: Cancel import
- **WHEN** the user cancels at the preview or duplicate-confirmation step
- **THEN** no tags are created or updated

