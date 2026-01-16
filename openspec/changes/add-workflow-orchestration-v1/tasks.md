## 0. Discovery
- [ ] 0.1 Research executor interruption hooks (Claude Code, Codex) and define pause/stop/interrupt UI scope

## 1. Implementation
- [ ] 1.1 Add DB migration: create `task_groups` table (`id`, `project_id`, `title`, `description`, `status`, `baseline_ref`, `schema_version`, `graph_json`, timestamps)
- [ ] 1.2 Add DB migration: add `task_kind` (default `default`), `task_group_id`, `task_group_node_id` to `tasks` with indexes
- [ ] 1.3 Update models/types: TaskGroup entity, Task fields, and generated TS types
- [ ] 1.4 Implement graph validation (DAG, unique node ids, edges reference existing nodes, no self edges)
- [ ] 1.5 Implement entry task aggregation logic and TaskGroup suggested status derivation
- [ ] 1.6 Add API endpoints for task group CRUD, node layout updates, node/task status sync, TaskGroup status updates, and suggested status in responses
- [ ] 1.7 Build Project-scoped workflow view (React Flow) with node detail panel, TaskGroup status controls, suggested status display/apply, and layout persistence
- [ ] 1.8 Add Kanban entry task behavior for TaskGroups (distinct marker + open workflow view)
- [ ] 1.9 Implement checkpoint approval UX and merge node affordances
- [ ] 1.10 Add node stop/force stop controls wired to attempt stop endpoint
- [ ] 1.11 Add tests for graph validation, status transitions, entry task aggregation, suggested status derivation, TaskGroup status updates, and node stop actions
