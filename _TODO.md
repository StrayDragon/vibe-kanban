# Current Status

- Milestone hard cut refactor is implemented.
- Legacy project/task automation semantics are removed; automation now lives on milestone (`task_groups`) only.
- Frontend milestone settings UX, scheduler dispatch, prompt injection, and `run-next-step` enqueue are in place.
- Tests already run and passed earlier in this branch:
  - `pnpm run check`
  - `pnpm run lint`
  - `pnpm run prepare-db`
  - `cargo test -p db`
  - `cargo test --workspace`
  - `pnpm run generate-types:check`
  - `cargo test -p server mcp::task_server::tools`
  - `pnpm run e2e:test -- e2e/milestone-automation.spec.ts`

# Manual Validation Notes

- `just run 127.0.0.1 3001 0` was used to boot the release app locally.
- DevTools manual verification succeeded on 2026-03-09:
  - Auto milestone scenario:
    - milestone metadata rendered correctly in workflow details
    - `node-a` completed first, then `node-b` entered `In Review`
    - this confirmed one-at-a-time dispatch semantics
  - Manual milestone scenario:
    - `POST /api/task-groups/:id/run-next-step` returned `status: queued`
    - response included `candidate_task_id`
    - after scheduler consumption and reload, the candidate node moved to `In Review`
  - Non-eligible path also checked:
    - while a node was already active, `run-next-step` returned `status: not_eligible`

# Remaining Work

- Archive OpenSpec change: `openspec/changes/add-milestones-and-objective-orchestration`
- If desired before archive, re-run a final quick smoke after restarting the local app
- Push branch when ready

# Helpful Local Artifacts

- Manual validation seed data:
  - `/tmp/vk-manual-milestone.json`
  - `/tmp/vk-run-next-milestone.json`
