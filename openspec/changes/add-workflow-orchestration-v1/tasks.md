## 0. Discovery
- [x] 0.1 Research executor interruption hooks (Claude Code, Codex) and define pause/stop/interrupt UI scope
- [x] 0.2 Define TaskGroup deletion semantics (cascade task deletion order, entry task handling)
- [x] 0.3 Confirm baseline default strategy for v1 (project default branch vs. topology-derived)

## 1. Implementation
- [x] 1.1 Add DB migration: create `task_groups` table (`id`, `project_id`, `title`, `description`, `status`, `baseline_ref`, `schema_version`, `graph_json`, timestamps)
- [x] 1.2 Add DB migration: add `task_kind` (default `default`), `task_group_id`, `task_group_node_id` to `tasks` with indexes
- [x] 1.3 Add DB migration: partial unique index for entry tasks (`task_kind=group`) and enforce unique node linkage
- [x] 1.4 Update models/types: TaskGroup entity, Task fields, and generated TS types
- [x] 1.5 Implement graph validation (DAG, unique node ids, edges reference existing nodes, no self edges)
- [x] 1.6 Implement entry task lifecycle: auto-create on TaskGroup create, cascade delete, and block invalid `task_kind=group` payloads
- [x] 1.7 Implement entry task aggregation logic and TaskGroup suggested status derivation
- [x] 1.8 Add API endpoints for task group CRUD, node layout updates, node/task status sync, TaskGroup status updates, and suggested status in responses
- [x] 1.9 Build Task/TaskGroup creation modal tabs and TaskGroup create flow (default baseline, no executor/auto-start)
- [x] 1.10 Build Project-scoped workflow view (React Flow) with node detail panel, TaskGroup status controls, suggested status display/apply, and layout persistence
- [x] 1.11 Add Kanban entry task behavior for TaskGroups (distinct marker + open workflow view)
- [x] 1.12 Implement checkpoint approval UX and merge node affordances
- [x] 1.13 Add node stop/force stop controls wired to attempt stop endpoint
- [x] 1.14 Add tests for graph validation, entry task lifecycle, status transitions, suggested status derivation, TaskGroup status updates, and node stop actions
- [x] 1.15 Update TaskGroup node schema to store executor profile selection and base strategy; remove cost/artifacts/agentRole UI fields
- [x] 1.16 Update attempt creation to use node executor profile selection and base strategy (topology vs baseline)
- [x] 1.17 Add node detail controls for executor profile and base strategy selection
- [x] 1.18 Document topology base selection behavior (multi-predecessor + fallback) in spec/design
- [x] 1.19 Remove unused delete-mode variant warning in task deletion flow
- [x] 1.20 Run devtools smoke checks (fake agent selectors, base strategy fallback copy)
- [x] 1.21 Confirm TaskGroup entry task appears in Kanban after create (devtools verification)
- [x] 1.22 Add Kanban type badges for task/task group/subtask cards
- [x] 1.23 Add TaskGroup navigation affordances for grouped tasks in Kanban
- [x] 1.24 Add Kanban hierarchical grouping for TaskGroup tasks (group headers + indented subtasks)
