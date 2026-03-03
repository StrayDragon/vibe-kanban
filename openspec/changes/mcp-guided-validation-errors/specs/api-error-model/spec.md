# api-error-model Specification (Incremental)

## ADDED Requirements (MCP)

### Requirement: MCP invalid params errors SHALL be guided and actionable
When an MCP tool call fails due to invalid/missing input parameters (including both deserialization/decoding failures and tool-level semantic validation), the system SHALL return a structured tool error (`isError=true`) whose `structuredContent` is an object and includes:
- `code` (stable string describing the failure kind)
- `retryable` (boolean; MUST be `false` for invalid params)
- `hint` (a human/actionable next step)
- `details` (machine-readable context)

For “missing required field(s)” failures, `details` SHALL include:
- `missing_fields: string[]`
- `next_tools: { tool: string, args: object }[]` (recommended recovery sequence)
- `example_args: object` (minimal payload for the next call)

For “invalid identifier” failures (e.g. malformed UUID), `details` SHALL include `path` and SHOULD include a hint describing how to discover a valid id (e.g. `project_id` → `list_projects`).
For invalid identifiers, `details` SHOULD also include `value` (the provided value) unless the field is sensitive (token-like), in which case it MUST be redacted.

For unknown/unsupported fields, the system SHALL return `code=unknown_field` and include `details.unknown_fields: string[]` and a hint pointing to the correct field name(s).

#### Scenario: list_tasks called without project_id is actionable
- **WHEN** a client calls `list_tasks` without providing `project_id`
- **THEN** the tool result is `isError=true` and includes `code=missing_required`, `details.missing_fields` containing `project_id`, and a hint to call `list_projects` to obtain a valid `project_id`

#### Scenario: list_tasks called with invalid project_id is actionable
- **WHEN** a client calls `list_tasks` with a malformed UUID in `project_id`
- **THEN** the tool result is `isError=true` and includes `code=invalid_uuid`, `details.path` pointing to `project_id`, and a hint to call `list_projects` to obtain a valid `project_id`

#### Scenario: list_tasks called with typo field is self-correcting
- **WHEN** a client calls `list_tasks` with `projectId` instead of `project_id`
- **THEN** the tool result is `isError=true` and includes `code=unknown_field`, `details.unknown_fields` containing `projectId`, and a hint describing the correct `project_id` field
