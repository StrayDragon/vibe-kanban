# Change: Refactor task runtime quality hotspots

## Why
The task runtime path includes blocking git operations on async request handlers and frontend history initialization that re-runs under streaming updates. These issues hurt responsiveness and reliability.

## What Changes
- Move CLI-backed git operations in request paths to blocking-safe execution boundaries.
- Ensure conversation history initial load runs once per attempt and does not reset during live stream updates.
- Split oversized follow-up UI logic into smaller modules while preserving behavior.

## Impact
- Affected specs: `task-runtime-quality`, `execution-logs`
- Affected code: `crates/services/src/services/git/mod.rs`, `crates/server/src/routes/task_attempts/handlers.rs`, `frontend/src/hooks/execution-processes/useConversationHistory.ts`, `frontend/src/components/tasks/TaskFollowUpSection.tsx`
- Out of scope: authentication policy changes.
