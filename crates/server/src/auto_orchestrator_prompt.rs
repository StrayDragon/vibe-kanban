use db::models::{project::Project, task::Task};

const DEFAULT_AUTO_ORCHESTRATION_PROMPT: &str =
    include_str!("../../../docs/auto-orchestration-prompt.md");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptRepoContext {
    pub display_name: String,
    pub target_branch: String,
}

pub fn render_auto_orchestration_prompt(
    task: &Task,
    project: &Project,
    repos: &[PromptRepoContext],
    attempt: Option<i32>,
) -> String {
    let attempt_section = if let Some(attempt) = attempt.filter(|attempt| *attempt > 0) {
        format!(
            "Continuation context:\n- This is retry/continuation attempt #{attempt}.\n- Resume from the current workspace state instead of restarting from scratch.\n- Re-check only the parts affected by your new changes."
        )
    } else {
        "First-run context:\n- This is the first unattended attempt for this task.\n- Start by understanding the task and planning the smallest safe implementation path.".to_string()
    };

    let repository_context = if repos.is_empty() {
        "- No repositories were resolved for this task.".to_string()
    } else {
        repos
            .iter()
            .map(|repo| format!("- {} -> {}", repo.display_name, repo.target_branch))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let description = task
        .description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("No description provided.");

    DEFAULT_AUTO_ORCHESTRATION_PROMPT
        .replace("{task_id}", &task.id.to_string())
        .replace("{task_title}", task.title.trim())
        .replace("{task_status}", &task.status.to_string())
        .replace("{project_name}", project.name.trim())
        .replace("{task_description}", description)
        .replace("{repository_context}", &repository_context)
        .replace("{attempt_section}", &attempt_section)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use db::{
        models::{project::Project, task::Task},
        types::{ProjectMcpExecutorPolicyMode, TaskCreatedByKind, TaskKind, TaskStatus},
    };
    use uuid::Uuid;

    use super::{PromptRepoContext, render_auto_orchestration_prompt};

    fn sample_project() -> Project {
        Project {
            id: Uuid::new_v4(),
            name: "Demo Project".to_string(),
            dev_script: None,
            dev_script_working_dir: None,
            default_agent_working_dir: None,
            git_no_verify_override: None,
            scheduler_max_concurrent: 2,
            scheduler_max_retries: 3,
            default_continuation_turns: 0,
            mcp_auto_executor_policy_mode: ProjectMcpExecutorPolicyMode::InheritAll,
            mcp_auto_executor_policy_allow_list: Vec::new(),
            after_prepare_hook: None,
            before_cleanup_hook: None,
            remote_project_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn sample_task() -> Task {
        Task {
            id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            title: "Fix flaky test".to_string(),
            description: Some("Make the retry path deterministic.".to_string()),
            status: TaskStatus::Todo,
            task_kind: TaskKind::Default,
            milestone_id: None,
            milestone_node_id: None,
            parent_workspace_id: None,
            origin_task_id: None,
            created_by_kind: TaskCreatedByKind::HumanUi,
            continuation_turns_override: None,
            shared_task_id: None,
            archived_kanban_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn render_first_run_prompt_includes_task_context() {
        let prompt = render_auto_orchestration_prompt(
            &sample_task(),
            &sample_project(),
            &[PromptRepoContext {
                display_name: "repo-a".to_string(),
                target_branch: "main".to_string(),
            }],
            None,
        );

        assert!(prompt.contains("Vibe Kanban task"));
        assert!(prompt.contains("Fix flaky test"));
        assert!(prompt.contains("repo-a -> main"));
        assert!(prompt.contains("First-run context"));
    }

    #[test]
    fn render_retry_prompt_includes_attempt_section() {
        let prompt =
            render_auto_orchestration_prompt(&sample_task(), &sample_project(), &[], Some(2));

        assert!(prompt.contains("retry/continuation attempt #2"));
        assert!(prompt.contains("Resume from the current workspace state"));
    }
}
