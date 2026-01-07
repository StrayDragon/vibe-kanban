# Change: Update execution log history streaming

## Why
Long-running or highly parallel tasks can push server memory well past 3GB because log history is retained in memory and log WebSocket streams can remain open after execution finishes. The frontend also loads full attempt history into memory and keeps growing arrays for raw logs, which keeps browser memory high even when only the newest messages matter.

## What Changes
- Bound server-side in-memory log history with configurable limits and ensure log streams close after the Finished signal.
- Add tail-first log history APIs for normalized and raw entries with pagination and stable entry indexes.
- Introduce a persistent, indexed log entry store for both normalized and raw logs; support updates to existing entries (e.g., tool approval state).
- Move live log streaming to entry-indexed events (append/replace/finished) so clients can keep bounded windows without JSON patch index gaps.
- Add new v2 log endpoints/streams and remove legacy WS endpoints after tests pass.
- Update the attempt conversation UI to show only the latest entries by default and allow loading earlier history with a spinner, while keeping a bounded client-side cache.
- Cap raw log viewer buffers and surface a "history truncated" indicator with an option to load more.

## Impact
- Affected specs: execution-logs
- Affected code:
  - crates/utils/src/msg_store.rs
  - crates/db/migrations/ (new log entry tables)
  - crates/db/src/models/ (new log entry models)
  - crates/services/src/services/container.rs
  - crates/server/src/routes/execution_processes.rs
  - frontend/src/hooks/useConversationHistory.ts
  - frontend/src/components/logs/VirtualizedList.tsx
  - frontend/src/hooks/useLogStream.ts
  - frontend/src/utils/streamJsonPatchEntries.ts (new stream handler or replacement)
  - frontend/src/components/tasks/TaskDetails/ProcessLogsViewer.tsx
  - frontend/src/components/tasks/TaskDetails/ProcessesTab.tsx
