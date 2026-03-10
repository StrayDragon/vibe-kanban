use std::{collections::HashMap, path::Path, time::Duration};

use anyhow::{Context, Result};
use app_runtime::Deployment;
use chrono::{Duration as ChronoDuration, Utc};
use db::models::{
    milestone::Milestone,
    project::Project,
    task::{TaskStatus, TaskWithAttemptStatus},
    task_dispatch_state::{TaskDispatchState, UpsertTaskDispatchState},
    workspace_repo::CreateWorkspaceRepo,
};
use repos::git::GitService;
use tasks::orchestration::{self, CreateTaskAttemptInput};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    DeploymentImpl,
    auto_orchestrator_prompt::{PromptRepoContext, render_auto_orchestration_prompt},
    milestone_dispatch::{
        milestone_dispatch_enabled, milestone_has_active_attempt, next_milestone_dispatch_candidate,
    },
    task_runtime::DeploymentTaskRuntime,
};

const POLL_INTERVAL: Duration = Duration::from_secs(10);
const CLAIM_TTL: Duration = Duration::from_secs(120);
const MAX_BACKOFF_SECONDS: i64 = 60 * 10;

#[derive(Debug, Clone)]
struct ResolvedWorkspaceRepo {
    create: CreateWorkspaceRepo,
    display_name: String,
}

pub fn spawn(deployment: DeploymentImpl) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let shutdown = deployment.shutdown_token();
        if let Err(err) = run_loop(deployment, shutdown).await {
            warn!(error = %err, "auto orchestrator stopped with error");
        }
    })
}

async fn run_loop(deployment: DeploymentImpl, shutdown: CancellationToken) -> Result<()> {
    let mut interval = tokio::time::interval(POLL_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                info!("auto orchestrator shutting down");
                return Ok(());
            }
            _ = interval.tick() => {
                if let Err(err) = poll_once(&deployment).await {
                    warn!(error = %err, "auto orchestrator poll failed");
                }
            }
        }
    }
}

async fn poll_once(deployment: &DeploymentImpl) -> Result<()> {
    let projects = Project::find_all(&deployment.db().pool)
        .await
        .context("failed to load projects for auto orchestration")?;

    for project in projects {
        reconcile_project(deployment, &project)
            .await
            .with_context(|| format!("failed to reconcile project {}", project.id))?;
    }

    Ok(())
}

async fn reconcile_project(deployment: &DeploymentImpl, project: &Project) -> Result<()> {
    let mut tasks = db::models::task::Task::find_by_project_id_with_attempt_status(
        &deployment.db().pool,
        project.id,
    )
    .await?;
    tasks.sort_by_key(|task| task.created_at);

    let active_runs = tasks
        .iter()
        .filter(|task| task.has_in_progress_attempt)
        .count() as i32;
    let mut available_slots = (project.scheduler_max_concurrent - active_runs).max(0);

    for task in &tasks {
        reconcile_task_state(deployment, project, task).await?;
    }

    if available_slots <= 0 {
        return Ok(());
    }

    let tasks_by_id: HashMap<Uuid, TaskWithAttemptStatus> =
        tasks.into_iter().map(|task| (task.id, task)).collect();

    let mut milestones = Milestone::find_by_project_id(&deployment.db().pool, project.id)
        .await
        .context("failed to load milestones for milestone orchestration")?;
    milestones.retain(milestone_dispatch_enabled);

    if milestones.is_empty() {
        return Ok(());
    }

    // Prioritize explicit "run next step" requests first, oldest-first.
    milestones.sort_by_key(|milestone| {
        (
            milestone.run_next_step_requested_at.is_none(),
            milestone
                .run_next_step_requested_at
                .unwrap_or(milestone.created_at),
            milestone.created_at,
        )
    });

    for milestone in milestones {
        if available_slots <= 0 {
            break;
        }

        if milestone_has_active_attempt(&milestone, &tasks_by_id) {
            continue;
        }

        let Some(task) = next_milestone_dispatch_candidate(&milestone, &tasks_by_id) else {
            if milestone.run_next_step_requested_at.is_some() {
                warn!(
                    milestone_id = %milestone.id,
                    project_id = %project.id,
                    "run next step requested but no eligible node; clearing request"
                );
                if let Err(err) =
                    Milestone::clear_run_next_step_request(&deployment.db().pool, milestone.id)
                        .await
                {
                    warn!(
                        milestone_id = %milestone.id,
                        project_id = %project.id,
                        error = %err,
                        "failed to clear run next step request"
                    );
                }
            }
            continue;
        };

        match dispatch_task(deployment, project, task).await {
            Ok(DispatchOutcome::Started) => {
                available_slots -= 1;
                if milestone.run_next_step_requested_at.is_some()
                    && let Err(err) =
                        Milestone::clear_run_next_step_request(&deployment.db().pool, milestone.id)
                            .await
                {
                    warn!(
                        milestone_id = %milestone.id,
                        project_id = %project.id,
                        error = %err,
                        "failed to clear run next step request after dispatch"
                    );
                }
            }
            Ok(DispatchOutcome::Blocked) => {
                if milestone.run_next_step_requested_at.is_some()
                    && let Err(err) =
                        Milestone::clear_run_next_step_request(&deployment.db().pool, milestone.id)
                            .await
                {
                    warn!(
                        milestone_id = %milestone.id,
                        project_id = %project.id,
                        error = %err,
                        "failed to clear run next step request after block"
                    );
                }
            }
            Ok(DispatchOutcome::RetryScheduled | DispatchOutcome::Skipped) => {}
            Err(err) => {
                warn!(
                    task_id = %task.id,
                    milestone_id = %milestone.id,
                    project_id = %project.id,
                    error = %err,
                    "milestone dispatch failed"
                );
            }
        }
    }

    Ok(())
}

