use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
};

use db::{
    DbErr,
    models::{
        project::{
            CreateProject, Project, ProjectError, ProjectFileSearchResponse, SearchMatchType,
            SearchResult, UpdateProject, WorkspaceLifecycleHookConfig,
        },
        project_repo::{CreateProjectRepo, ProjectRepo},
        repo::Repo,
    },
};
use ignore::WalkBuilder;
use thiserror::Error;
use uuid::Uuid;

use super::{
    file_ranker::FileRanker,
    file_search_cache::{CacheError, FileSearchCache, RepoSearchResponse, SearchMode, SearchQuery},
    repo::{RepoError, RepoService},
};

#[derive(Debug, Error)]
pub enum ProjectServiceError {
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error("Path does not exist: {0}")]
    PathNotFound(PathBuf),
    #[error("Path is not a directory: {0}")]
    PathNotDirectory(PathBuf),
    #[error("Path is not a git repository: {0}")]
    NotGitRepository(PathBuf),
    #[error("Duplicate git repository path")]
    DuplicateGitRepoPath,
    #[error("Duplicate repository name in project")]
    DuplicateRepositoryName,
    #[error("Repository not found")]
    RepositoryNotFound,
    #[error("Git operation failed: {0}")]
    GitError(String),
    #[error("Invalid dev script: {0}")]
    InvalidDevScript(String),
    #[error("Invalid dev script working directory: {0}")]
    InvalidDevScriptWorkingDir(String),
    #[error("Invalid scheduler setting: {0}")]
    InvalidSchedulerSetting(String),
    #[error("Invalid workspace lifecycle hook: {0}")]
    InvalidWorkspaceLifecycleHook(String),
}

pub type Result<T> = std::result::Result<T, ProjectServiceError>;

impl From<RepoError> for ProjectServiceError {
    fn from(e: RepoError) -> Self {
        match e {
            RepoError::PathNotFound(p) => Self::PathNotFound(p),
            RepoError::PathNotDirectory(p) => Self::PathNotDirectory(p),
            RepoError::NotGitRepository(p) => Self::NotGitRepository(p),
            RepoError::Io(e) => Self::Io(e),
            RepoError::Database(e) => Self::Database(e),
            _ => Self::RepositoryNotFound,
        }
    }
}

#[derive(Clone, Default)]
pub struct ProjectService;

impl ProjectService {
    pub fn new() -> Self {
        Self
    }

    fn validate_single_command_text(label: &str, script: &str) -> Result<()> {
        let trimmed = script.trim();
        if trimmed.is_empty() {
            return Err(ProjectServiceError::InvalidWorkspaceLifecycleHook(format!(
                "{label} command cannot be empty"
            )));
        }

        let tokens: Vec<String> = shlex::split(trimmed).ok_or_else(|| {
            ProjectServiceError::InvalidWorkspaceLifecycleHook(format!(
                "{label} command must be valid shell-like command text"
            ))
        })?;
        if tokens.is_empty() {
            return Err(ProjectServiceError::InvalidWorkspaceLifecycleHook(format!(
                "{label} command must include an executable"
            )));
        }
        let has_forbidden = tokens.iter().any(|token| {
            matches!(
                token.as_str(),
                "|" | "||" | "&" | "&&" | ";" | ">" | ">>" | "<" | "<<"
            )
        });
        if has_forbidden {
            return Err(ProjectServiceError::InvalidWorkspaceLifecycleHook(format!(
                "{label} command must be a single command without shell operators"
            )));
        }
        Ok(())
    }

