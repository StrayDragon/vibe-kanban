## Context

Vibe Kanban’s MCP server (`mcp_task_server`) is used both by humans (via MCP Inspector) and by external orchestrators. A common failure mode is incorrect tool invocation due to:

- Missing required identifiers (e.g. calling `list_tasks` without `project_id`)
- Malformed identifiers (e.g. invalid UUID strings)
- Misunderstanding “where to get ids” in a multi-step flow (project → repo/task → attempt → session → approvals)

While tool descriptions already document `Required/Optional/Next`, runtime errors from parameter decoding/deserialization can still be generic (“Invalid tool parameters”) and lack guided remediation. This is especially painful in UI-driven tools (Inspector) where users expect the error itself to tell them what to do next.

Today, parameter decoding is centralized in `crates/server/src/mcp/params.rs` (`Parameters<P>`), which is the ideal place to standardize and improve invalid-params errors for all tools.

## Goals / Non-Goals

**Goals:**
- Make MCP parameter/validation failures actionable for both humans and orchestrators:
  - A stable `code` describing the failure kind (e.g. `missing_required`, `invalid_uuid`)
  - `missing_fields` and/or `path` when applicable
  - A `hint` that points to the next tool(s) to call to obtain valid values
- Ensure hints never reference non-existent tools and remain consistent with the current tool set.
- Keep changes isolated to the MCP layer (no new tools, no schema-breaking changes).

**Non-Goals:**
- Changing tool signatures to make required ids optional (smart defaults) in this change.
- Building a full JSON Schema validator for tool arguments.
- Modifying HTTP API error payloads beyond what is required for MCP alignment.

## Decisions

### 1) Centralize “guided invalid params” in `Parameters<P>` parsing

We keep the single choke point (`FromContextPart<ToolCallContext>` for `Parameters<P>`) as the source of truth for:
- Mapping serde decoding failures into a consistent structured error payload
- Adding id-discovery guidance

This avoids duplicating ad-hoc validation in each tool handler and guarantees uniform behavior across the tool set.

**Alternative:** Per-tool manual validation and custom errors.
- Rejected: inconsistent, higher maintenance, and misses cases where the handler never executes (deserialization fails first).

### 2) Introduce a small, stable error taxonomy for invalid params (Option A)

Use structured `code` values for parameter failures:
- `missing_required`: one or more required fields are absent
- `invalid_uuid`: a UUID field cannot be parsed
- `invalid_type`: wrong JSON type for a field
- `unknown_field`: client sent an unsupported field (if detectable)
- `invalid_params`: fallback when classification is unclear

In all cases, include:
- `tool`: tool name
- `path`: best-effort JSON path (or `missing_fields` list)
- `error`: original serde error string (for debugging)
- `hint`: next-step guidance when possible
- `retryable`: always `false` for parameter failures
- `next_tools`: recommended recovery tool sequence (when applicable)
- `example_args`: minimal payload for the suggested next call (when applicable)

**Alternative:** Keep `code=invalid_params` only.
- Rejected: loses machine-actionable detail and does not meet the “guided remediation” goal.

### 3) Generate `next_tools` and `example_args` from a single registry

To avoid duplicating guidance logic across tools (and to keep maintenance cost low), we introduce one in-code “guidance registry” that maps common fields (and/or tool+field) to:
- the recommended discovery tool(s) (e.g. `project_id` → `list_projects`)
- a minimal `example_args` object for the *next* call (e.g. `{ "project_id": "<uuid>" }`)

Both `hint` (human-readable) and `details.next_tools` / `details.example_args` (machine-readable) are derived from this registry.

### 4) Guidance is “id-discovery oriented” and tool-set aware

Hints focus on “where to get a valid id”:
- `project_id` → call `list_projects`
- `repo_id` → call `list_repos`
- `task_id` → call `list_tasks` or `get_task` (if you already have one)
- `attempt_id` → call `list_task_attempts` (and/or use `list_tasks.latest_attempt_id`)
- `session_id` → use `start_attempt`/`send_follow_up` responses, or `tail_attempt_feed.latest_session_id`
- `approval_id` / `execution_process_id` → call `list_approvals` / `tail_attempt_feed` to locate current pending approvals and process ids

We explicitly avoid referencing hypothetical tools (e.g. `get_context`) in hints.

### 5) Top-level error message should be specific (Inspector UX)

Inspector highlights the error message prominently. For common cases, prefer:
- “Missing required field(s): project_id” over “Invalid tool parameters”

Structured fields remain the primary machine-integration surface; the message is optimized for human debugging.

### 6) Apply the same structure to per-tool validation errors

Many tools perform semantic validation after deserialization (e.g. validating a status string). These errors SHOULD use the same structured shape and stable codes so orchestrators can apply uniform recovery strategies.

Implementation-wise, tools should use a shared helper to produce:
- consistent `code/hint/retryable/details`
- optional `next_tools/example_args` when there is an obvious discovery step

### 7) Be strict on unknown fields (self-correcting typos)

For MCP request structs, enable strict parsing (e.g. via `#[serde(deny_unknown_fields)]`) so typos like `projectId` produce a direct `unknown_field` error rather than a confusing “missing required project_id” downstream.

### 8) Echo values, but redact sensitive inputs

For debugging, we echo user-provided values in error `details` (e.g. `value` for `invalid_uuid`), except for sensitive fields (e.g. `*token*`, auth headers). Sensitive values MUST be redacted (e.g. `"<redacted>"`).

## Risks / Trade-offs

- **Breaking string matchers** → Mitigation: keep structured fields stable and documented; avoid removing fields; add tests.
- **Stale guidance** (tool name changes) → Mitigation: keep hint mapping close to tool definitions, add a smoke test that verifies hinted tools exist in `tools/list`.
- **Over-specific codes** → Mitigation: keep taxonomy small; fall back to `invalid_params` when uncertain.

## Migration Plan

- No data migration.
- Rollout as a normal server update.
- This intentionally changes `details.code` for some invalid-params cases (more specific codes). Integrators SHOULD rely on structured fields, not error strings.

## Open Questions

*(none for this change; decisions above scope the behavior.)*
