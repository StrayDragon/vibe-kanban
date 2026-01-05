## 1. Implementation
- [x] 1.1 Add config field for llman Claude Code config path and migrate config version.
- [x] 1.2 Add llman TOML parser and mapping to Claude Code profile variants with `LLMAN_<GROUP>` naming.
- [x] 1.3 Add server import endpoint that updates profiles from llman and persists overrides.
- [x] 1.4 Expose the llman path and an "Import from llman" action in settings UI.
- [x] 1.5 Add tests for TOML parsing, variant naming/collisions, import behavior, and config migration.
