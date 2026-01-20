## ADDED Requirements
### Requirement: Project-scoped task groups
The system SHALL allow a Project to own one or more TaskGroups. A TaskGroup SHALL include nodes, edges, and layout data for the workflow view.

#### Scenario: Create task group for a project
- **WHEN** a user creates a task group for a project
- **THEN** the task group is stored and retrievable with its nodes, edges, and layout

### Requirement: Task group core fields
Each TaskGroup SHALL store `title`, `schemaVersion`, `baselineRef`, and `status`. `description` is OPTIONAL. `status` SHALL use the same value set as Task status.

#### Scenario: Store task group title
- **WHEN** a task group is created with a title
- **THEN** the title is persisted and returned in task group responses

#### Scenario: Store task group status
- **WHEN** a task group is created with a status value
- **THEN** the status is persisted and returned in task group responses

### Requirement: Task group status is user-controlled
TaskGroup status SHALL be user-editable. Node status changes SHALL NOT automatically update TaskGroup status. The system SHALL NOT auto-set TaskGroup status to `cancelled`.

#### Scenario: Manual status update
- **WHEN** a user updates the TaskGroup status to `inreview`
- **THEN** the TaskGroup status is updated and shown in the workflow view

#### Scenario: Nodes do not override TaskGroup status
- **WHEN** a TaskGroup has status `inreview` and nodes change status
- **THEN** the TaskGroup status remains `inreview`

### Requirement: Task group status suggestions
The system SHALL compute a read-only `suggestedStatus` based on node states using the same aggregation rules as entry task status. The system SHALL NOT overwrite TaskGroup status unless the user explicitly applies the suggestion.

#### Scenario: Suggested status updates without overriding
- **WHEN** node statuses change
- **THEN** the `suggestedStatus` updates and the TaskGroup status remains unchanged

#### Scenario: Apply suggested status
- **WHEN** a user applies the suggested status
- **THEN** the TaskGroup status is updated to the suggested value

### Requirement: Task group node base branch selection
The system SHALL allow each TaskGroup node to choose a base branch strategy for worktree creation:
- `topology` (default): use the most recent completed predecessor/merge output. When multiple predecessors are completed, the most recently completed predecessor/merge output SHALL be selected.
- `baseline`: use the TaskGroup `baselineRef`.
If no completed predecessors exist, the system SHALL fall back to `baselineRef`.

#### Scenario: Start task using topology base
- **WHEN** a node with base strategy `topology` is started and has a completed predecessor
- **THEN** the worktree is created from the predecessor/merge output

#### Scenario: Multiple predecessors choose most recent output
- **WHEN** a node with base strategy `topology` is started and multiple predecessors are Done
- **THEN** the worktree is created from the most recently completed predecessor/merge output

#### Scenario: Topology falls back to baseline
- **WHEN** a node with base strategy `topology` is started and no predecessors are completed
- **THEN** the worktree is created from the TaskGroup `baselineRef`

#### Scenario: Start task using baseline base
- **WHEN** a node with base strategy `baseline` is started
- **THEN** the worktree is created from the task group `baselineRef`

### Requirement: Task group baseline defaults
The system SHALL prefill `baselineRef` with the Project default branch in TaskGroup creation and allow the user to edit it.

#### Scenario: Prefill baselineRef on create
- **WHEN** a user opens TaskGroup creation
- **THEN** the baseline field defaults to the Project's default branch

### Requirement: Task group entry task
The system SHALL create exactly one Task with `taskKind=group` per TaskGroup. The entry Task SHALL reference the TaskGroup via `taskGroupId`, appear in the Kanban with a distinct marker, and open the workflow view when selected.

#### Scenario: Create task group creates entry task
- **WHEN** a user creates a task group
- **THEN** the system creates a single entry task linked to the TaskGroup

#### Scenario: Entry task uniqueness
- **WHEN** an entry task already exists for a TaskGroup
- **THEN** the system rejects creation of another `taskKind=group` task for the same TaskGroup

#### Scenario: Open task group from Kanban
- **WHEN** a user selects a task group entry task in the Kanban
- **THEN** the workflow view opens for the linked `taskGroupId`

### Requirement: Kanban task type badges
Kanban cards SHALL display a type badge that distinguishes `task`, `task group`, and TaskGroup `subtask` nodes.

#### Scenario: Task group entry badge
- **WHEN** a TaskGroup entry task appears in the Kanban
- **THEN** the card shows a `task group` badge

#### Scenario: Subtask badge
- **WHEN** a Task belongs to a TaskGroup node
- **THEN** the card shows a `subtask` badge

#### Scenario: Regular task badge
- **WHEN** a Task is not linked to any TaskGroup
- **THEN** the card shows a `task` badge

