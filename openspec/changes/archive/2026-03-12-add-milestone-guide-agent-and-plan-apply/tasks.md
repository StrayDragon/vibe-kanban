## 1. Plan Contract + Shared Types

- [x] 1.1 Add a versioned milestone plan contract (`MilestonePlanV1`) plus preview/apply response DTOs in Rust and regenerate TS types. Verification: `pnpm run generate-types:check`
- [x] 1.2 Add a distinct task created-by attribution for planner-created tasks (e.g. `TaskCreatedByKind::MilestonePlanner`) and ensure it round-trips through all task reads. Verification: `cargo test -p db && pnpm run generate-types:check`

## 2. Backend: Validate / Preview / Apply

- [x] 2.1 Implement server-side plan validation (schema_version, node ids unique, DAG, task references belong to project, tasks not linked to other milestones, baseline ref validity). Verification: `cargo test -p server`
- [x] 2.2 Add `POST /api/milestones/:id/plan/preview` that returns a deterministic diff summary and performs no writes. Verification: `cargo test -p server`
- [x] 2.3 Add `POST /api/milestones/:id/plan/apply` that applies the plan atomically (create missing tasks, update milestone metadata + graph) and is retry-safe via HTTP idempotency. Verification: `cargo test -p server && pnpm run prepare-db`
- [x] 2.4 Persist minimal plan provenance for applied plans (new table recommended) and surface it in milestone reads. Verification: `cargo test -p db -p server && pnpm run generate-types:check`

## 3. Frontend: Planning UX (Chat -> Preview -> Apply)

- [x] 3.1 Add a milestone workflow “Plan” surface that can show a detected plan payload (from agent output or pasted JSON) and render preview/apply actions. Verification: `pnpm run check && pnpm run lint`
- [x] 3.2 Implement plan preview call + diff rendering (metadata changes + nodes/edges summary + tasks-to-create list). Verification: `pnpm run check`
- [x] 3.3 Implement plan apply call with explicit confirmation and reliable state refresh. Verification: `pnpm run check`
- [x] 3.4 Add visual markers for planner-created tasks/nodes (badge + filterable indicator). Verification: `pnpm run check && manual UI smoke`
- [x] 3.5 Add a deterministic “Auto-wire topology” helper as fallback (optional, if not already covered by plan output). Verification: `pnpm run check`

## 4. Guide Agent Prompting (Milestone Entry Task)

- [x] 4.1 Add a dedicated “milestone planning” prompt template for guide attempts that instructs the agent to emit `MilestonePlanV1` in the canonical encoding. Verification: `cargo test -p server`
- [x] 4.2 Add a UI affordance to start/resume a guide agent attempt on the milestone entry task (without allowing milestone entry tasks to be merged). Verification: manual UI smoke

## 5. End-to-End Validation

- [x] 5.1 Add Playwright coverage for “agent emits plan -> UI previews -> apply -> graph updated” (`e2e/milestone-planning.spec.ts`). Verification: `pnpm run e2e:test -- e2e/milestone-planning.spec.ts`
- [x] 5.2 Run full checks. Verification: `just check && just lint && pnpm run e2e`
