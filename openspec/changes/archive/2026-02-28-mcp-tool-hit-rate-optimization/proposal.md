## Why

Vibe Kanban’s MCP tools are increasingly called by non-human clients (LLM agents and external orchestrators). Today the tool surface is usable but still easy to mis-call: ambiguous IDs (attempt/session/process), cursor semantics, and “optional but actually required” fields lead to low first-pass tool accuracy and retry loops.

Improving schema-driven guidance (descriptions + required fields) is the fastest way to increase agent hit-rate without building a second UI or inventing a separate protocol.

## What Changes

- Standardize MCP tool descriptions to be **agent-oriented and action-guiding** (inputs → decision → next step), optimized for tool selection and correct parameter filling.
- Tighten request/response schemas so “required in practice” becomes **required by schema** (use `oneOf`/branch schemas for multi-mode tools).
- Add optional `request_id` idempotency keys for mutating MCP calls (e.g. create task, start attempt, follow-up send/queue) so orchestrators can safely retry without creating duplicates.
- Normalize pagination semantics across tools:
  - `cursor` is **only** for paging older history.
  - `after_*` (e.g. `after_entry_index`) is for **incremental tailing** (new entries only).
- Add a small set of missing MCP tools required for an agent to fully replace “clicking the UI”:
  - `list_executors` (capability discovery; avoid hard-coded executor strings)
  - `stop_attempt` (stop/kill runaway attempt execution)
  - `tail_session_messages` (or `get_session_transcript`) to replay conversation context
  - `get_attempt_file` / `get_attempt_patch` for bounded artifact retrieval with explicit guardrails
- Improve MCP error payloads with consistent fields (`code`, `retryable`, `hint`, `details`) so agents can recover without guessing.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `mcp-task-tools`: add requirements for agent-guided tool descriptions, branch-specific schemas, consistent pagination semantics, and the additional tools above.

## Impact

- Backend (MCP): `crates/server/src/mcp/task_server.rs` (tool set, schema, descriptions, error model).
- Backend (HTTP API): may add or reuse endpoints for stopping attempts, reading transcripts, and bounded file/patch retrieval.
- Docs: agent-facing workflow guidance (e.g. `_MCP.md`) should stay aligned with tool behavior.

## Goals

- Increase agent tool-call hit rate (correct tool + correct params on the first try).
- Make the “closed-loop” workflow self-evident from tool descriptions: status → logs → changes → artifacts → follow-up/stop.
- Keep tool responses token-efficient (defaults are small; allow opt-in `detail`).

## Non-goals

- Authentication/authorization design (assumed same-host deployment for now).
- Adding long-lived streaming to MCP (prefer pull/pagination in MCP; SSE/WS stays in `/api` where needed).
- Reworking the task/attempt data model.

## Risks

- Some changes may be **BREAKING** for existing MCP clients if we split tools or tighten schemas.
- Overly long descriptions can increase token usage; we must keep them structured and short.
- Incomplete “hint” mapping can create new failure loops; needs coverage tests.

## Verification

- Tool-schema tests: ensure branch-required fields are enforced and errors point to the missing path.
- Behavior tests: simulate common agent mistakes (wrong cursor usage, missing IDs, cancel vs stop) and validate returned `hint`/`retryable`.
- Manual inspection: verify MCP tool list and descriptions in a real MCP client.
