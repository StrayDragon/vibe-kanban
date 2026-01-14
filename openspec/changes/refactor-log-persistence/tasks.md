## 1. Implementation
- [x] 1.1 Add a log persistence mode (auto-detect log_entries table; optional env override) to choose between log_entries-only and legacy JSONL.
- [x] 1.2 Update log persistence streams to avoid JSONL writes when log_entries mode is active, while still handling SessionId updates.
- [x] 1.3 Keep legacy backfill from JSONL to log_entries, and gate it to executions that still have legacy rows.
- [x] 1.4 Add JSONL retention cleanup for completed executions (configurable retention window, safe to disable).
- [x] 1.5 Add/adjust tests or diagnostics for backfill completeness and cleanup behavior.
