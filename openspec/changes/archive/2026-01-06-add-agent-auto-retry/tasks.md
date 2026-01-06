## 1. Implementation
- [x] 1.1 Add auto-retry config fields to executor configs + JSON schemas; update defaults
- [x] 1.2 Validate regex list, delay seconds, and max attempts on save
- [x] 1.3 Detect recoverable failures on coding-agent completion and schedule delayed retry
- [x] 1.4 Emit a system tip entry for auto-retry scheduling/execution
- [x] 1.5 Enforce per-process max retry attempts and avoid loops
- [x] 1.6 Add tests for regex matching and retry scheduling behavior
