## 1. Bulk Hydration (De-N+1)

- [x] 1.1 Implement bulk uuid resolution for Task foreign keys (project/workspace/task/shared_task/archived_kanban/milestone) and remove per-row `ids::*_uuid_by_id()` calls from list hydration.
- [x] 1.2 Implement bulk attempt status computation (running set + latest status + latest executor) and wire into list hydration.
- [x] 1.3 Implement bulk dispatch_state fetch for tasks list hydration.
- [x] 1.4 Implement bulk orchestration diagnostics fetch (milestone automation + project defaults + orchestration state) for candidate tasks.
- [x] 1.5 Refactor `find_all_with_attempt_status`, `find_by_project_id_with_attempt_status`, `find_filtered_with_attempt_status` to use the bulk hydration path.

## 2. Tests + Verification

- [x] 2.1 Add DB tests covering attempt status/executor/dispatch_state/orchestration behaviors for list hydration.
- [x] 2.2 Run `cargo test -p db`, `cargo test --workspace`, `just qa`, and `just openspec-check`.
