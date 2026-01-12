# Change: Log replay resilience for evicted in-memory history

## Why
MsgStore history is bounded by `VK_LOG_HISTORY_MAX_BYTES`/`VK_LOG_HISTORY_MAX_ENTRIES`. When eviction occurs during a running process, the log history endpoints currently serve only in-memory entries, so older logs cannot be replayed even though persistent log entries exist. This can leave the UI stuck showing older history that cannot be loaded and obscures when history is actually missing.

## What Changes
- Detect in-memory eviction for raw and normalized history requests and fall back to persistent log entry storage when older entries are requested.
- Compute `has_more` based on persistent storage when eviction is detected and expose a history-completeness flag when older entries are missing.
- Surface a UI hint when log history is partial, distinct from the "load more" experience.

## Impact
- Affected specs: `execution-logs`
- Affected code: `crates/utils/src/msg_store.rs`, `crates/services/src/services/container.rs`, `crates/server/src/routes/execution_processes.rs`, `frontend/src/hooks/useLogStream.ts`, `frontend/src/hooks/useConversationHistory.ts`, `frontend/src/components/tasks/TaskDetails/ProcessLogsViewer.tsx`, `frontend/src/components/logs/VirtualizedList.tsx`, `crates/server/src/bin/generate_types.rs` (regenerate shared types)
