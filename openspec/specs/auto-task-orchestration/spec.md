# auto-task-orchestration Specification

## Purpose
TBD - created by archiving change add-optional-auto-orchestration. Update Purpose after archive.
## Requirements
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

### Requirement: Retry and review lifecycle
The system SHALL persist automation lifecycle state for auto-managed tasks, including retries, blocked conditions, and human-review handoff.

#### Scenario: Failed task is scheduled for retry
- **WHEN** an auto-managed task attempt fails and the retry limit has not been reached
- **THEN** the task stores `retry_scheduled` dispatch state
- **AND** the task includes the next retry timestamp

#### Scenario: Retry limit blocks further dispatch
- **WHEN** an auto-managed task has exhausted its retry budget
- **THEN** the task stores blocked dispatch state
- **AND** the task exposes the blocking reason without requiring log inspection

#### Scenario: Successful execution awaits human review
- **WHEN** an auto-managed task completes execution and enters review without a failed last attempt
- **THEN** the task stores dispatch state indicating that human review is required
- **AND** the scheduler SHALL NOT immediately redispatch the task

### Requirement: Versioned auto-orchestration workflow prompt
The system SHALL keep a repo-versioned workflow prompt for unattended auto-orchestration runs. The prompt SHALL be rendered from `vk` task context rather than external tracker-specific wording.

#### Scenario: Auto-managed task renders a `vk`-native prompt
- **WHEN** the scheduler starts an auto-managed task attempt
- **THEN** the workflow prompt includes the task's identifier, title, description, repository/workspace context, and relevant project automation context
- **AND** the prompt SHALL NOT require external tracker objects such as Linear issue state or comments

#### Scenario: Retry attempt receives continuation instructions
- **WHEN** the scheduler starts a retry or continuation attempt for the same task
- **THEN** the rendered prompt includes the attempt number
- **AND** the prompt instructs the agent to continue from current workspace state instead of restarting completed investigation unnecessarily

#### Scenario: Unattended prompt forbids casual human escalation
- **WHEN** an auto-managed task runs under orchestration
- **THEN** the prompt instructs the agent not to ask a human for generic follow-up actions
- **AND** the prompt limits early exit to true blockers or explicit `vk` review handoff conditions

### Requirement: Human-first ownership visibility
The system SHALL make manual, auto-managed, and waiting-for-human-review states visually distinct in human-facing task surfaces.

#### Scenario: Task list distinguishes ownership from runtime state
- **WHEN** a user views a task list or board
- **THEN** each task SHALL show an ownership indicator separate from its runtime / dispatch indicator
- **AND** the distinction SHALL NOT rely on color alone

#### Scenario: Project task surfaces provide orchestration lanes
- **WHEN** a project has a mix of manual and auto-managed tasks
- **THEN** the UI SHALL provide a clear way to filter or group tasks by `manual`, `managed`, `needs review`, and `blocked/deferred`
- **AND** the project-level execution mode SHALL remain visible near those groupings

### Requirement: Human review handoff for auto-managed runs
Auto-managed task results SHALL be consumable by humans through a task-centric handoff surface.

#### Scenario: Auto-managed run enters review with one summary surface
- **WHEN** an auto-managed task reaches a review/handoff state
- **THEN** task detail SHALL present a single handoff summary composed from the latest attempt summary, diff summary, and available validation or failure details
- **AND** the user SHALL NOT need to inspect raw logs just to understand the outcome at a high level

#### Scenario: Human can choose follow-up action from handoff state
- **WHEN** a user reviews an auto-managed outcome
- **THEN** the UI SHALL support clear next actions such as approving the result, requesting rework, or taking the task over manually
- **AND** those actions SHALL map back onto the existing task/runtime model without requiring a separate orchestration-only workflow

### Requirement: Related follow-up tasks are attributable
Agent- or MCP-created follow-up tasks SHALL remain understandable to human operators.

#### Scenario: Related task shows its origin
- **WHEN** a task is created as a follow-up to another task
- **THEN** the system SHALL preserve a reference to the originating task or an equivalent task relation
- **AND** the UI SHALL expose that relationship in task detail or list surfaces

#### Scenario: Project policy constrains follow-up task automation
- **WHEN** an agent or MCP caller creates a related task and requests automation
- **THEN** project policy SHALL determine whether that request stays manual, inherits project behavior, or may run as explicit auto-managed work
- **AND** the effective result SHALL be inspectable by the operator

### Requirement: Human and agent control transfer remains explicit
The system SHALL make control transfer between human-driven and auto-managed execution legible to both interactive and programmatic clients.

#### Scenario: Human pauses a managed task
- **WHEN** a human operator pauses or takes over an auto-managed task
- **THEN** the task SHALL expose a persisted state that explains automation is paused due to explicit human control
- **AND** programmatic reads SHALL reflect that state without requiring raw-log inspection

#### Scenario: Managed task resumes after human intervention
- **WHEN** a task returns from human-managed control back into automation
- **THEN** the task SHALL expose the resulting effective automation mode and scheduling eligibility
- **AND** the scheduler SHALL use the resumed state without requiring an out-of-band orchestration API

### Requirement: MCP callers can consume auto-managed handoff state
Programmatic clients SHALL be able to understand the result of an auto-managed run through concise handoff surfaces.

#### Scenario: MCP caller reads review-ready task outcome
- **WHEN** an auto-managed task reaches a review-required handoff state
- **THEN** the system SHALL expose a concise summary that includes the latest run summary, validation outcome, and diff summary
- **AND** an MCP caller SHALL NOT need to scrape raw execution logs just to decide whether to approve, rework, or pause automation

### Requirement: Executor/profile selection remains policy-bound
The system SHALL keep project owners in control of which executor/profile variants may be used for auto-managed work.

#### Scenario: MCP caller requests an allowed executor/profile
- **WHEN** a programmatic caller requests an executor/profile variant that project policy allows
- **THEN** the resulting attempt SHALL use that effective executor/profile
- **AND** attempt/session surfaces SHALL make that effective selection inspectable

#### Scenario: MCP caller requests a disallowed executor/profile
- **WHEN** a programmatic caller requests an executor/profile variant that project policy does not allow
- **THEN** the task SHALL remain persisted without silently escalating execution rights
- **AND** the system SHALL expose a structured diagnostic explaining why the requested profile was not eligible

### Requirement: Auto-managed starts honor required workspace preparation hooks
Auto-managed task dispatch SHALL honor required workspace preparation hooks before starting coding-agent execution.

#### Scenario: Scheduler defers dispatch on required hook failure
- **WHEN** an auto-managed task belongs to a project whose blocking `after_prepare` hook fails
- **THEN** the scheduler SHALL NOT continue into coding-agent execution
- **AND** the task SHALL expose a structured non-dispatch reason for the hook failure

