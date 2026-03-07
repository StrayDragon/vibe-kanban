use chrono::Utc;

use super::{TaskAutomationDiagnostic, TaskKind, TaskStatus, TaskWithAttemptStatus};
use crate::{
    models::project::Project,
    types::{
        ProjectExecutionMode, TaskAutomationMode, TaskAutomationReasonCode, TaskDispatchStatus,
    },
};

pub fn effective_task_automation_mode(
    project_mode: ProjectExecutionMode,
    task_mode: TaskAutomationMode,
) -> ProjectExecutionMode {
    match task_mode {
        TaskAutomationMode::Inherit => project_mode,
        TaskAutomationMode::Manual => ProjectExecutionMode::Manual,
        TaskAutomationMode::Auto => ProjectExecutionMode::Auto,
    }
}

pub fn enrich_task_with_automation_context(task: &mut TaskWithAttemptStatus) {
    task.effective_automation_mode = effective_task_automation_mode(
        task.project_execution_mode.clone(),
        task.automation_mode.clone(),
    );
    task.automation_diagnostic = base_diagnostic(task);
}

pub fn decorate_project_tasks_with_automation_context(
    project: &Project,
    tasks: &mut [TaskWithAttemptStatus],
) {
    for task in tasks.iter_mut() {
        task.project_execution_mode = project.execution_mode.clone();
        enrich_task_with_automation_context(task);
    }

    let active_runs = tasks
        .iter()
        .filter(|task| task.has_in_progress_attempt)
        .count() as i32;
    let mut available_slots = (project.scheduler_max_concurrent - active_runs).max(0);

    let mut indices: Vec<usize> = (0..tasks.len()).collect();
    indices.sort_by_key(|&index| tasks[index].created_at);

    for index in indices {
        let task = &mut tasks[index];
        if !is_dispatch_candidate(task) || !retry_ready(task) {
            continue;
        }

        if available_slots > 0 {
            available_slots -= 1;
            continue;
        }

        task.automation_diagnostic = Some(TaskAutomationDiagnostic {
            reason_code: TaskAutomationReasonCode::ConcurrencyLimitReached,
            reason_detail: format!(
                "Project auto-orchestration is at its concurrency limit (max {} concurrent runs).",
                project.scheduler_max_concurrent
            ),
            actionable: true,
        });
    }
}

pub fn is_dispatch_candidate(task: &TaskWithAttemptStatus) -> bool {
    if task.effective_automation_mode != ProjectExecutionMode::Auto {
        return false;
    }
    if task.has_in_progress_attempt {
        return false;
    }
    if task.task_kind == TaskKind::Group || task.task_group_id.is_some() {
        return false;
    }
    if matches!(task.status, TaskStatus::Done | TaskStatus::Cancelled) {
        return false;
    }
    if let Some(state) = &task.dispatch_state
        && state.status == TaskDispatchStatus::Blocked
    {
        return false;
    }

    match task.status {
        TaskStatus::Todo => true,
        TaskStatus::InReview => task.last_attempt_failed,
        _ => false,
    }
}

pub fn retry_ready(task: &TaskWithAttemptStatus) -> bool {
    if task.status == TaskStatus::Todo {
        return true;
    }

    let Some(state) = task.dispatch_state.as_ref() else {
        return true;
    };

    if state.status != TaskDispatchStatus::RetryScheduled {
        return false;
    }

    state
        .next_retry_at
        .map(|next_retry_at| next_retry_at <= Utc::now())
        .unwrap_or(true)
}

fn blocked_reason_code(task: &TaskWithAttemptStatus) -> TaskAutomationReasonCode {
    let haystack = task
        .dispatch_state
        .as_ref()
        .and_then(|state| {
            state
                .blocked_reason
                .as_deref()
                .or(state.last_error.as_deref())
                .map(str::to_owned)
        })
        .unwrap_or_default()
        .to_ascii_lowercase();

    if haystack.contains("retry limit reached") {
        TaskAutomationReasonCode::RetryExhausted
    } else if haystack.contains("no repositories configured") {
        TaskAutomationReasonCode::NoProjectRepos
    } else if haystack.contains("base branch") || haystack.contains("no suitable base branch") {
        TaskAutomationReasonCode::BaseBranchUnresolved
    } else {
        TaskAutomationReasonCode::Blocked
    }
}

