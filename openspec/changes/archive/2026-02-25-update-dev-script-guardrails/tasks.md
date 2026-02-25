## 1. Implementation
- [x] 1.1 Introduce validated dev-script command model (avoid raw `sh -c` execution path).
- [x] 1.2 Enforce workspace-root and working-directory validation for dev-script execution.
- [x] 1.3 Add policy checks for who/what can update and execute dev scripts in existing API model.
- [x] 1.4 Emit audit events/structured logs for dev-script updates and runs.
- [x] 1.5 Add backend tests for rejected unsafe scripts and accepted safe scripts.

## 2. Verification
- [x] 2.1 `cargo test -p executors parse_direct_command -- --nocapture`, `cargo test -p services validate_dev_script_update -- --nocapture`, and `cargo test -p server task_attempts -- --nocapture`
- [x] 2.2 `openspec validate update-dev-script-guardrails --strict`
