## REMOVED Requirements

### Requirement: Optional project-level orchestration mode
**Reason**: Unattended dispatch is milestone-scoped. `Project.execution_mode` is removed to avoid maintaining two automation models.
**Migration**: Create a milestone (task group), enable milestone automation, or use “run next step” to advance one eligible node via scheduler enqueue.

### Requirement: Task-level automation override
**Reason**: `Task.automation_mode` is removed. Tasks are human-started by default; milestones own the automation policy.
**Migration**: Move automated work into milestone nodes and enable milestone automation (or enqueue one step). Pause milestone automation to take over manually.

### Requirement: Visible automation diagnostics and control surfaces
**Reason**: Task-level automation lanes and ownership indicators keyed to `automation_mode` are removed in favor of milestone-first goal tracking.
**Migration**: Use milestone surfaces (workflow view) for progress and “what runs next” decisions, while task surfaces continue to show attempt status and dispatch state.

### Requirement: MCP automation controls remain safe and explicit
**Reason**: There is no longer a task-level automation write surface to control safely.
**Migration**: Use milestone-scoped APIs/tools to enable automation or enqueue the next step.

## MODIFIED Requirements

### Requirement: Automatic orchestration of eligible internal tasks
The scheduler SHALL only auto-dispatch tasks that are eligible under milestone orchestration rules and current runtime state. The scheduler SHALL reuse the existing task-attempt runtime path instead of creating a separate execution pipeline.

#### Scenario: Regular tasks are never auto-dispatched
- **WHEN** a task is not linked to a milestone/task group node
- **THEN** the scheduler SHALL NOT auto-dispatch it

#### Scenario: Milestone-managed node task is dispatched
- **WHEN** a task belongs to a milestone/task group node
- **AND** the milestone has automation enabled or an enqueued “run next step” request
- **AND** the milestone has no other node task with an in-progress attempt
- **AND** the node's predecessor nodes are all `done`
- **AND** the node task is not terminal
- **THEN** the scheduler SHALL dispatch the node task through the existing orchestration flow