async fn reconcile_task_state(
    deployment: &DeploymentImpl,
    project: &Project,
    task: &TaskWithAttemptStatus,
) -> Result<()> {
    if task.dispatch_state.is_none() && !task.has_in_progress_attempt {
        return Ok(());
    }

    let Some(existing) = task.dispatch_state.clone() else {
        return Ok(());
    };

    if matches!(task.status, TaskStatus::Done | TaskStatus::Cancelled) {
        if existing.status != db::types::TaskDispatchStatus::Idle {
            TaskDispatchState::upsert(
                &deployment.db().pool,
                task.id,
                &UpsertTaskDispatchState {
                    controller: existing.controller,
                    status: db::types::TaskDispatchStatus::Idle,
                    retry_count: existing.retry_count,
                    max_retries: project.scheduler_max_retries,
                    last_error: None,
                    blocked_reason: None,
                    next_retry_at: None,
                    claim_expires_at: None,
                },
            )
            .await?;
        }
        return Ok(());
    }

    if task.has_in_progress_attempt {
        if existing.status != db::types::TaskDispatchStatus::Running {
            TaskDispatchState::upsert(
                &deployment.db().pool,
                task.id,
                &UpsertTaskDispatchState {
                    controller: existing.controller,
                    status: db::types::TaskDispatchStatus::Running,
                    retry_count: existing.retry_count,
                    max_retries: project.scheduler_max_retries,
                    last_error: existing.last_error,
                    blocked_reason: None,
                    next_retry_at: None,
                    claim_expires_at: None,
                },
            )
            .await?;
        }
        return Ok(());
    }

    if task.status == TaskStatus::InReview && !task.last_attempt_failed {
        if existing.status != db::types::TaskDispatchStatus::AwaitingHumanReview {
            TaskDispatchState::upsert(
                &deployment.db().pool,
                task.id,
                &UpsertTaskDispatchState {
                    controller: existing.controller,
                    status: db::types::TaskDispatchStatus::AwaitingHumanReview,
                    retry_count: existing.retry_count,
                    max_retries: project.scheduler_max_retries,
                    last_error: None,
                    blocked_reason: None,
                    next_retry_at: None,
                    claim_expires_at: None,
                },
            )
            .await?;
        }
        return Ok(());
    }

    if task.status == TaskStatus::InReview && task.last_attempt_failed {
        let next_retry_count = (existing.retry_count + 1).max(1);
        if next_retry_count > project.scheduler_max_retries {
            if existing.status != db::types::TaskDispatchStatus::Blocked
                || existing.max_retries != project.scheduler_max_retries
            {
                TaskDispatchState::upsert(
                    &deployment.db().pool,
                    task.id,
                    &UpsertTaskDispatchState {
                        controller: existing.controller,
                        status: db::types::TaskDispatchStatus::Blocked,
                        retry_count: existing.retry_count,
                        max_retries: project.scheduler_max_retries,
                        last_error: existing.last_error,
                        blocked_reason: Some("Retry limit reached".to_string()),
                        next_retry_at: None,
                        claim_expires_at: None,
                    },
                )
                .await?;
            }
            return Ok(());
        }

        let next_retry_at = Utc::now() + retry_backoff(next_retry_count);
        if existing.status != db::types::TaskDispatchStatus::RetryScheduled {
            TaskDispatchState::upsert(
                &deployment.db().pool,
                task.id,
                &UpsertTaskDispatchState {
                    controller: existing.controller,
                    status: db::types::TaskDispatchStatus::RetryScheduled,
                    retry_count: next_retry_count,
                    max_retries: project.scheduler_max_retries,
                    last_error: Some("Last attempt failed".to_string()),
                    blocked_reason: None,
                    next_retry_at: Some(next_retry_at),
                    claim_expires_at: None,
                },
            )
            .await?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DispatchOutcome {
    Started,
    RetryScheduled,
    Blocked,
    Skipped,
}

async fn upsert_blocked_state(
    deployment: &DeploymentImpl,
    project: &Project,
    task: &TaskWithAttemptStatus,
    retry_count: i32,
    reason: impl Into<String>,
) -> Result<()> {
    let reason = reason.into();
    TaskDispatchState::upsert(
        &deployment.db().pool,
        task.id,
        &UpsertTaskDispatchState {
            controller: db::types::TaskDispatchController::Scheduler,
            status: db::types::TaskDispatchStatus::Blocked,
            retry_count,
            max_retries: project.scheduler_max_retries,
            last_error: Some(reason.clone()),
            blocked_reason: Some(reason),
            next_retry_at: None,
            claim_expires_at: None,
        },
    )
    .await?;

    Ok(())
}

async fn dispatch_task(
    deployment: &DeploymentImpl,
    project: &Project,
    task: &TaskWithAttemptStatus,
) -> Result<DispatchOutcome> {
    let executor_profile_id = {
        let config = deployment.config().read().await;
        config.executor_profile.clone()
    };

    let current_retry_count = task
        .dispatch_state
        .as_ref()
        .map(|state| state.retry_count)
        .unwrap_or(0);

    let resolved_repos = match resolve_workspace_repos(deployment, project).await {
        Ok(repos) => repos,
        Err(err) => {
            upsert_blocked_state(
                deployment,
                project,
                task,
                current_retry_count,
                err.to_string(),
            )
            .await?;
            return Ok(DispatchOutcome::Blocked);
        }
    };

    if resolved_repos.is_empty() {
        upsert_blocked_state(
            deployment,
            project,
            task,
            current_retry_count,
            "Project has no repositories configured",
        )
        .await?;
        return Ok(DispatchOutcome::Blocked);
    }

    let prompt_repos: Vec<PromptRepoContext> = resolved_repos
        .iter()
        .map(|repo| PromptRepoContext {
            display_name: repo.display_name.clone(),
            target_branch: repo.create.target_branch.clone(),
        })
        .collect();
    let repos: Vec<CreateWorkspaceRepo> =
        resolved_repos.into_iter().map(|repo| repo.create).collect();
    let prompt = render_auto_orchestration_prompt(
        &task.task,
        project,
        &prompt_repos,
        (current_retry_count > 0).then_some(current_retry_count + 1),
    );

    TaskDispatchState::upsert(
        &deployment.db().pool,
        task.id,
        &UpsertTaskDispatchState {
            controller: db::types::TaskDispatchController::Scheduler,
            status: db::types::TaskDispatchStatus::Claimed,
            retry_count: current_retry_count,
            max_retries: project.scheduler_max_retries,
            last_error: None,
            blocked_reason: None,
            next_retry_at: None,
            claim_expires_at: Some(Utc::now() + ChronoDuration::from_std(CLAIM_TTL)?),
        },
    )
    .await?;

    let runtime = DeploymentTaskRuntime::new(deployment.container());
    match orchestration::create_task_attempt(
        &runtime,
        &deployment.db().pool,
        &CreateTaskAttemptInput {
            task_id: task.id,
            executor_profile_id,
            repos,
            prompt_override: Some(prompt),
        },
    )
    .await
    {
        Ok(_) => {
            TaskDispatchState::upsert(
                &deployment.db().pool,
                task.id,
                &UpsertTaskDispatchState {
                    controller: db::types::TaskDispatchController::Scheduler,
                    status: db::types::TaskDispatchStatus::Running,
                    retry_count: current_retry_count,
                    max_retries: project.scheduler_max_retries,
                    last_error: None,
                    blocked_reason: None,
                    next_retry_at: None,
                    claim_expires_at: None,
                },
            )
            .await?;
            info!(
                task_id = %task.id,
                project_id = %project.id,
                "scheduler dispatched task"
            );
            Ok(DispatchOutcome::Started)
        }
        Err(err) => {
            let err_message = err.to_string();
            let next_retry_count = current_retry_count + 1;
            let (status, retry_count, blocked_reason, next_retry_at) =
                if is_blocking_workspace_hook_error(&err_message) {
                    (
                        db::types::TaskDispatchStatus::Blocked,
                        current_retry_count,
                        Some(err_message.clone()),
                        None,
                    )
                } else if next_retry_count > project.scheduler_max_retries {
                    (
                        db::types::TaskDispatchStatus::Blocked,
                        next_retry_count,
                        Some("Retry limit reached while starting attempt".to_string()),
                        None,
                    )
                } else {
                    (
                        db::types::TaskDispatchStatus::RetryScheduled,
                        next_retry_count,
                        None,
                        Some(Utc::now() + retry_backoff(next_retry_count)),
                    )
                };

            TaskDispatchState::upsert(
                &deployment.db().pool,
                task.id,
                &UpsertTaskDispatchState {
                    controller: db::types::TaskDispatchController::Scheduler,
                    status: status.clone(),
                    retry_count,
                    max_retries: project.scheduler_max_retries,
                    last_error: Some(err_message),
                    blocked_reason,
                    next_retry_at,
                    claim_expires_at: None,
                },
            )
            .await?;

            Ok(match status {
                db::types::TaskDispatchStatus::RetryScheduled => DispatchOutcome::RetryScheduled,
                db::types::TaskDispatchStatus::Blocked => DispatchOutcome::Blocked,
                _ => DispatchOutcome::Skipped,
            })
        }
    }
}

async fn resolve_workspace_repos(
    deployment: &DeploymentImpl,
    project: &Project,
) -> Result<Vec<ResolvedWorkspaceRepo>> {
    let repos = deployment
        .project()
        .get_repositories(&deployment.db().pool, project.id)
        .await?;
    let preferred_branch = {
        let config = deployment.config().read().await;
        config
            .github
            .default_pr_base
            .clone()
            .unwrap_or_else(|| "main".to_string())
    };

    let mut workspace_repos = Vec::with_capacity(repos.len());
    for repo in repos {
        let target_branch = resolve_target_branch(deployment.git(), &repo.path, &preferred_branch)
            .with_context(|| {
                format!(
                    "Base branch unresolved for repo {} ({}); tried preferred, main, master",
                    repo.display_name, repo.id
                )
            })?;
        workspace_repos.push(ResolvedWorkspaceRepo {
            create: CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch,
            },
            display_name: repo.display_name,
        });
    }

    Ok(workspace_repos)
}

fn resolve_target_branch(git: &GitService, repo_path: &Path, preferred: &str) -> Result<String> {
    let mut candidates = vec![preferred.trim().to_string()];
    for fallback in ["main", "master"] {
        if !candidates.iter().any(|candidate| candidate == fallback) {
            candidates.push(fallback.to_string());
        }
    }

    for candidate in candidates {
        if candidate.is_empty() {
            continue;
        }
        if git
            .check_branch_exists(repo_path, &candidate)
            .unwrap_or(false)
            || git
                .check_remote_branch_exists(repo_path, &candidate)
                .unwrap_or(false)
        {
            return Ok(candidate);
        }
    }

    Err(anyhow::anyhow!(
        "No suitable base branch found; tried preferred, main, master"
    ))
}

fn is_blocking_workspace_hook_error(message: &str) -> bool {
    message
        .to_ascii_lowercase()
        .contains("workspace lifecycle hook failed during after_prepare")
}

fn retry_backoff(retry_count: i32) -> ChronoDuration {
    let exponent = retry_count.saturating_sub(1).min(6) as u32;
    let seconds = (15_i64 * 2_i64.pow(exponent)).min(MAX_BACKOFF_SECONDS);
    ChronoDuration::seconds(seconds)
}

#[cfg(test)]
mod tests {
    use super::{MAX_BACKOFF_SECONDS, retry_backoff};

    #[test]
    fn retry_backoff_is_capped() {
        assert_eq!(retry_backoff(1).num_seconds(), 15);
        assert_eq!(retry_backoff(2).num_seconds(), 30);
        assert_eq!(retry_backoff(7).num_seconds(), MAX_BACKOFF_SECONDS);
        assert_eq!(retry_backoff(9).num_seconds(), MAX_BACKOFF_SECONDS);
    }
}
