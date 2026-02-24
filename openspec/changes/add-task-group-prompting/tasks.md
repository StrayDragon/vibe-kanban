## 0. Scope & Constraints
- Scope: node instructions persistence + prompt append + workflow-draft preservation.
- Non-goals: changing the overall workflow orchestration model; changing task-group schema versions; redesigning the workflow UI.

## 1. Backend: persist instructions
- [ ] 1.1 Extend TaskGroup graph update/create paths to persist optional `TaskGroupNode.instructions`.
- [ ] 1.2 Treat empty/whitespace-only instructions as `null` / absent for storage consistency.
- [ ] 1.3 Add logs on update/start to confirm whether instructions were applied (without logging full prompt contents).

## 2. Backend: append instructions to prompt
- [ ] 2.1 When starting an attempt from a TaskGroup node, append non-empty node instructions to the initial prompt.
- [ ] 2.2 Ensure instructions are appended only once and do not override the base task prompt.

## 3. Frontend: edit instructions
- [ ] 3.1 Add/confirm UI affordance to edit node instructions in `frontend/src/pages/TaskGroupWorkflow/TaskGroupWorkflow.tsx`.
- [ ] 3.2 Ensure clearing instructions persists as empty/absent (matches backend normalization).

## 4. Frontend: preserve workflow drafts on refresh
- [ ] 4.1 Ensure server refreshes do not overwrite local unsaved workflow edits (dirty state preserves draft).
- [ ] 4.2 Only replace local draft with server state after explicit save/discard.

## 5. Tests
- [ ] 5.1 DB/model test: instructions persist and empty normalizes to null.
- [ ] 5.2 Services test: prompt append occurs only when instructions are non-empty.
- [ ] 5.3 Frontend test: editing + clearing node instructions updates payload.
- [ ] 5.4 Frontend test: receiving refreshed server data preserves unsaved draft edits.

## 6. Verification
- [ ] 6.1 `cargo test --workspace`
- [ ] 6.2 `pnpm -C frontend run test`
- [ ] 6.3 `pnpm -C frontend run check`
- [ ] 6.4 `pnpm -C frontend run lint`

## Acceptance Criteria
- Node instructions are persisted and returned consistently (empty becomes absent).
- Starting an attempt from a node appends instructions to the initial prompt only when present.
- Workflow view never loses unsaved edits on background refresh; save/discard restores sync with server.

