# Module: Task Groups

## Goals
- Make Task Groups promptable and automation-friendly.
- Keep graph schema backward compatible.

## In Scope
- Use TaskGroupNode.instructions to influence agent prompt.
- UI support for editing node instructions.
- Minimal changes to TaskGroup create/update flows.

## Out of Scope / Right Boundary
- Automatic planner that generates graphs.
- Multi-agent scheduling or orchestration engine.
- Breaking changes to TaskGroup graph schema.

## Design Summary
- TaskGroupNode.instructions is optional and stored in the graph.
- When a task attempt starts from a Task Group node, append instructions to the prompt.
- Keep existing nodes functional when instructions is null.

## Testing
- Unit tests in task_group model for instructions persistence.
- Manual flow: create/edit node instructions, start attempt, verify prompt contains instructions.
