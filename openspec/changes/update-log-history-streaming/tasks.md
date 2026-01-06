## 1. Backend streaming lifecycle
- [ ] 1.1 Add MsgStore helpers for tail retrieval and stream termination on Finished.
- [ ] 1.2 Update raw/normalized log WebSocket handlers to close after Finished and release MsgStore references.

## 2. Log history API
- [ ] 2.1 Add an HTTP endpoint for normalized entry history with limit + cursor.
- [ ] 2.2 Implement history retrieval for running processes from MsgStore tail.
- [ ] 2.3 Implement history retrieval for completed processes from DB logs (best-effort, bounded).

## 3. Frontend lazy-load UX
- [ ] 3.1 Update useConversationHistory to load tail first and page older entries on demand.
- [ ] 3.2 Add UI controls (Load earlier history + spinner) and preserve scroll position on prepend.
- [ ] 3.3 Bound raw log buffers and show a truncation indicator in ProcessLogsViewer.

## 4. Config and defaults
- [ ] 4.1 Add env-based defaults for history byte and entry limits.
- [ ] 4.2 Document new environment variables and default behavior.

## 5. Tests
- [ ] 5.1 Add unit tests for MsgStore tail and Finished termination.
- [ ] 5.2 Add frontend tests for lazy history loading and truncation indicator (as feasible).
