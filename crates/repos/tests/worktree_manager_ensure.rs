use chrono::Utc;
use db::models::repo::Repo;
use repos::{
    git::GitService,
    workspace_manager::{RepoWorkspaceInput, WorkspaceManager},
    worktree_manager::WorktreeManager,
};
use tempfile::TempDir;
use uuid::Uuid;

mod git_test_utils;
use git_test_utils::{git_rev_parse, git_try};

#[tokio::test]
async fn create_worktree_succeeds_when_git_worktrees_dir_is_missing() {
    let td = TempDir::new().expect("tempdir");

    let repo_path = td.path().join("repo");
    GitService::new()
        .initialize_repo_with_main_branch(&repo_path)
        .expect("init repo");

    // No worktrees exist yet, so `.git/worktrees` should be absent.
    assert!(
        !repo_path.join(".git").join("worktrees").exists(),
        "expected .git/worktrees to be absent before first worktree add"
    );

    // Pre-create a stale directory to force the ensure path to check git metadata.
    let worktree_path = td.path().join("workspace").join("repo");
    std::fs::create_dir_all(&worktree_path).expect("create stale worktree dir");

    WorktreeManager::create_worktree(
        &repo_path,
        "vk/test-metadata-missing",
        &worktree_path,
        "main",
        true,
    )
    .await
    .expect("create worktree should succeed");

    assert!(
        worktree_path.join(".git").is_file(),
        "expected worktree to have .git file marker"
    );
    assert!(
        repo_path.join(".git").join("worktrees").exists(),
        "expected .git/worktrees to exist after worktree creation"
    );
}

#[tokio::test]
async fn ensure_workspace_creates_missing_attempt_branch_from_target_branch() {
    let td = TempDir::new().expect("tempdir");

    let repo_path = td.path().join("repo");
    GitService::new()
        .initialize_repo_with_main_branch(&repo_path)
        .expect("init repo");

    let main_oid = git_rev_parse(&repo_path, "refs/heads/main");

    let attempt_branch = "vk/test-missing-branch";
    let attempt_ref = format!("refs/heads/{attempt_branch}");
    assert!(
        git_try(
            &repo_path,
            vec![
                "rev-parse".to_string(),
                "--verify".to_string(),
                attempt_ref.clone(),
            ],
        )
        .is_err(),
        "attempt branch should not exist before ensure"
    );

    let repo_model = Repo {
        id: Uuid::new_v4(),
        path: repo_path.clone(),
        name: "repo".to_string(),
        display_name: "repo".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let inputs = vec![RepoWorkspaceInput::new(repo_model, "main".to_string())];

    let workspace_dir = td.path().join("workspace");
    WorkspaceManager::ensure_workspace_exists(&workspace_dir, &inputs, attempt_branch)
        .await
        .expect("ensure workspace should succeed");

    let attempt_oid = git_rev_parse(&repo_path, &attempt_ref);
    assert_eq!(
        attempt_oid, main_oid,
        "attempt branch should be created from target branch commit"
    );

    let worktree_path = workspace_dir.join("repo");
    assert!(
        worktree_path.exists(),
        "expected worktree to exist at workspace_dir/<repo_name>"
    );
    assert!(
        worktree_path.join(".git").is_file(),
        "expected created worktree to have .git file marker"
    );
}

#[tokio::test]
async fn ensure_worktree_is_idempotent_when_already_set_up() {
    let td = TempDir::new().expect("tempdir");

    let repo_path = td.path().join("repo");
    GitService::new()
        .initialize_repo_with_main_branch(&repo_path)
        .expect("init repo");

    let branch = "vk/test-idempotent";
    let worktree_path = td.path().join("workspace").join("repo");

    WorktreeManager::create_worktree(&repo_path, branch, &worktree_path, "main", true)
        .await
        .expect("create worktree");

    WorktreeManager::ensure_worktree_exists(&repo_path, branch, &worktree_path)
        .await
        .expect("ensure should be idempotent");

    assert!(
        worktree_path.join(".git").is_file(),
        "expected worktree to remain valid"
    );
}
