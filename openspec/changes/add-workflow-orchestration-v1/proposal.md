# Change: Add task group orchestration v1

## Why
Teams need deterministic, reusable task group assets to orchestrate parallel agent work while minimizing conflicts and idle time.

## What Changes
- Add a TaskGroup capability scoped to a Project, represented as a DAG of nodes and edges.
- Define TaskGroup schema with nodes, edges, phases, baseline reference, checkpoint and merge nodes, plus planning metadata (agent role, cost, artifacts, instructions).
- Extend Task with `taskGroupId`, `taskGroupNodeId`, and `taskKind=group` for entry tasks; add TaskGroup status and suggested status.
- Provide a Project-scoped workflow view (React Flow) as the human-friendly UI for TaskGroups.
- Synchronize Task status with node status, derive entry task status, and enforce blocker edges.

## Impact
- Affected specs: workflow-orchestration (new)
- Affected code: task group storage/models, API surfaces, frontend workflow view
- Breaking changes: none (additive data and UI)