### Requirement: Task group navigation affordances
Tasks linked to a TaskGroup SHALL expose a direct UI affordance to open the TaskGroup workflow view.

#### Scenario: Navigate from a subtask card
- **WHEN** a user views a TaskGroup subtask card in the Kanban
- **THEN** the card provides a quick way to open the TaskGroup workflow

### Requirement: Kanban task group hierarchy
Kanban columns SHALL visually group tasks that share a TaskGroup and indicate hierarchy between the TaskGroup entry and its subtasks.

#### Scenario: Grouped tasks in a column
- **WHEN** multiple tasks in a column share a TaskGroup
- **THEN** they are grouped under a TaskGroup header with hierarchical styling for subtasks

#### Scenario: Ungrouped tasks remain flat
- **WHEN** a task is not part of any TaskGroup
- **THEN** it appears outside any TaskGroup grouping

### Requirement: Task kind defaults
The system SHALL support `taskKind` values `default` and `group`. Tasks without a `taskKind` value SHALL be treated as `default`.

#### Scenario: Backward-compatible task kind
- **WHEN** a legacy task without `taskKind` is loaded
- **THEN** it is treated as a `default` task

### Requirement: Entry task validation
Tasks with `taskKind=group` SHALL require `taskGroupId` and SHALL NOT set `taskGroupNodeId`.

#### Scenario: Reject entry task without TaskGroup
- **WHEN** a task is created or updated with `taskKind=group` and no `taskGroupId`
- **THEN** the system rejects the change

#### Scenario: Reject entry task node linkage
- **WHEN** a task is created or updated with `taskKind=group` and a `taskGroupNodeId`
- **THEN** the system rejects the change

### Requirement: Task-group node linkage
Each TaskGroup node SHALL reference exactly one Task, and a Task SHALL store `taskGroupNodeId` when linked to a TaskGroup node. A Task MAY belong to at most one TaskGroup node.

#### Scenario: Link task to task group node
- **WHEN** a task group node is created for an existing task
- **THEN** the node stores the task id and the task stores the `taskGroupNodeId`

### Requirement: Task project alignment
All Tasks referenced by a TaskGroup SHALL belong to the same Project as the TaskGroup.

#### Scenario: Reject cross-project tasks
- **WHEN** a task group node references a task from another project
- **THEN** the system rejects the change

### Requirement: Node layout persistence
Each TaskGroup node SHALL store layout coordinates (`x`, `y`) for the workflow view. Changes to node layout SHALL be persisted.

#### Scenario: Persist node position
- **WHEN** a user moves a node in the workflow view
- **THEN** the node layout coordinates are saved and restored on reload

### Requirement: Node configuration metadata
Each TaskGroup node SHALL expose configuration aligned with Task creation, including executor profile selection (agent + configuration), optional node instructions, and base branch strategy. Node title and description SHALL be sourced from the linked Task.

#### Scenario: View node configuration
- **WHEN** a user views a task group node
- **THEN** the workflow view displays the task title/description and node configuration (executor profile, base strategy, instructions)

### Requirement: Executor profile assignment
Task group nodes SHALL allow an `executorProfileId` used to preselect the agent and configuration when starting the task.

#### Scenario: Start task with executor profile
- **WHEN** a node with an `executorProfileId` is started
- **THEN** the attempt defaults to the configured agent and configuration

### Requirement: Executor profile fallback
If the configured `executorProfileId` is unavailable, the system SHALL allow manual selection before starting the task.

#### Scenario: Manual agent selection
- **WHEN** a node is started and the configured `executorProfileId` is unavailable
- **THEN** the user is prompted to choose an available agent profile

### Requirement: Graph validation
The system SHALL enforce that task group graphs are directed acyclic graphs (DAG). Node ids MUST be unique. Edges MUST reference existing nodes and MUST NOT be self-referential.

#### Scenario: Reject cyclic dependency
- **WHEN** an edge addition creates a cycle
- **THEN** the system rejects the change and preserves the prior graph

#### Scenario: Reject invalid edge
- **WHEN** an edge references a missing node or the same node on both ends
- **THEN** the system rejects the change

### Requirement: Schema version compatibility
The system SHALL reject TaskGroup payloads with an unsupported `schemaVersion`.

#### Scenario: Reject unsupported version
- **WHEN** a TaskGroup payload is submitted with an unknown `schemaVersion`
- **THEN** the system rejects the change and returns an error

### Requirement: Blocker dependencies
All edges SHALL be treated as blocker dependencies in v1. A node SHALL NOT be startable until all predecessor nodes are Done. Node status SHALL mirror the linked Task status.

#### Scenario: Upstream incomplete
- **WHEN** any predecessor task is not Done
- **THEN** the successor node remains NotReady and cannot be started

