## Why

Vibe Kanban is increasingly operated by non-human clients (LLM agents and external orchestrators like Zirvox). Today’s MCP tool set supports “create task → start attempt → follow up”, but it lacks a minimal observability loop (attempt status, incremental logs, and changes summary), forcing clients to either scrape UI-only streams or reimplement bespoke API polling logic.

## What Changes

- Add **three** new MCP tools (snake_case, minimal surface):
  - `get_attempt_status`: given `attempt_id`, return attempt/workspace info plus latest session/process summary and a coarse state (`running|completed|failed`) with a lightweight failure summary.
  - `tail_attempt_logs`: given `attempt_id` and `cursor/limit`, return tail-first log history (raw + normalized) using existing cursor semantics.
  - `get_attempt_changes`: given `attempt_id`, return diff summary + changed-file list, respecting existing diff preview guard (blocked + reason) and omitting full contents by default.
- Prefer explicit identifiers (`attempt_id`) and avoid relying on MCP process cwd for context.
- Keep the MCP tool count small: this change is intentionally scoped to the smallest “machine orchestration closed loop”.

## Capabilities

### New Capabilities
- (none)

### Modified Capabilities
- `mcp-task-tools`: extend the MCP tool set to include `get_attempt_status`, `tail_attempt_logs`, and `get_attempt_changes` with stable, documented schemas and predictable error handling.

## Impact

- Backend (Rust/Axum): add small, composable endpoints/helpers to resolve latest session/execution-process for an attempt and to compute diff summary + changed files without streaming.
- MCP server: extend `crates/server/src/mcp/task_server.rs` with the three tools and JSON schemas (field descriptions, UUID/RFC3339 formats).
- Consumers:
  - LLM agents can stay within MCP for control + observability without learning the full HTTP/WS/SSE surface.
  - Zirvox may still use direct `/api` WS/SSE where appropriate, but it no longer needs to invent an observability model from scratch.

## Goals

- Enable a reliable, minimal attempt-level closed loop for non-human clients: status → logs → changes.
- Keep tool naming consistent (snake_case) and the tool set small enough for LLMs to select correctly.
- Reuse existing log cursor semantics and diff guardrails; avoid breaking existing UI flows.

## Non-goals

- No authentication / access-control work (assume same-host deployment for now).
- No MCP event subscription tool and no new long-lived streaming protocol in MCP for this phase.
- No `task.search` or tag-based workflows (defer until data model semantics are decided).
- No request-id idempotency/correlation protocol work beyond what existing IDs already provide.

## Risks

- **Polling pressure**: `tail_attempt_logs` could be called too frequently; mitigate with cursor/limit defaults and clear guidance in schema docs.
- **Diff cost**: even summary/file-list computation can be heavy on large repos; must respect existing diff preview guard and omit contents by default.
- **Failure summary ambiguity**: a “reason” may be best-effort; keep it coarse and sourced from process status + last error-like entry when available.

## Verification

- Add tests for attempt status resolution and diff guard behavior.
- Add MCP tool schema/handler tests for validation and stable response shapes.
- Manual smoke: run backend + `mcp_task_server`, call the three tools against a real attempt and confirm (a) status transitions, (b) cursor paging, (c) blocked diff handling.
