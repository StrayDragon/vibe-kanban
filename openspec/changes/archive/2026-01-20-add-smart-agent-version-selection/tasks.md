## 1. Implementation
- [x] 1.1 Add async command resolver + cached results in `executors` (pnpm > npm > npx, no pnpm dlx).
- [x] 1.2 Use resolved commands in executor builders and kick off resolver init at startup.
- [x] 1.3 Expose resolver metadata in API (`UserSystemInfo`) and regenerate shared types.
- [x] 1.4 Show resolved source/version and latest fallback notice in Agent Settings UI.
- [x] 1.5 Add tests for resolution ordering and fallback cases.
