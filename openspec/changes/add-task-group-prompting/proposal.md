# Change: Add task-group prompting (node instructions + draft protection)

## Why
- Task Group nodes need per-node “instructions” to make agent prompting automation-friendly.
- The workflow UI should not lose unsaved edits when fresh server data arrives.

## What Changes
- Persist optional `TaskGroupNode.instructions` when creating/updating TaskGroup graphs.
- When starting an attempt from a TaskGroup node, append non-empty node instructions to the initial task prompt.
- UI: allow editing/clearing node instructions in the TaskGroup workflow view.
- UI: preserve unsaved workflow drafts on refresh; only replace draft after explicit save/discard.

## Impact
- New spec: `task-group-prompting`.
- Spec update: `workflow-orchestration` (draft preservation requirement).
- Code areas: task-group DB model + API, services prompt assembly, frontend `TaskGroupWorkflow`, tests.

