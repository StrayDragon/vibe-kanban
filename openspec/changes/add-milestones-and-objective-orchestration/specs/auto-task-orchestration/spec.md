## MODIFIED Requirements

### Requirement: Automatic orchestration of eligible internal tasks
The scheduler SHALL only auto-dispatch internal tasks that are eligible under current project/task settings and runtime state. The scheduler SHALL reuse the existing task-attempt runtime path instead of creating a separate execution pipeline.

#### Scenario: Eligible task is auto-dispatched
- **WHEN** a task is auto-managed, has no in-progress attempt, is not done/cancelled, and is otherwise eligible
- **THEN** the scheduler starts an attempt through the existing orchestration flow
- **AND** the task exposes dispatch state showing that it was claimed or is running

#### Scenario: Non-milestone grouped tasks stay unscheduled
- **WHEN** a task is a task-group entry task or belongs to a task group node
- **AND** the owning task group is not eligible for unattended milestone dispatch
- **THEN** the scheduler SHALL NOT auto-dispatch it
- **AND** the task SHALL expose a machine-readable reason that grouped tasks are not eligible for auto orchestration

#### Scenario: Milestone-managed grouped tasks are eligible for dispatch
- **WHEN** a task belongs to a task group node
- **AND** the owning task group is eligible for unattended milestone dispatch
- **AND** the owning project's execution mode is `manual` or `auto`
- **AND** the task group has no other node task with an in-progress attempt
- **AND** the node's predecessor nodes are all `done`
- **THEN** the scheduler SHALL treat the node task as eligible for dispatch through the existing orchestration flow

#### Scenario: Task-level manual override blocks milestone dispatch
- **WHEN** a task belongs to a task group node
- **AND** the task's automation mode is `manual`
- **THEN** the scheduler SHALL NOT auto-dispatch the task

#### Scenario: Manual project does not dispatch inherited tasks
- **WHEN** a task inherits automation settings from a project in `manual` mode
- **THEN** the scheduler SHALL NOT auto-dispatch the task
