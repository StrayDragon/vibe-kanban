use axum::{
    Router,
    middleware::from_fn_with_state,
    routing::{get, post},
};

use super::{
    abort_conflicts_task_attempt, change_target_branch, create_task_attempt,
    force_push_task_attempt_branch, get_task_attempt, get_task_attempt_branch_status,
    get_task_attempt_changes, get_task_attempt_children, get_task_attempt_repos,
    get_task_attempt_status, get_task_attempts, get_task_attempts_latest_summaries,
    get_task_attempts_with_latest_session, gh_cli_setup_handler, images, merge_task_attempt,
    open_task_attempt_in_editor, pr, push_task_attempt_branch, rebase_task_attempt,
    remove_task_attempt_worktree, rename_branch, run_agent_setup, run_cleanup_script,
    run_setup_script, start_dev_server, stop_task_attempt_execution, ws,
};
use crate::{DeploymentImpl, middleware::load_workspace_middleware};

pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    let task_attempt_id_router = Router::new()
        .route("/", get(get_task_attempt))
        .route("/status", get(get_task_attempt_status))
        .route("/changes", get(get_task_attempt_changes))
        .route("/run-agent-setup", post(run_agent_setup))
        .route("/gh-cli-setup", post(gh_cli_setup_handler))
        .route("/start-dev-server", post(start_dev_server))
        .route("/run-setup-script", post(run_setup_script))
        .route("/run-cleanup-script", post(run_cleanup_script))
        .route("/branch-status", get(get_task_attempt_branch_status))
        .route("/diff/ws", get(ws::stream_task_attempt_diff_ws))
        .route("/merge", post(merge_task_attempt))
        .route("/push", post(push_task_attempt_branch))
        .route("/push/force", post(force_push_task_attempt_branch))
        .route("/rebase", post(rebase_task_attempt))
        .route("/conflicts/abort", post(abort_conflicts_task_attempt))
        .route("/pr", post(pr::create_github_pr))
        .route("/pr/attach", post(pr::attach_existing_pr))
        .route("/pr/comments", get(pr::get_pr_comments))
        .route("/open-editor", post(open_task_attempt_in_editor))
        .route("/children", get(get_task_attempt_children))
        .route("/stop", post(stop_task_attempt_execution))
        .route("/remove-worktree", post(remove_task_attempt_worktree))
        .route("/change-target-branch", post(change_target_branch))
        .route("/rename-branch", post(rename_branch))
        .route("/repos", get(get_task_attempt_repos))
        .layer(from_fn_with_state(
            deployment.clone(),
            load_workspace_middleware::<DeploymentImpl>,
        ));

    let task_attempts_router = Router::new()
        .route("/", get(get_task_attempts).post(create_task_attempt))
        .route(
            "/with-latest-session",
            get(get_task_attempts_with_latest_session),
        )
        .route(
            "/latest-summaries",
            post(get_task_attempts_latest_summaries),
        )
        .nest("/{id}", task_attempt_id_router)
        .nest("/{id}/images", images::router(deployment));

    Router::new().nest("/task-attempts", task_attempts_router)
}
