# Change: Update execution log history streaming

## Why
Long-running or highly parallel tasks can push server memory well past 3GB because log history is retained in memory and log WebSocket streams can remain open after execution finishes. The frontend also loads full attempt history into memory and keeps growing arrays for raw logs, which keeps browser memory high even when only the newest messages matter.

## What Changes
- Bound server-side in-memory log history with configurable limits and ensure log streams close after the Finished signal.
- Add a tail-first log history API with pagination for older entries, so the UI can lazy-load history on demand.
- Update the attempt conversation UI to show only the latest entries by default and allow loading earlier history with a spinner, while keeping a bounded client-side cache.
- Cap raw log viewer buffers and surface a "history truncated" indicator with an option to load more.

## Impact
- Affected specs: execution-logs
- Affected code:
  - crates/utils/src/msg_store.rs
  - crates/services/src/services/container.rs
  - crates/server/src/routes/execution_processes.rs
  - frontend/src/hooks/useConversationHistory.ts
  - frontend/src/components/logs/VirtualizedList.tsx
  - frontend/src/hooks/useLogStream.ts
  - frontend/src/components/tasks/TaskDetails/ProcessLogsViewer.tsx
  - frontend/src/components/tasks/TaskDetails/ProcessesTab.tsx