    fn validate_workspace_relative_dir(label: &str, working_dir: &str) -> Result<()> {
        let trimmed = working_dir.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        let path = Path::new(trimmed);
        if path.is_absolute() {
            return Err(ProjectServiceError::InvalidWorkspaceLifecycleHook(format!(
                "{label} working directory must be relative to the workspace root"
            )));
        }
        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(ProjectServiceError::InvalidWorkspaceLifecycleHook(format!(
                "{label} working directory cannot traverse outside the workspace"
            )));
        }
        Ok(())
    }

    fn validate_workspace_hook(phase: &str, hook: &WorkspaceLifecycleHookConfig) -> Result<()> {
        Self::validate_single_command_text(phase, &hook.command)?;
        if let Some(working_dir) = hook.working_dir.as_deref() {
            Self::validate_workspace_relative_dir(phase, working_dir)?;
        }

        match phase {
            "after_prepare" => {
                match hook.failure_policy {
                    db::types::WorkspaceLifecycleHookFailurePolicy::BlockStart
                    | db::types::WorkspaceLifecycleHookFailurePolicy::WarnOnly => {}
                    db::types::WorkspaceLifecycleHookFailurePolicy::BlockCleanup => {
                        return Err(ProjectServiceError::InvalidWorkspaceLifecycleHook(
                            "after_prepare hooks only support block_start or warn_only".to_string(),
                        ));
                    }
                }
                if hook.run_mode.is_none() {
                    return Err(ProjectServiceError::InvalidWorkspaceLifecycleHook(
                        "after_prepare hooks require a run_mode".to_string(),
                    ));
                }
            }
            "before_cleanup" => {
                match hook.failure_policy {
                    db::types::WorkspaceLifecycleHookFailurePolicy::WarnOnly
                    | db::types::WorkspaceLifecycleHookFailurePolicy::BlockCleanup => {}
                    db::types::WorkspaceLifecycleHookFailurePolicy::BlockStart => {
                        return Err(ProjectServiceError::InvalidWorkspaceLifecycleHook(
                            "before_cleanup hooks only support warn_only or block_cleanup"
                                .to_string(),
                        ));
                    }
                }
                if hook.run_mode.is_some() {
                    return Err(ProjectServiceError::InvalidWorkspaceLifecycleHook(
                        "before_cleanup hooks do not support run_mode".to_string(),
                    ));
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn validate_dev_script_update(payload: &UpdateProject) -> Result<()> {
        if let Some(script) = payload.dev_script.as_deref() {
            let trimmed = script.trim();
            if !trimmed.is_empty() {
                Self::validate_single_command_text("dev_script", trimmed).map_err(
                    |err| match err {
                        ProjectServiceError::InvalidWorkspaceLifecycleHook(message) => {
                            ProjectServiceError::InvalidDevScript(message)
                        }
                        other => other,
                    },
                )?;
            }
        }

        if let Some(working_dir) = payload.dev_script_working_dir.as_deref() {
            Self::validate_workspace_relative_dir("dev_script", working_dir).map_err(|err| {
                match err {
                    ProjectServiceError::InvalidWorkspaceLifecycleHook(message) => {
                        ProjectServiceError::InvalidDevScriptWorkingDir(message)
                    }
                    other => other,
                }
            })?;
        }

        if let Some(Some(hook)) = payload.after_prepare_hook.as_ref() {
            Self::validate_workspace_hook("after_prepare", hook)?;
        }

        if let Some(Some(hook)) = payload.before_cleanup_hook.as_ref() {
            Self::validate_workspace_hook("before_cleanup", hook)?;
        }

        if let Some(max_concurrent) = payload.scheduler_max_concurrent
            && max_concurrent < 1
        {
            return Err(ProjectServiceError::InvalidSchedulerSetting(
                "Scheduler max concurrent must be at least 1".to_string(),
            ));
        }

        if let Some(max_retries) = payload.scheduler_max_retries
            && max_retries < 0
        {
            return Err(ProjectServiceError::InvalidSchedulerSetting(
                "Scheduler max retries must be zero or greater".to_string(),
            ));
        }

        Ok(())
    }

    pub async fn create_project(
        &self,
        pool: &db::DbPool,
        repo_service: &RepoService,
        payload: CreateProject,
    ) -> Result<Project> {
        // Validate all repository paths and check for duplicates within the payload
        let mut seen_names = HashSet::new();
        let mut seen_paths = HashSet::new();
        let mut normalized_repos = Vec::new();

        for repo in &payload.repositories {
            let path = repo_service.normalize_path(&repo.git_repo_path)?;
            repo_service.validate_git_repo_path(&path)?;

            let normalized_path = path.to_string_lossy().to_string();

            if !seen_names.insert(repo.display_name.clone()) {
                return Err(ProjectServiceError::DuplicateRepositoryName);
            }

            if !seen_paths.insert(normalized_path.clone()) {
                return Err(ProjectServiceError::DuplicateGitRepoPath);
            }

            normalized_repos.push(CreateProjectRepo {
                display_name: repo.display_name.clone(),
                git_repo_path: normalized_path,
            });
        }

        let id = Uuid::new_v4();

        let project = Project::create(pool, &payload, id)
            .await
            .map_err(|e| ProjectServiceError::Project(ProjectError::CreateFailed(e.to_string())))?;

        let mut created_repo: Option<Repo> = None;
        for repo in &normalized_repos {
            let repo_entity =
                Repo::find_or_create(pool, Path::new(&repo.git_repo_path), &repo.display_name)
                    .await?;
            ProjectRepo::create(pool, project.id, repo_entity.id).await?;
            if created_repo.is_none() {
                created_repo = Some(repo_entity);
            }
        }

        if normalized_repos.len() == 1
            && let Some(repo) = created_repo
        {
            Project::update(
                pool,
                project.id,
                &UpdateProject {
                    name: None,
                    dev_script: None,
                    dev_script_working_dir: None,
                    default_agent_working_dir: Some(repo.name),
                    git_no_verify_override: None,
                    scheduler_max_concurrent: None,
                    scheduler_max_retries: None,
                    after_prepare_hook: None,
                    before_cleanup_hook: None,
                },
            )
            .await?;
        }

        Ok(project)
    }

    pub async fn update_project(
        &self,
        pool: &db::DbPool,
        existing: &Project,
        payload: UpdateProject,
    ) -> Result<Project> {
        Self::validate_dev_script_update(&payload)?;
        let project = Project::update(pool, existing.id, &payload).await?;

        Ok(project)
    }

    pub async fn add_repository(
        &self,
        pool: &db::DbPool,
        repo_service: &RepoService,
        project_id: Uuid,
        payload: &CreateProjectRepo,
    ) -> Result<Repo> {
        tracing::debug!(
            "Adding repository '{}' to project {} (path: {})",
            payload.display_name,
            project_id,
            payload.git_repo_path
        );

        let path = repo_service.normalize_path(&payload.git_repo_path)?;
        repo_service.validate_git_repo_path(&path)?;

        let repository = ProjectRepo::add_repo_to_project(
            pool,
            project_id,
            &path.to_string_lossy(),
            &payload.display_name,
        )
        .await
        .map_err(|e| match e {
            db::models::project_repo::ProjectRepoError::AlreadyExists => {
                ProjectServiceError::DuplicateGitRepoPath
            }
            db::models::project_repo::ProjectRepoError::Database(e) => {
                ProjectServiceError::Database(e)
            }
            _ => ProjectServiceError::RepositoryNotFound,
        })?;

        // If the project now has multiple repositories and still carries a default
        // working directory, clear it. This avoids stale single-repo defaults
        // under concurrent add-repo requests.
        let repo_count_after = ProjectRepo::find_by_project_id(pool, project_id)
            .await?
            .len();
        let has_default_agent_working_dir = Project::find_by_id(pool, project_id)
            .await?
            .and_then(|project| project.default_agent_working_dir)
            .map(|dir| !dir.trim().is_empty())
            .unwrap_or(false);
        if repo_count_after >= 2 && has_default_agent_working_dir {
            Project::clear_default_agent_working_dir(pool, project_id).await?;
        }

        tracing::info!(
            "Added repository {} to project {} (path: {})",
            repository.id,
            project_id,
            repository.path.display()
        );

        Ok(repository)
    }

    pub async fn delete_repository(
        &self,
        pool: &db::DbPool,
        project_id: Uuid,
        repo_id: Uuid,
    ) -> Result<()> {
        tracing::debug!(
            "Removing repository {} from project {}",
            repo_id,
            project_id
        );

        ProjectRepo::remove_repo_from_project(pool, project_id, repo_id)
            .await
            .map_err(|e| match e {
                db::models::project_repo::ProjectRepoError::NotFound => {
                    ProjectServiceError::RepositoryNotFound
                }
                db::models::project_repo::ProjectRepoError::Database(e) => {
                    ProjectServiceError::Database(e)
                }
                _ => ProjectServiceError::RepositoryNotFound,
            })?;

        if let Err(e) = Repo::delete_orphaned(pool).await {
            tracing::error!("Failed to delete orphaned repos: {}", e);
        }

        tracing::info!("Removed repository {} from project {}", repo_id, project_id);

        Ok(())
    }

    pub async fn delete_project(&self, pool: &db::DbPool, project_id: Uuid) -> Result<u64> {
        let rows_affected = Project::delete(pool, project_id).await?;

        if let Err(e) = Repo::delete_orphaned(pool).await {
            tracing::error!("Failed to delete orphaned repos: {}", e);
        }

        Ok(rows_affected)
    }

    pub async fn get_repositories(&self, pool: &db::DbPool, project_id: Uuid) -> Result<Vec<Repo>> {
        let repos = ProjectRepo::find_repos_for_project(pool, project_id).await?;
        Ok(repos)
    }

    pub async fn search_files(
        &self,
        cache: &FileSearchCache,
        repositories: &[Repo],
        query: &SearchQuery,
    ) -> Result<ProjectFileSearchResponse> {
        let query_str = query.q.trim();
        if query_str.is_empty() || repositories.is_empty() {
            return Ok(ProjectFileSearchResponse {
                results: vec![],
                index_truncated: false,
                truncated_repos: vec![],
            });
        }

        // Search in parallel and prefix paths with repo name
        let search_futures: Vec<_> = repositories
            .iter()
            .map(|repo| {
                let repo_name = repo.name.clone();
                let repo_path = repo.path.clone();
                let query = query.clone();
                async move {
                    let response = self
                        .search_single_repo(cache, &repo_path, &query)
                        .await
                        .unwrap_or_else(|e| {
                            tracing::warn!("Search failed for repo {}: {}", repo_name, e);
                            RepoSearchResponse {
                                results: vec![],
                                index_truncated: false,
                            }
                        });
                    (repo_name, response)
                }
            })
            .collect();

        let repo_results = futures::future::join_all(search_futures).await;

        let mut truncated_repos: Vec<String> = repo_results
            .iter()
            .filter_map(|(repo_name, response)| {
                if response.index_truncated {
                    Some(repo_name.clone())
                } else {
                    None
                }
            })
            .collect();
        truncated_repos.sort();
        truncated_repos.dedup();
        let index_truncated = !truncated_repos.is_empty();

        let mut all_results: Vec<SearchResult> = repo_results
            .into_iter()
            .flat_map(|(repo_name, response)| {
                response.results.into_iter().map(move |r| SearchResult {
                    path: format!("{}/{}", repo_name, r.path),
                    is_file: r.is_file,
                    match_type: r.match_type,
                })
            })
            .collect();

        all_results.sort_by(|a, b| {
            let priority = |m: &SearchMatchType| match m {
                SearchMatchType::FileName => 0,
                SearchMatchType::DirectoryName => 1,
                SearchMatchType::FullPath => 2,
            };
            priority(&a.match_type)
                .cmp(&priority(&b.match_type))
                .then_with(|| a.path.cmp(&b.path))
        });

        all_results.truncate(10);
        Ok(ProjectFileSearchResponse {
            results: all_results,
            index_truncated,
            truncated_repos,
        })
    }

    async fn search_single_repo(
        &self,
        cache: &FileSearchCache,
        repo_path: &Path,
        query: &SearchQuery,
    ) -> Result<RepoSearchResponse> {
        let query_str = query.q.trim();
        if query_str.is_empty() {
            return Ok(RepoSearchResponse {
                results: vec![],
                index_truncated: false,
            });
        }

        // Try cache first
        match cache.search(repo_path, query_str, query.mode.clone()).await {
            Ok(response) => Ok(response),
            Err(CacheError::Miss) | Err(CacheError::BuildError(_)) => {
                // Fall back to filesystem search
                Ok(RepoSearchResponse {
                    results: self
                        .search_files_in_repo(repo_path, query_str, query.mode.clone())
                        .await?,
                    index_truncated: false,
                })
            }
        }
    }

    async fn search_files_in_repo(
        &self,
        repo_path: &Path,
        query: &str,
        mode: SearchMode,
    ) -> Result<Vec<SearchResult>> {
        if !repo_path.exists() {
            return Err(ProjectServiceError::PathNotFound(repo_path.to_path_buf()));
        }

        let mut results = Vec::new();
        let query_lower = query.to_lowercase();

        let walker = match mode {
            SearchMode::Settings => {
                // Settings mode: Include ignored files but exclude performance killers
                WalkBuilder::new(repo_path)
                    .git_ignore(false)
                    .git_global(false)
                    .git_exclude(false)
                    .hidden(false)
                    .filter_entry(|entry| {
                        let name = entry.file_name().to_string_lossy();
                        name != ".git"
                            && name != "node_modules"
                            && name != "target"
                            && name != "dist"
                            && name != "build"
                    })
                    .build()
            }
            SearchMode::TaskForm => WalkBuilder::new(repo_path)
                .git_ignore(true)
                .git_global(true)
                .git_exclude(true)
                .hidden(false)
                .filter_entry(|entry| {
                    let name = entry.file_name().to_string_lossy();
                    name != ".git"
                })
                .build(),
        };

        for result in walker {
            let entry = result.map_err(std::io::Error::other)?;
            let path = entry.path();

            // Skip the root directory itself
            if path == repo_path {
                continue;
            }

            let relative_path = path
                .strip_prefix(repo_path)
                .map_err(std::io::Error::other)?;
            let relative_path_str = relative_path.to_string_lossy().to_lowercase();

            let file_name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_lowercase())
                .unwrap_or_default();

            if file_name.contains(&query_lower) {
                results.push(SearchResult {
                    path: relative_path.to_string_lossy().to_string(),
                    is_file: path.is_file(),
                    match_type: SearchMatchType::FileName,
                });
            } else if relative_path_str.contains(&query_lower) {
                let match_type = if path
                    .parent()
                    .and_then(|p| p.file_name())
                    .map(|name| name.to_string_lossy().to_lowercase())
                    .unwrap_or_default()
                    .contains(&query_lower)
                {
                    SearchMatchType::DirectoryName
                } else {
                    SearchMatchType::FullPath
                };

                results.push(SearchResult {
                    path: relative_path.to_string_lossy().to_string(),
                    is_file: path.is_file(),
                    match_type,
                });
            }
        }

        // Apply git history-based ranking
        let file_ranker = FileRanker::new();
        match file_ranker.get_stats(repo_path).await {
            Ok(stats) => {
                file_ranker.rerank(&mut results, &stats);
            }
            Err(_) => {
                // Fallback to basic priority sorting
                results.sort_by(|a, b| {
                    let priority = |match_type: &SearchMatchType| match match_type {
                        SearchMatchType::FileName => 0,
                        SearchMatchType::DirectoryName => 1,
                        SearchMatchType::FullPath => 2,
                    };

                    priority(&a.match_type)
                        .cmp(&priority(&b.match_type))
                        .then_with(|| a.path.cmp(&b.path))
                });
            }
        }

        results.truncate(10);
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use db::{
        DBService,
        models::{
            project::{CreateProject, Project, UpdateProject, WorkspaceLifecycleHookConfig},
            project_repo::{CreateProjectRepo, ProjectRepo},
        },
        types::{WorkspaceLifecycleHookFailurePolicy, WorkspaceLifecycleHookRunMode},
    };
    use sea_orm::Database;
    use sea_orm_migration::MigratorTrait;
    use tempfile::tempdir;
    use uuid::Uuid;

    use super::{ProjectService, ProjectServiceError, RepoService};
    use crate::git::GitService;

    async fn setup_db() -> DBService {
        let pool = Database::connect("sqlite::memory:").await.unwrap();
        db_migration::Migrator::up(&pool, None).await.unwrap();
        DBService { pool }
    }

    fn empty_update_payload() -> UpdateProject {
        UpdateProject {
            name: None,
            dev_script: None,
            dev_script_working_dir: None,
            default_agent_working_dir: None,
            git_no_verify_override: None,
            scheduler_max_concurrent: None,
            scheduler_max_retries: None,
            after_prepare_hook: None,
            before_cleanup_hook: None,
        }
    }

    #[test]
    fn validate_dev_script_update_accepts_single_command() {
        let mut payload = empty_update_payload();
        payload.dev_script = Some("npm run dev -- --host 127.0.0.1".to_string());
        payload.dev_script_working_dir = Some("repo-a".to_string());

        let result = ProjectService::validate_dev_script_update(&payload);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_dev_script_update_rejects_shell_operators() {
        let mut payload = empty_update_payload();
        payload.dev_script = Some("npm run dev && rm -rf /".to_string());

        let result = ProjectService::validate_dev_script_update(&payload);
        assert!(matches!(
            result,
            Err(ProjectServiceError::InvalidDevScript(_))
        ));
    }

    #[test]
    fn validate_dev_script_update_rejects_absolute_working_dir() {
        let mut payload = empty_update_payload();
        payload.dev_script = Some("npm run dev".to_string());
        payload.dev_script_working_dir = Some("/tmp".to_string());

        let result = ProjectService::validate_dev_script_update(&payload);
        assert!(matches!(
            result,
            Err(ProjectServiceError::InvalidDevScriptWorkingDir(_))
        ));
    }

    #[test]
    fn validate_dev_script_update_rejects_parent_traversal() {
        let mut payload = empty_update_payload();
        payload.dev_script = Some("npm run dev".to_string());
        payload.dev_script_working_dir = Some("../outside".to_string());

        let result = ProjectService::validate_dev_script_update(&payload);
        assert!(matches!(
            result,
            Err(ProjectServiceError::InvalidDevScriptWorkingDir(_))
        ));
    }

    #[test]
    fn validate_dev_script_update_rejects_invalid_scheduler_max_concurrent() {
        let mut payload = empty_update_payload();
        payload.scheduler_max_concurrent = Some(0);

        let result = ProjectService::validate_dev_script_update(&payload);
        assert!(matches!(
            result,
            Err(ProjectServiceError::InvalidSchedulerSetting(_))
        ));
    }

    #[test]
    fn validate_dev_script_update_rejects_invalid_scheduler_max_retries() {
        let mut payload = empty_update_payload();
        payload.scheduler_max_retries = Some(-1);

        let result = ProjectService::validate_dev_script_update(&payload);
        assert!(matches!(
            result,
            Err(ProjectServiceError::InvalidSchedulerSetting(_))
        ));
    }

    #[test]
    fn validate_after_prepare_hook_accepts_workspace_relative_command() {
        let mut payload = empty_update_payload();
        payload.after_prepare_hook = Some(Some(WorkspaceLifecycleHookConfig {
            command: "pnpm install --frozen-lockfile".to_string(),
            working_dir: Some("frontend".to_string()),
            failure_policy: WorkspaceLifecycleHookFailurePolicy::BlockStart,
            run_mode: Some(WorkspaceLifecycleHookRunMode::OncePerWorkspace),
        }));

        let result = ProjectService::validate_dev_script_update(&payload);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_after_prepare_hook_rejects_cleanup_failure_policy() {
        let mut payload = empty_update_payload();
        payload.after_prepare_hook = Some(Some(WorkspaceLifecycleHookConfig {
            command: "pnpm install".to_string(),
            working_dir: None,
            failure_policy: WorkspaceLifecycleHookFailurePolicy::BlockCleanup,
            run_mode: Some(WorkspaceLifecycleHookRunMode::EveryPrepare),
        }));

        let result = ProjectService::validate_dev_script_update(&payload);
        assert!(matches!(
            result,
            Err(ProjectServiceError::InvalidWorkspaceLifecycleHook(message))
                if message.contains("after_prepare hooks only support")
        ));
    }

    #[test]
    fn validate_before_cleanup_hook_rejects_run_mode() {
        let mut payload = empty_update_payload();
        payload.before_cleanup_hook = Some(Some(WorkspaceLifecycleHookConfig {
            command: "scripts/cleanup-artifacts".to_string(),
            working_dir: Some("repo-a".to_string()),
            failure_policy: WorkspaceLifecycleHookFailurePolicy::WarnOnly,
            run_mode: Some(WorkspaceLifecycleHookRunMode::OncePerWorkspace),
        }));

        let result = ProjectService::validate_dev_script_update(&payload);
        assert!(matches!(
            result,
            Err(ProjectServiceError::InvalidWorkspaceLifecycleHook(message))
                if message.contains("before_cleanup hooks do not support run_mode")
        ));
    }

    #[test]
    fn validate_hook_rejects_absolute_working_dir() {
        let mut payload = empty_update_payload();
        payload.after_prepare_hook = Some(Some(WorkspaceLifecycleHookConfig {
            command: "pnpm install".to_string(),
            working_dir: Some("/tmp".to_string()),
            failure_policy: WorkspaceLifecycleHookFailurePolicy::WarnOnly,
            run_mode: Some(WorkspaceLifecycleHookRunMode::OncePerWorkspace),
        }));

        let result = ProjectService::validate_dev_script_update(&payload);
        assert!(matches!(
            result,
            Err(ProjectServiceError::InvalidWorkspaceLifecycleHook(message))
                if message.contains("must be relative to the workspace root")
        ));
    }

    #[test]
    fn validate_hook_rejects_parent_traversal_working_dir() {
        let mut payload = empty_update_payload();
        payload.before_cleanup_hook = Some(Some(WorkspaceLifecycleHookConfig {
            command: "scripts/cleanup".to_string(),
            working_dir: Some("../outside".to_string()),
            failure_policy: WorkspaceLifecycleHookFailurePolicy::BlockCleanup,
            run_mode: None,
        }));

        let result = ProjectService::validate_dev_script_update(&payload);
        assert!(matches!(
            result,
            Err(ProjectServiceError::InvalidWorkspaceLifecycleHook(message))
                if message.contains("cannot traverse outside the workspace")
        ));
    }

    #[tokio::test]
    async fn add_repository_concurrent_requests_clear_default_working_dir() {
        let db = setup_db().await;
        let project_service = ProjectService::new();
        let repo_service = RepoService::new();
        let git = GitService::new();

        let project_id = Uuid::new_v4();
        Project::create(
            &db.pool,
            &CreateProject {
                name: "race-test".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();
        let mut update = empty_update_payload();
        update.default_agent_working_dir = Some("repo-a".to_string());
        Project::update(&db.pool, project_id, &update)
            .await
            .unwrap();

        let tmp = tempdir().unwrap();
        let repo_a = tmp.path().join("repo-a");
        let repo_b = tmp.path().join("repo-b");
        git.initialize_repo_with_main_branch(&repo_a).unwrap();
        git.initialize_repo_with_main_branch(&repo_b).unwrap();

        let payload_a = CreateProjectRepo {
            display_name: "Repo A".to_string(),
            git_repo_path: repo_a.to_string_lossy().to_string(),
        };
        let payload_b = CreateProjectRepo {
            display_name: "Repo B".to_string(),
            git_repo_path: repo_b.to_string_lossy().to_string(),
        };

        let (result_a, result_b) = tokio::join!(
            project_service.add_repository(&db.pool, &repo_service, project_id, &payload_a),
            project_service.add_repository(&db.pool, &repo_service, project_id, &payload_b)
        );
        assert!(result_a.is_ok(), "first add failed: {result_a:?}");
        assert!(result_b.is_ok(), "second add failed: {result_b:?}");

        let project = Project::find_by_id(&db.pool, project_id)
            .await
            .unwrap()
            .unwrap();
        assert!(
            project
                .default_agent_working_dir
                .as_deref()
                .unwrap_or_default()
                .is_empty()
        );

        let repos = ProjectRepo::find_by_project_id(&db.pool, project_id)
            .await
            .unwrap();
        assert_eq!(repos.len(), 2);
    }
}
