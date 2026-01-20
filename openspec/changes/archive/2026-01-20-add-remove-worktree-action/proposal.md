# Change: Add manual worktree removal for task attempts

## Why
Users need a fast, explicit way to remove a task attempt's worktree without deleting the task or attempt, especially when they want to reclaim disk space or reset a broken workspace.

## What Changes
- Add a "Remove worktree" action in both the task view and attempt view menus.
- Task view: prompt to select an attempt (default to latest), then confirm removal.
- Attempt view: act on the currently viewed attempt.
- Hide the action when no eligible attempt/worktree exists and disable it while any attempt process or dev server is running.
- Add a confirmation dialog that warns about deleting the worktree directory and losing uncommitted changes (no dirty check).
- Add a backend endpoint that stops attempt processes, removes worktrees and workspace directory, and clears container_ref.
- Keep task/attempt records and branches intact so worktrees can be recreated later.

## Impact
- Affected specs: workspace-management (new)
- Affected code: frontend attempt actions menu and dialog, task attempts API, workspace cleanup logic
