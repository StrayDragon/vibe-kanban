## Context
The system currently cleans workspaces on task deletion or periodic expiry, but it lacks an immediate, user-triggered cleanup path. The UI already exposes attempt-specific actions in the Actions dropdown, and backend services already support worktree cleanup and container_ref clearing.

## Goals / Non-Goals
- Goals:
  - Let users remove a task attempt's worktree without deleting task/attempt records.
  - Prevent removal while processes or dev servers are running.
  - Provide a clear confirmation flow that warns about data loss.
- Non-Goals:
  - Changing task status or deleting attempts/branches.
  - Altering the existing periodic cleanup policies.

## Decisions
- Decision: Add `POST /api/task-attempts/{id}/remove-worktree` with no request body.
  - The handler re-checks for running processes/dev servers, stops processes, cleans worktrees (no dirty check), removes workspace directory, and clears container_ref.
- Decision: Use existing workspace cleanup utilities.
  - Use `ContainerService::try_stop(..., include_dev_server = true)` and `WorkspaceManager::cleanup_workspace` + `Workspace::clear_container_ref` for consistent cleanup.
- Decision: Provide the action in both task and attempt menus.
  - Attempt view acts on the currently viewed attempt.
  - Task view prompts for attempt selection, defaulting to the latest attempt and only listing attempts with container_ref.
  - The action is hidden when no eligible attempt exists and disabled while processes are running.

## Risks / Trade-offs
- Removal is destructive and will discard uncommitted changes; the dialog must clearly warn users.

## Migration Plan
- No DB migrations required.
- Add new endpoint and UI wiring; existing data remains valid.

## Open Questions
- None.