fn base_diagnostic(task: &TaskWithAttemptStatus) -> Option<TaskAutomationDiagnostic> {
    if matches!(task.status, TaskStatus::Done | TaskStatus::Cancelled) {
        return None;
    }

    if task.task_kind == TaskKind::Group || task.task_group_id.is_some() {
        return Some(TaskAutomationDiagnostic {
            reason_code: TaskAutomationReasonCode::TaskGroupUnsupported,
            reason_detail:
                "Task groups and grouped child tasks are not yet supported by auto orchestration."
                    .to_string(),
            actionable: false,
        });
    }

    match task.automation_mode {
        TaskAutomationMode::Manual => {
            return Some(TaskAutomationDiagnostic {
                reason_code: TaskAutomationReasonCode::TaskManualOverride,
                reason_detail:
                    "This task is pinned to manual mode and will not be auto-dispatched."
                        .to_string(),
                actionable: true,
            });
        }
        TaskAutomationMode::Inherit
            if task.project_execution_mode == ProjectExecutionMode::Manual =>
        {
            return Some(TaskAutomationDiagnostic {
                reason_code: TaskAutomationReasonCode::ProjectManual,
                reason_detail:
                    "This task inherits a project-level manual mode and will wait for a human start."
                        .to_string(),
                actionable: true,
            });
        }
        _ => {}
    }

    if let Some(state) = &task.dispatch_state {
        match state.status {
            TaskDispatchStatus::AwaitingHumanReview => {
                return Some(TaskAutomationDiagnostic {
                    reason_code: TaskAutomationReasonCode::AwaitingHumanReview,
                    reason_detail: "Automation is paused while this task awaits human review."
                        .to_string(),
                    actionable: true,
                });
            }
            TaskDispatchStatus::RetryScheduled => {
                let detail = state
                    .next_retry_at
                    .map(|time| format!("Retry is scheduled for {}.", time.to_rfc3339()))
                    .unwrap_or_else(|| {
                        "Retry is queued and waiting for the next scheduler pass.".to_string()
                    });
                return Some(TaskAutomationDiagnostic {
                    reason_code: TaskAutomationReasonCode::RetryNotReady,
                    reason_detail: detail,
                    actionable: false,
                });
            }
            TaskDispatchStatus::Blocked => {
                return Some(TaskAutomationDiagnostic {
                    reason_code: blocked_reason_code(task),
                    reason_detail: state
                        .blocked_reason
                        .clone()
                        .or_else(|| state.last_error.clone())
                        .unwrap_or_else(|| "Auto orchestration blocked this task.".to_string()),
                    actionable: true,
                });
            }
            TaskDispatchStatus::Claimed
            | TaskDispatchStatus::Running
            | TaskDispatchStatus::Idle => {}
        }
    }

    if task.status == TaskStatus::InReview && !task.last_attempt_failed {
        return Some(TaskAutomationDiagnostic {
            reason_code: TaskAutomationReasonCode::AwaitingHumanReview,
            reason_detail: "Automation is paused while this task awaits human review.".to_string(),
            actionable: true,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use uuid::Uuid;

    use super::*;
    use crate::{
        models::{project::Project, task::Task, task_dispatch_state::TaskDispatchState},
        types::{
            ProjectExecutionMode, TaskAutomationMode, TaskAutomationReasonCode, TaskCreatedByKind,
            TaskDispatchController, TaskDispatchStatus,
        },
    };

    fn sample_project() -> Project {
        Project {
            id: Uuid::new_v4(),
            name: "Project".to_string(),
            dev_script: None,
            dev_script_working_dir: None,
            default_agent_working_dir: None,
            git_no_verify_override: None,
            execution_mode: ProjectExecutionMode::Manual,
            scheduler_max_concurrent: 1,
            scheduler_max_retries: 3,
            remote_project_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn sample_task() -> TaskWithAttemptStatus {
        TaskWithAttemptStatus {
            task: Task {
                id: Uuid::new_v4(),
                project_id: Uuid::new_v4(),
                title: "Task".to_string(),
                description: None,
                status: TaskStatus::Todo,
                automation_mode: TaskAutomationMode::Inherit,
                task_kind: TaskKind::Default,
                task_group_id: None,
                task_group_node_id: None,
                parent_workspace_id: None,
                origin_task_id: None,
                created_by_kind: TaskCreatedByKind::HumanUi,
                shared_task_id: None,
                archived_kanban_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            has_in_progress_attempt: false,
            last_attempt_failed: false,
            executor: String::new(),
            project_execution_mode: ProjectExecutionMode::Manual,
            effective_automation_mode: ProjectExecutionMode::Manual,
            dispatch_state: None,
            automation_diagnostic: None,
        }
    }

    #[test]
    fn inherit_respects_manual_project_default() {
        let mut task = sample_task();
        enrich_task_with_automation_context(&mut task);
        assert_eq!(task.effective_automation_mode, ProjectExecutionMode::Manual);
        assert_eq!(
            task.automation_diagnostic.map(|d| d.reason_code),
            Some(TaskAutomationReasonCode::ProjectManual)
        );
    }

    #[test]
    fn task_auto_override_enables_auto_on_manual_project() {
        let mut task = sample_task();
        task.automation_mode = TaskAutomationMode::Auto;
        enrich_task_with_automation_context(&mut task);
        assert_eq!(task.effective_automation_mode, ProjectExecutionMode::Auto);
        assert!(task.automation_diagnostic.is_none());
    }

    #[test]
    fn grouped_tasks_are_marked_unsupported() {
        let mut task = sample_task();
        task.task_group_id = Some(Uuid::new_v4());
        task.project_execution_mode = ProjectExecutionMode::Auto;
        enrich_task_with_automation_context(&mut task);
        assert_eq!(
            task.automation_diagnostic.map(|d| d.reason_code),
            Some(TaskAutomationReasonCode::TaskGroupUnsupported)
        );
    }

    #[test]
    fn retry_scheduled_tasks_expose_retry_reason() {
        let mut task = sample_task();
        task.project_execution_mode = ProjectExecutionMode::Auto;
        task.status = TaskStatus::InReview;
        task.last_attempt_failed = true;
        task.dispatch_state = Some(TaskDispatchState {
            task_id: task.id,
            controller: TaskDispatchController::Scheduler,
            status: TaskDispatchStatus::RetryScheduled,
            retry_count: 1,
            max_retries: 3,
            last_error: None,
            blocked_reason: None,
            next_retry_at: Some(Utc::now()),
            claim_expires_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        });
        enrich_task_with_automation_context(&mut task);
        assert_eq!(
            task.automation_diagnostic.map(|d| d.reason_code),
            Some(TaskAutomationReasonCode::RetryNotReady)
        );
    }

    #[test]
    fn review_waiting_tasks_expose_review_reason() {
        let mut task = sample_task();
        task.project_execution_mode = ProjectExecutionMode::Auto;
        task.status = TaskStatus::InReview;
        task.dispatch_state = Some(TaskDispatchState {
            task_id: task.id,
            controller: TaskDispatchController::Scheduler,
            status: TaskDispatchStatus::AwaitingHumanReview,
            retry_count: 0,
            max_retries: 3,
            last_error: None,
            blocked_reason: None,
            next_retry_at: None,
            claim_expires_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        });
        enrich_task_with_automation_context(&mut task);
        assert_eq!(
            task.automation_diagnostic.map(|d| d.reason_code),
            Some(TaskAutomationReasonCode::AwaitingHumanReview)
        );
    }

    #[test]
    fn concurrency_limit_adds_reason_to_extra_ready_tasks() {
        let mut project = sample_project();
        project.execution_mode = ProjectExecutionMode::Auto;
        project.scheduler_max_concurrent = 1;

        let mut running = sample_task();
        running.project_execution_mode = ProjectExecutionMode::Auto;
        running.has_in_progress_attempt = true;
        running.effective_automation_mode = ProjectExecutionMode::Auto;

        let mut queued = sample_task();
        queued.title = "Queued".to_string();
        queued.project_execution_mode = ProjectExecutionMode::Auto;
        queued.created_at = Utc::now() + chrono::Duration::seconds(1);

        let mut tasks = vec![running, queued];
        decorate_project_tasks_with_automation_context(&project, &mut tasks);

        assert_eq!(
            tasks[1]
                .automation_diagnostic
                .as_ref()
                .map(|d| &d.reason_code),
            Some(&TaskAutomationReasonCode::ConcurrencyLimitReached)
        );
    }
}
