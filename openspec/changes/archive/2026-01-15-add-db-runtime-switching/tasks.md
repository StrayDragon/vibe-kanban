## 1. Implementation
- [x] 1.1 Update DBService initialization to use `DATABASE_URL` when present and default to the project SQLite path when missing.
- [x] 1.2 Fail fast with clear errors on invalid URLs; reject non-SQLite backends for now.
- [x] 1.3 Apply SQLite pragmas via `ConnectOptions::after_connect` and keep WAL/synchronous/busy_timeout settings.
- [x] 1.4 Remove the sqlite_master migration check and deletion behavior.
- [x] 1.5 Add tests for SQLite foreign key enforcement and URL handling (missing defaults + invalid fails) if feasible.
