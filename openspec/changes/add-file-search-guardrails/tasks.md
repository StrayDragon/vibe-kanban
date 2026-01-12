## 1. Implementation
- [ ] 1.1 Add `VK_FILE_SEARCH_MAX_FILES` to cache budgets and log when truncation occurs.
- [ ] 1.2 Update file index build to enforce the cap and record `index_truncated` metadata.
- [ ] 1.3 Skip watcher registration for truncated repos and rely on TTL refresh.
- [ ] 1.4 Propagate `index_truncated` in search responses and regenerate shared types.
- [ ] 1.5 Add tests/diagnostics for truncation and watcher skip behavior.