### Requirement: Merge nodes
The system SHALL support a `merge` node kind to represent integration tasks that depend on multiple predecessors. Merge nodes SHALL NOT require manual approval unless explicitly configured.

#### Scenario: Merge readiness
- **WHEN** all predecessors of a merge node are Done
- **THEN** the merge node becomes Ready like a task node

#### Scenario: Merge approval optional
- **WHEN** a merge node is not configured for approval
- **THEN** it proceeds like a normal task node without a gatekeeper step

### Requirement: Phase grouping
Each TaskGroup node SHALL define a `phase` integer representing a logical batch for grouping and planning. Dependencies are still expressed only by edges.

#### Scenario: Parallel nodes in the same phase
- **WHEN** two nodes share the same phase and have no blocking edges
- **THEN** they are eligible to be started in parallel

### Requirement: Checkpoint nodes
The system SHALL support a `checkpoint` node kind that requires manual approval before downstream nodes can become Ready.

#### Scenario: Gatekeeper approval
- **WHEN** all predecessors of a checkpoint are Done
- **THEN** the checkpoint enters a pending approval state and downstream nodes remain blocked until approval

### Requirement: Edge data flow labels
Edges SHALL allow an optional `dataFlow` label for documentation and display only.

#### Scenario: Display data flow label
- **WHEN** an edge defines a dataFlow label
- **THEN** the workflow view displays the label without altering dependency behavior

### Requirement: Task group entry status
Task group entry task status SHALL be derived from the linked node states.

#### Scenario: Entry status in review
- **WHEN** any node is InReview
- **THEN** the entry task status is InReview

#### Scenario: Entry status in progress
- **WHEN** any node is InProgress and no node is InReview
- **THEN** the entry task status is InProgress

#### Scenario: Entry status done
- **WHEN** all nodes are Done
- **THEN** the entry task status is Done

#### Scenario: Entry status cancelled
- **WHEN** all nodes are Cancelled
- **THEN** the entry task status is Cancelled

#### Scenario: Entry status todo
- **WHEN** none of the above conditions are met
- **THEN** the entry task status is Todo

#### Scenario: Entry status independent from TaskGroup status
- **WHEN** the TaskGroup status is updated manually
- **THEN** the entry task status continues to follow node-derived rules

### Requirement: Entry task status is read-only
The system SHALL prevent manual status updates on task group entry tasks.

#### Scenario: Reject manual status update
- **WHEN** a user attempts to set the status of an entry task
- **THEN** the system rejects the change

### Requirement: Task group entry task is non-executable
Task group entry tasks SHALL NOT create execution attempts. Starting an entry task SHALL open the workflow view instead.

#### Scenario: Prevent entry task execution
- **WHEN** a user attempts to start an entry task
- **THEN** the system redirects to the workflow view without creating an attempt

### Requirement: Task group deletion cascades tasks
Deleting a TaskGroup SHALL delete all linked node Tasks using the standard Task deletion flow, then delete the entry Task. Deleting the entry Task SHALL delete the TaskGroup and linked node Tasks.

#### Scenario: Delete TaskGroup cascades tasks
- **WHEN** a user deletes a TaskGroup
- **THEN** linked node tasks are deleted via the standard Task deletion flow and the entry task is removed

#### Scenario: Delete entry task cascades TaskGroup
- **WHEN** a user deletes a task group entry task
- **THEN** the TaskGroup and its linked node tasks are deleted

### Requirement: Project workflow view
The system SHALL provide a Project-scoped workflow view to create, edit, and monitor task group nodes and edges.

#### Scenario: Edit workflow graph in project
- **WHEN** a user adds or removes nodes or edges in the workflow view
- **THEN** the stored task group graph updates and the view reflects the change

### Requirement: Task group creation tabs
The system SHALL present Task and TaskGroup creation modes as tabs in the create modal. TaskGroup creation SHALL omit executor, repo selection, and auto-start controls.

#### Scenario: Switch to TaskGroup tab
- **WHEN** a user selects the TaskGroup tab in the create modal
- **THEN** TaskGroup fields are shown and Task execution fields are hidden

### Requirement: Node task detail view
The workflow view SHALL display the linked Task detail (including conversation) when a node is selected.

#### Scenario: Open task detail from node
- **WHEN** a user selects a node in the workflow view
- **THEN** the linked Task detail is shown without leaving the workflow view

### Requirement: Node interruption controls
The workflow view SHALL allow users to stop or force stop a running node task. Stop is best-effort and MAY fall back to force stop when the executor does not support graceful interrupt.

#### Scenario: Stop node execution
- **WHEN** a user chooses Stop on a running node
- **THEN** the system requests a stop for the current attempt without the force flag

#### Scenario: Force stop node execution
- **WHEN** a user chooses Force Stop on a running node
- **THEN** the system requests a stop for the current attempt with the force flag
