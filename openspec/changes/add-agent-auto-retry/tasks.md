## 1. Implementation
- [ ] 1.1 Add auto-retry config fields to executor configs + JSON schemas; update defaults
- [ ] 1.2 Validate regex list, delay seconds, and max attempts on save
- [ ] 1.3 Detect recoverable failures on coding-agent completion and schedule delayed retry
- [ ] 1.4 Emit a system tip entry for auto-retry scheduling/execution
- [ ] 1.5 Enforce per-process max retry attempts and avoid loops
- [ ] 1.6 Add tests for regex matching and retry scheduling behavior
