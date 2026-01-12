## 1. Implementation
- [ ] 1.1 Add MsgStore history metadata helpers (min index + eviction flags) for raw and normalized logs.
- [ ] 1.2 Update `log_history_page` to use DB fallback when in-memory history is evicted and to compute `has_more` from persistent storage.
- [ ] 1.3 Extend `LogHistoryPage` with a history-completeness flag and regenerate shared types.
- [ ] 1.4 Update raw log UI to show a partial-history hint and keep "Load more" tied to `has_more`.
- [ ] 1.5 Update conversation history UI to surface partial-history hints for normalized logs (if applicable).
- [ ] 1.6 Add/adjust tests covering eviction fallback and partial-history signaling.
