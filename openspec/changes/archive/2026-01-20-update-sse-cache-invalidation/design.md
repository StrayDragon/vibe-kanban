## Context
The backend already exposes an SSE event stream at `/events` with JSON Patch updates for tasks, workspaces, and execution processes. The frontend still polls task attempts and branch status every 5 seconds, leading to redundant load.

## Goals / Non-Goals
- Goals:
  - Reduce 5s polling by using SSE-driven invalidation for task attempts and branch status queries.
  - Keep data fresh with a safe fallback when SSE is disconnected.
  - Avoid adding new backend endpoints.
- Non-Goals:
  - Replace existing WebSocket-based streams for tasks/projects/logs.
  - Change backend event formats.

## Decisions
- Use a single EventSource connection (provider/context) to `/api/events` and expose connectivity state to hooks.
- Listen only for `json_patch` SSE events and inspect patch paths to determine invalidations.
- Invalidate:
  - `taskAttempts` + `taskAttemptsWithSessions` for the related task when `/workspaces/<id>` patches include `task_id`.
  - `branchStatus` for the related attempt when `/workspaces/<id>` patches include the workspace id.
  - As a fallback, invalidate all `branchStatus` and `taskAttemptsWithSessions` queries on `/execution_processes/<id>` patches.
- Gate polling behind SSE connectivity and document visibility; when SSE is connected, polling is disabled. When SSE is disconnected, poll only while the tab is visible.

## Risks / Trade-offs
- SSE history replay on connection may trigger a burst of invalidations; mitigate by batching invalidations in a microtask or short debounce.
- Execution process updates do not map directly to workspaces; broad invalidation may refetch more than necessary.

## Migration Plan
- Frontend-only change; no data migrations required.
- Rollout via UI release; fallback polling ensures safety if SSE is unstable.

## Open Questions
- Should execution-process-triggered invalidation scope be narrowed further by correlating sessions to workspaces (requires new event data)?
