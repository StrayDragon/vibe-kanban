# Change: Update execution logs resilience (normalization + UI stability)

## Why
- Certain log-normalization paths can still panic or silently drop continuity when events are anomalous/out-of-order.
- The UI can suffer from unstable identity for log items (expensive re-renders, wrong associations) when history is prepended.
- Conversation history loading should clear deterministically when there are no processes to load.

## What Changes
- Backend: harden normalization and streaming paths to avoid panic; emit explicit “normalization error” entries and continue.
- Frontend: ensure `useConversationHistory` clears loading when there are no processes; ensure log rendering uses stable item identity (entry index / patchKey) rather than array indexes/JSON.stringify comparisons.
- Add targeted tests for both backend normalization resilience and frontend identity/loading behavior.

## Impact
- Spec updates: `execution-logs` (ADDED requirements).
- Code areas: executors normalization modules, event stream mapping, frontend hooks/components for logs, tests.

