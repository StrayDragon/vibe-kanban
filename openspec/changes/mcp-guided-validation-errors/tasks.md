## 1. Specifications

- [ ] 1.1 Review the incremental spec at `openspec/changes/mcp-guided-validation-errors/specs/api-error-model/spec.md` for OpenSpec formatting (Requirement + `#### Scenario` WHEN/THEN) and alignment with the intended MCP error behavior; verify by inspection and `openspec status --change mcp-guided-validation-errors`.

## 2. Guided MCP Invalid-Params Errors

- [ ] 2.1 Add a single “guidance registry” (examples) used to derive `hint`, `details.next_tools`, and `details.example_args` for common fields; verify by unit testing at least `project_id` and `repo_id` mappings.
- [ ] 2.2 Enhance `crates/server/src/mcp/params.rs` (`Parameters<P>` parsing) to classify common decode failures and emit structured tool errors with stable `code` (`missing_required`, `invalid_uuid`, `unknown_field`, …) and actionable guidance fields; verify by locally calling a tool with missing required args using MCP Inspector.
- [ ] 2.3 Fix/expand id-discovery hint mapping (e.g. `session_id` guidance must reference existing tools like `tail_attempt_feed` / `list_task_attempts`, not non-existent tools); verify by grepping the hint text and ensuring all referenced tools exist in `tools/list`.
- [ ] 2.4 Make MCP request parsing strict by applying `#[serde(deny_unknown_fields)]` to MCP request structs (so typos like `projectId` produce `unknown_field`); verify via a unit test and an Inspector call.
- [ ] 2.5 Improve the top-level `ErrorData` message for missing-field cases (e.g. “Missing required field(s): project_id”) so Inspector surfaces a helpful headline; verify by reproducing the error in Inspector and checking the displayed message.
- [ ] 2.6 Standardize tool-level semantic validation errors (post-deserialization) to use the same structured error shape and codes (including optional `next_tools/example_args`); verify by triggering an in-tool validation failure such as invalid `list_tasks.status`.

## 3. Tests & Smoke Checks

- [ ] 3.1 Add unit tests for `Parameters<P>` invalid params classification (missing field + invalid UUID + unknown field) under `crates/server/` and assert the structured fields (`code`, `missing_fields`/`path`/`unknown_fields`, `hint`, `next_tools`, `example_args`, `retryable=false`); verify via `cargo test -p server`.
- [ ] 3.2 Add a lightweight regression test that validates all tool names referenced by guidance exist in `tools/list`; verify via `cargo test -p server`.
- [ ] 3.3 Run a manual Inspector smoke test (`just mcp-inspector`): call `list_tasks` with no args and confirm the response is actionable (includes `code=missing_required`, `details.next_tools`, and a hint to call `list_projects`); then retry with a valid `project_id` to confirm normal success.
