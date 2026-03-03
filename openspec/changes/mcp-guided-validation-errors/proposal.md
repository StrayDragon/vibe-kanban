## Why

External orchestrators and developers frequently drive Vibe Kanban via MCP using schema-driven tooling (e.g. MCP Inspector). Today, a common mistake like calling `list_tasks` without `project_id` yields a generic parameter error that is technically correct but not actionable, causing repeated trial-and-error and reduced MCP adoption.

We want MCP tool invocation failures—especially “missing required inputs” and “invalid identifier formats”—to provide guided, next-step instructions so callers can quickly recover (e.g. “call `list_projects` to obtain `project_id`”).

## What Changes

- Improve MCP tool parameter validation errors to be **actionable and guide the next call**:
  - Detect missing required fields (e.g. `project_id`) and return a structured error with `code=missing_required`, `missing_fields=[...]`, and an explicit `hint` with the next tool(s) to call.
  - Improve invalid identifier errors (e.g. malformed UUIDs) with `code=invalid_uuid`, a precise `path`, and a `hint` pointing to the discovery tool that can provide a valid id.
  - Include machine-actionable guidance for orchestrators:
    - `details.next_tools=[{ tool, args }]` as a recommended recovery sequence
    - `details.example_args` as a copy/paste-able minimal payload for the next call (generated from a single in-code examples registry to avoid drift).
  - Fix/align any misleading hints (e.g. avoid referencing non-existent tools in guidance).
- Apply the same error model to both:
  - decode/deserialization failures (handler never runs)
  - per-tool semantic validation failures (e.g. invalid enum/status values)
- Make MCP request parsing stricter:
  - Reject unknown fields with `code=unknown_field` (so typos like `projectId` become self-correcting).
- Echo provided values in errors where helpful (e.g. invalid UUID), while redacting sensitive token-like fields.
- Keep tool input schemas and normal success responses unchanged; this change focuses on **error UX** for orchestrators/Inspector.

## Capabilities

### New Capabilities
- (none)

### Modified Capabilities
- `api-error-model`: Strengthen MCP invalid-params guidance requirements (structured codes + missing field lists + actionable hints).

## Impact

### Goals
- Make “invalid params” failures self-healing for orchestrators: errors should directly suggest the *next* tool call and required fields.
- Reduce time-to-debug when using MCP Inspector by improving the top-level error message and structured details.
- Keep behavior changes minimal and focused on parameter/validation errors (no new tools required).

### Non-goals
- Changing core tool semantics (e.g. making `project_id` optional) in this change.
- Adding new “help” tools or expanding the MCP tool set.

### Risks
- Orchestrators that pattern-match existing error strings could be impacted if the top-level message changes; structured fields should remain stable and are the recommended integration surface.
- Over-guidance can become stale if tool names/flows change; guidance must be kept in sync with the actual tool set.

### Verification
- Use MCP Inspector to reproduce common mistakes (e.g. call `list_tasks` without `project_id`, provide malformed UUIDs) and confirm:
  - Errors include stable `code`, `missing_fields` (when applicable), `path` (when applicable), and an actionable `hint`.
  - Suggested “next tools” exist and return the necessary ids for a correct retry.
