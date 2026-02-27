## 0. Scope & Constraints
- Scope: Add three MCP tools to complete the minimal attempt-level closed loop: status → logs → changes.
- Constraints: snake_case tool names; keep tool count small; same-host deployment; no auth/access-control work in this change.
- Non-goals: no MCP streaming/events tool, no `task.search`, no idempotency/correlation protocol work.

## 1. Backend: attempt aggregation APIs

- [x] 1.1 Add `GET /api/task-attempts/{id}/status` that returns attempt/workspace + latest session + latest relevant execution process + coarse `state` (verify: add a server test covering `idle` and `running/failed` states from seeded DB rows).
- [x] 1.2 Add `GET /api/task-attempts/{id}/changes?force=` that returns diff `summary`, `blocked/blockedReason`, and a changed-file list (no contents) with repo-prefixed paths (verify: add a server test that returns `blocked=true` when guard triggers and `blocked=false` when forced, using a small temp git repo fixture).

## 2. MCP: minimal observability tools

- [x] 2.1 Add `get_attempt_status` MCP tool (schemas + handler) backed by `/api/task-attempts/{id}/status` (verify: unit test/tool test asserts required fields + UUID/RFC3339 formatting).
- [x] 2.2 Add `tail_attempt_logs` MCP tool that resolves `latest_execution_process_id` for the attempt and returns cursor-paged tail logs (default `normalized`) (verify: unit test covers “no process → empty page” and “cursor returns older page” behavior).
- [x] 2.3 Add `get_attempt_changes` MCP tool backed by `/api/task-attempts/{id}/changes` and ensure blocked/force semantics match the spec (verify: unit test covers blocked vs forced responses).
- [x] 2.4 Update MCP server instructions string to recommend the workflow: `get_attempt_status` → `tail_attempt_logs` → `get_attempt_changes` (verify: tools list contains the three new names and instructions mention them).

## 3. Specs

- [x] 3.1 Update `openspec/specs/mcp-task-tools/spec.md` to include the three new tools and requirements (verify: spec includes tool names and scenario coverage for status/logs/changes).

## 4. Verification

- [x] 4.1 `cargo test --workspace`
- [x] 4.2 Manual smoke: run `pnpm run dev`, then run `cargo run --bin mcp_task_server` and call the three new tools against a real attempt; confirm status, cursor paging, and diff blocked/force behavior.
