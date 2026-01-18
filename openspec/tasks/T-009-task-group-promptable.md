# Task: T-009 Task Group Promptability

## Background / Motivation
- Issue: P2-TG-01
- Evidence: Task Group nodes have instructions but are not used to shape prompts.

## Scope
### In Scope
- Use TaskGroupNode.instructions to influence agent prompt when starting an attempt.
- Surface instructions editing in TaskGroupWorkflow UI.

### Out of Scope / Right Boundary
- Automatic graph generation.
- Multi-agent orchestration engine.
- Schema changes that break existing graphs.

## Design
### Proposed
- When create_task_attempt detects a Task Group node:
  - If node.instructions is present, append it to task.to_prompt().
- Implement by adding optional prompt override to ContainerService::start_workspace.
- UI: add a small instructions editor per node in TaskGroupWorkflow.

### Alternatives Considered
- Store instructions in Task.description (would change task semantics).

## Change List
- crates/server/src/routes/task_attempts.rs: derive instructions for task group node.
- crates/services/src/services/container.rs: accept prompt override for CodingAgentInitialRequest.
- crates/db/src/models/task_group.rs: ensure instructions is preserved in graph updates.
- frontend/src/pages/TaskGroupWorkflow.tsx: edit node instructions.
- shared/types.ts: regenerate if types change.

## Acceptance Criteria
- Starting a task attempt from a Task Group node includes instructions in the prompt.
- Existing Task Groups without instructions behave unchanged.
- pnpm -C frontend run test passes.

## Risks & Rollback
- Risk: prompt format regression.
- Rollback: remove prompt override logic.

## Effort Estimate
- 2-3 days.

## Acceptance Scripts
```bash
# Unit test to verify prompt includes instructions (to be added with this task)
cargo test -p services task_group_instructions_append_to_prompt
```
Manual:
- In Task Group UI, edit a node's instructions.
- Start a task attempt from that node.
- Verify the initial prompt contains the instructions content.
