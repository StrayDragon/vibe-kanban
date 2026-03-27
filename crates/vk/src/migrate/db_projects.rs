use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use executors_protocol::ExecutorProfileId;
use serde::Serialize;

use crate::migrate::io::{atomic_write_out, sqlite_path_from_url, timestamp_slug};

#[derive(Debug, Default, Clone)]
pub struct ExportDbProjectsYamlArgs {
    pub out: Option<PathBuf>,
    pub install: bool,
    pub dry_run: bool,
    pub print_paths: bool,
    pub config_dir_override: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportDbProjectsYamlParseResult {
    Help,
    Run,
}

pub const EXPORT_DB_PROJECTS_YAML_DEPRECATION_WARNING: &str = "DEPRECATED: DB-backed projects/repos settings are legacy. This migration tool will be removed in a future release.";

pub fn export_db_projects_yaml_help() -> &'static str {
    r#"Export DB-backed project/repo settings to a YAML file.

DEPRECATED:
  DB-backed projects/repos settings are legacy. This migration tool will be removed in a future release.

Usage:
  export-db-projects-yaml --out <path>
  export-db-projects-yaml --out -
  export-db-projects-yaml --install
  export-db-projects-yaml --install --dry-run
  export-db-projects-yaml --print-paths

Notes:
  - Output is a minimal YAML containing only `projects: ...`
  - Secrets are NOT exported
  - Result is sanitized to be loadable by VK's YAML config loader
  - DB location is resolved the same way as VK:
    - Prefer `DATABASE_URL` (SQLite)
    - Otherwise `${VIBE_ASSET_DIR:-default_asset_dir}/db.sqlite`
  - Output directory for --install is resolved the same way as VK:
    - Prefer `VK_CONFIG_DIR`
    - Otherwise OS config dir (~/.config/vk on Linux/macOS, %APPDATA%\\vk on Windows)
  - --install writes a new file (projects.migrated.<timestamp>.yaml) and does NOT overwrite projects.yaml
"#
}

pub fn parse_export_db_projects_yaml_args(
    args: impl IntoIterator<Item = String>,
) -> Result<(ExportDbProjectsYamlArgs, ExportDbProjectsYamlParseResult)> {
    let mut out = None;
    let mut install = false;
    let mut dry_run = false;
    let mut print_paths = false;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--out" {
            let value = args.next().context("Missing value for --out")?;
            let trimmed = value.trim();
            if trimmed.is_empty() {
                anyhow::bail!("--out must not be empty");
            }
            out = Some(PathBuf::from(trimmed));
            continue;
        }
        if arg == "--install" {
            install = true;
            continue;
        }
        if arg == "--dry-run" {
            dry_run = true;
            continue;
        }
        if arg == "--print-paths" {
            print_paths = true;
            continue;
        }
        if arg == "--help" || arg == "-h" {
            return Ok((
                ExportDbProjectsYamlArgs::default(),
                ExportDbProjectsYamlParseResult::Help,
            ));
        }

        anyhow::bail!("Unknown argument: {arg}");
    }

    if out.is_some() && install {
        anyhow::bail!("--out and --install are mutually exclusive");
    }

    Ok((
        ExportDbProjectsYamlArgs {
            out,
            install,
            dry_run,
            print_paths,
            config_dir_override: None,
        },
        ExportDbProjectsYamlParseResult::Run,
    ))
}

pub async fn run(args: Vec<String>) -> Result<()> {
    let (parsed, action) = parse_export_db_projects_yaml_args(args)?;
    if action == ExportDbProjectsYamlParseResult::Help {
        println!("{}", export_db_projects_yaml_help());
        return Ok(());
    }
    run_export_db_projects_yaml(parsed).await
}

fn resolved_database_url() -> Result<String> {
    match std::env::var("DATABASE_URL") {
        Ok(url) => {
            let trimmed = url.trim();
            if trimmed.is_empty() {
                anyhow::bail!("DATABASE_URL is set but empty");
            }
            if !trimmed.starts_with("sqlite:") {
                anyhow::bail!("Only sqlite DATABASE_URL values are supported");
            }
            Ok(trimmed.to_string())
        }
        Err(std::env::VarError::NotPresent) => {
            let db_path = utils_assets::asset_dir().join("db.sqlite");
            Ok(format!("sqlite://{}?mode=rwc", db_path.to_string_lossy()))
        }
        Err(err) => anyhow::bail!("Failed to read DATABASE_URL: {err}"),
    }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn sanitize_repo_script(
    value: Option<String>,
    field: &str,
    project_id: uuid::Uuid,
) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.len() > 1024 * 64 {
        eprintln!(
            "warning: project {} {} is too large ({} bytes), dropping it",
            project_id,
            field,
            trimmed.len()
        );
        return None;
    }

    Some(trimmed.to_string())
}

fn sanitize_workspace_relative_dir(
    value: Option<String>,
    field: &str,
    project_id: uuid::Uuid,
) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let path = Path::new(trimmed);
    if path.is_absolute() {
        eprintln!(
            "warning: project {} {} is absolute path ('{}'), dropping it",
            project_id, field, trimmed
        );
        return None;
    }

    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            eprintln!(
                "warning: project {} {} contains '..' ('{}'), dropping it",
                project_id, field, trimmed
            );
            return None;
        }
    }

    Some(trimmed.to_string())
}

fn map_failure_policy(
    policy: db::types::WorkspaceLifecycleHookFailurePolicy,
) -> config::WorkspaceLifecycleHookFailurePolicy {
    match policy {
        db::types::WorkspaceLifecycleHookFailurePolicy::WarnOnly => {
            config::WorkspaceLifecycleHookFailurePolicy::WarnOnly
        }
        db::types::WorkspaceLifecycleHookFailurePolicy::BlockStart => {
            config::WorkspaceLifecycleHookFailurePolicy::BlockStart
        }
        db::types::WorkspaceLifecycleHookFailurePolicy::BlockCleanup => {
            config::WorkspaceLifecycleHookFailurePolicy::BlockCleanup
        }
    }
}

fn map_run_mode(
    mode: db::types::WorkspaceLifecycleHookRunMode,
) -> config::WorkspaceLifecycleHookRunMode {
    match mode {
        db::types::WorkspaceLifecycleHookRunMode::OncePerWorkspace => {
            config::WorkspaceLifecycleHookRunMode::OncePerWorkspace
        }
        db::types::WorkspaceLifecycleHookRunMode::EveryPrepare => {
            config::WorkspaceLifecycleHookRunMode::EveryPrepare
        }
    }
}

fn export_hook(
    hook: db::models::project::WorkspaceLifecycleHookConfig,
    phase: &str,
    project_id: uuid::Uuid,
) -> Option<config::WorkspaceLifecycleHookConfig> {
    let command = hook.command.trim();
    if command.is_empty() {
        eprintln!("warning: project {project_id} {phase} hook has empty command, dropping hook");
        return None;
    }

    let mut failure_policy = map_failure_policy(hook.failure_policy);
    let mut run_mode = hook.run_mode.map(map_run_mode);

    match phase {
        "after_prepare_hook" => {
            if matches!(
                failure_policy,
                config::WorkspaceLifecycleHookFailurePolicy::BlockCleanup
            ) {
                eprintln!(
                    "warning: project {project_id} after_prepare_hook has invalid failure_policy=block_cleanup, resetting to warn_only"
                );
                failure_policy = config::WorkspaceLifecycleHookFailurePolicy::WarnOnly;
            }
            if run_mode.is_none() {
                eprintln!(
                    "warning: project {project_id} after_prepare_hook missing run_mode, dropping hook"
                );
                return None;
            }
        }
        "before_cleanup_hook" => {
            if matches!(
                failure_policy,
                config::WorkspaceLifecycleHookFailurePolicy::BlockStart
            ) {
                eprintln!(
                    "warning: project {project_id} before_cleanup_hook has invalid failure_policy=block_start, resetting to warn_only"
                );
                failure_policy = config::WorkspaceLifecycleHookFailurePolicy::WarnOnly;
            }
            // before_cleanup does not support run_mode.
            run_mode = None;
        }
        _ => {}
    }

    let working_dir = sanitize_workspace_relative_dir(hook.working_dir, phase, project_id);

    Some(config::WorkspaceLifecycleHookConfig {
        command: command.to_string(),
        working_dir,
        failure_policy,
        run_mode,
    })
}

pub async fn export_projects_yaml(pool: &db::DbPool) -> Result<String> {
    let profiles = executors::profile::ExecutorConfigs::from_defaults();
    let db_projects = db::models::project::Project::find_all(pool)
        .await
        .context("Failed to list projects")?;

    let mut projects = Vec::new();
    for project in db_projects {
        let mut repos = Vec::new();
        let project_repos =
            db::models::project_repo::ProjectRepo::find_by_project_id_with_names(pool, project.id)
                .await
                .with_context(|| {
                    format!("Failed to load project repos for project {}", project.id)
                })?;

        for project_repo in project_repos {
            let Some(repo) = db::models::repo::Repo::find_by_id(pool, project_repo.repo_id)
                .await
                .with_context(|| {
                    format!(
                        "Failed to load repo {} (project {})",
                        project_repo.repo_id, project.id
                    )
                })?
            else {
                eprintln!(
                    "warning: project {} references missing repo {}, skipping it",
                    project.id, project_repo.repo_id
                );
                continue;
            };

            if !repo.path.is_absolute() {
                eprintln!(
                    "warning: project {} repo {} path is not absolute ('{}'), skipping it",
                    project.id,
                    repo.id,
                    repo.path.to_string_lossy()
                );
                continue;
            }

            repos.push(config::ProjectRepoConfig {
                path: repo.path.to_string_lossy().to_string(),
                display_name: normalize_optional_string(Some(repo.display_name)),
                setup_script: sanitize_repo_script(
                    project_repo.setup_script,
                    "setup_script",
                    project.id,
                ),
                cleanup_script: sanitize_repo_script(
                    project_repo.cleanup_script,
                    "cleanup_script",
                    project.id,
                ),
                copy_files: normalize_optional_string(project_repo.copy_files),
                parallel_setup_script: project_repo.parallel_setup_script,
            });
        }

        if repos.is_empty() {
            eprintln!(
                "warning: project {} has no valid repos to export; skipping project",
                project.id
            );
            continue;
        }

        let mut policy_mode = match project.mcp_auto_executor_policy_mode {
            db::types::ProjectMcpExecutorPolicyMode::InheritAll => {
                config::ProjectMcpExecutorPolicyMode::InheritAll
            }
            db::types::ProjectMcpExecutorPolicyMode::AllowList => {
                config::ProjectMcpExecutorPolicyMode::AllowList
            }
        };
        let mut allow_list = Vec::new();
        for entry in project.mcp_auto_executor_policy_allow_list {
            let executor_raw = entry.executor.trim();
            if executor_raw.is_empty() {
                continue;
            }

            let executor = match std::str::FromStr::from_str(executor_raw) {
                Ok(executor) => executor,
                Err(_) => {
                    eprintln!(
                        "warning: project {} has unsupported allow-list executor '{}', skipping it",
                        project.id, executor_raw
                    );
                    continue;
                }
            };

            let executor_profile_id = ExecutorProfileId {
                executor,
                variant: normalize_optional_string(entry.variant),
            };
            if let Err(err) = profiles.require_coding_agent(&executor_profile_id) {
                eprintln!(
                    "warning: project {} allow-list entry '{}' is not supported by this build, skipping it: {err}",
                    project.id, executor_profile_id
                );
                continue;
            }

            allow_list.push(executor_profile_id);
        }
        if matches!(policy_mode, config::ProjectMcpExecutorPolicyMode::AllowList)
            && allow_list.is_empty()
        {
            eprintln!(
                "warning: project {} policy mode is allow_list but allow-list is empty after filtering; falling back to inherit_all",
                project.id
            );
            policy_mode = config::ProjectMcpExecutorPolicyMode::InheritAll;
        }

        let after_prepare_hook = project
            .after_prepare_hook
            .and_then(|hook| export_hook(hook, "after_prepare_hook", project.id));
        let before_cleanup_hook = project
            .before_cleanup_hook
            .and_then(|hook| export_hook(hook, "before_cleanup_hook", project.id));

        let dev_script = sanitize_repo_script(project.dev_script, "dev_script", project.id);

        projects.push(config::ProjectConfig {
            id: Some(project.id),
            remote_project_id: project.remote_project_id,
            name: project.name,
            repos,
            dev_script,
            dev_script_working_dir: sanitize_workspace_relative_dir(
                project.dev_script_working_dir,
                "dev_script_working_dir",
                project.id,
            ),
            default_agent_working_dir: sanitize_workspace_relative_dir(
                project.default_agent_working_dir,
                "default_agent_working_dir",
                project.id,
            ),
            git_no_verify_override: project.git_no_verify_override,
            scheduler_max_concurrent: project.scheduler_max_concurrent,
            scheduler_max_retries: project.scheduler_max_retries,
            default_continuation_turns: project.default_continuation_turns,
            mcp_auto_executor_policy_mode: policy_mode,
            mcp_auto_executor_policy_allow_list: allow_list,
            after_prepare_hook,
            before_cleanup_hook,
        });
    }

    #[derive(Serialize)]
    struct ExportedConfig {
        projects: Vec<config::ProjectConfig>,
    }

    let yaml = serde_yaml::to_string(&ExportedConfig { projects })
        .context("Failed to serialize export as YAML")?;
    Ok(yaml)
}

fn build_projects_yaml_file(contents_yaml: &str, tool_name: &str) -> Result<String> {
    let header = "# yaml-language-server: $schema=./projects.schema.json\n";
    let generated = format!(
        "# Generated by {tool_name} at {}\n",
        Utc::now().to_rfc3339()
    );

    let mut out = String::new();
    out.push_str(header);
    out.push_str(&generated);
    out.push_str(contents_yaml);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

fn validate_exported_projects_yaml(projects_yaml: &str) -> Result<()> {
    let validate_dir =
        std::env::temp_dir().join(format!("vk-export-validate-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&validate_dir).context("Failed to create validation temp dir")?;
    let validate_config_path = validate_dir.join("config.yaml");
    let validate_projects_path = validate_dir.join("projects.yaml");
    std::fs::write(&validate_config_path, "{}\n")
        .context("Failed to write validation temp config.yaml")?;
    std::fs::write(&validate_projects_path, projects_yaml)
        .context("Failed to write validation temp projects.yaml")?;

    match config::try_load_config_from_file(&validate_config_path) {
        Ok(_) => {
            let _ = std::fs::remove_dir_all(&validate_dir);
            Ok(())
        }
        Err(err) => Err(anyhow::anyhow!(
            "Exported YAML is not loadable by VK config loader (validation dir: {}): {}",
            validate_dir.display(),
            err
        )),
    }
}

pub async fn run_export_db_projects_yaml(args: ExportDbProjectsYamlArgs) -> Result<()> {
    eprintln!("{EXPORT_DB_PROJECTS_YAML_DEPRECATION_WARNING}");

    let database_url = resolved_database_url()?;
    let config_dir = args
        .config_dir_override
        .clone()
        .unwrap_or_else(utils_core::vk_config_dir);
    let config_path = config_dir.join("config.yaml");
    let projects_path = config_dir.join("projects.yaml");
    let secret_env_path = config_dir.join("secret.env");

    if args.print_paths {
        println!("database_url: {database_url}");
        if let Some(path) = sqlite_path_from_url(&database_url) {
            println!("database_file: {}", path.display());
        }
        println!("config_dir: {}", config_dir.display());
        println!("config_yaml: {}", config_path.display());
        println!("projects_yaml: {}", projects_path.display());
        println!("secret_env: {}", secret_env_path.display());
        return Ok(());
    }

    if !args.install && args.out.is_none() {
        anyhow::bail!("Missing output mode: use --out <path> or --install (see --help)");
    }

    let db = db::DBService::new()
        .await
        .context("Failed to open database")?;
    let yaml = export_projects_yaml(&db.pool).await?;
    validate_exported_projects_yaml(&yaml)?;
    let file_contents = build_projects_yaml_file(&yaml, "vk migrate export-db-projects-yaml")?;

    if args.install {
        let slug = timestamp_slug();
        let out_path = config_dir.join(format!("projects.migrated.{slug}.yaml"));

        if args.dry_run {
            println!(
                "dry-run: would write migrated projects YAML to {}",
                out_path.display()
            );
            return Ok(());
        }

        atomic_write_out(&out_path, &file_contents)?;
        println!("Wrote migrated projects YAML to {}", out_path.display());
        eprintln!("next: merge into projects.yaml (or projects.d/*.yaml), then reload.");
        return Ok(());
    }

    let out_path = args.out.expect("checked above");
    if out_path.as_os_str() == "-" {
        print!("{file_contents}");
        return Ok(());
    }

    if args.dry_run {
        println!("dry-run: would write export to {}", out_path.display());
        return Ok(());
    }

    atomic_write_out(&out_path, &file_contents)?;
    println!("Exported projects YAML to {}", out_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};

    use super::*;

    struct TestEnvGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        prev_database_url: Option<String>,
        prev_asset_dir: Option<String>,
        prev_disable_background_tasks: Option<String>,
        config_dir: PathBuf,
    }

    fn test_lock() -> &'static std::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    impl TestEnvGuard {
        fn new(temp_root: &Path, db_url: String) -> Self {
            let lock = test_lock().lock().unwrap_or_else(|err| err.into_inner());
            let prev_database_url = std::env::var("DATABASE_URL").ok();
            let prev_asset_dir = std::env::var("VIBE_ASSET_DIR").ok();
            let prev_disable_background_tasks = std::env::var("VIBE_DISABLE_BACKGROUND_TASKS").ok();

            let config_dir = temp_root.join("vk-config");
            std::fs::create_dir_all(&config_dir).unwrap();

            // SAFETY: tests using TestEnvGuard are serialized by test_lock.
            unsafe {
                std::env::set_var("VIBE_ASSET_DIR", temp_root);
                std::env::set_var("DATABASE_URL", db_url);
                std::env::set_var("VIBE_DISABLE_BACKGROUND_TASKS", "1");
            }

            Self {
                _lock: lock,
                prev_database_url,
                prev_asset_dir,
                prev_disable_background_tasks,
                config_dir,
            }
        }
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            // SAFETY: tests using TestEnvGuard are serialized by test_lock.
            unsafe {
                match &self.prev_database_url {
                    Some(value) => std::env::set_var("DATABASE_URL", value),
                    None => std::env::remove_var("DATABASE_URL"),
                }
                match &self.prev_asset_dir {
                    Some(value) => std::env::set_var("VIBE_ASSET_DIR", value),
                    None => std::env::remove_var("VIBE_ASSET_DIR"),
                }
                match &self.prev_disable_background_tasks {
                    Some(value) => std::env::set_var("VIBE_DISABLE_BACKGROUND_TASKS", value),
                    None => std::env::remove_var("VIBE_DISABLE_BACKGROUND_TASKS"),
                }
            }
        }
    }

    #[tokio::test]
    async fn export_yaml_is_loadable_by_config_loader() {
        let temp_root =
            std::env::temp_dir().join(format!("vk-export-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _env_guard = TestEnvGuard::new(&temp_root, "sqlite::memory:".to_string());

        let db = db::DBService::new().await.expect("db service");

        let repo_path = temp_root.join("repo");
        std::fs::create_dir_all(&repo_path).unwrap();

        let project_id = uuid::Uuid::new_v4();
        let _project = db::models::project::Project::create(
            &db.pool,
            &db::models::project::CreateProject {
                name: "Export me".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .expect("create project");

        let repo_path_str = repo_path.to_string_lossy().to_string();
        let _repo = db::models::project_repo::ProjectRepo::add_repo_to_project(
            &db.pool,
            project_id,
            &repo_path_str,
            "Repo",
        )
        .await
        .expect("add repo");

        // Add an allow-list policy containing the default executor profile.
        let record = db::entities::project::Entity::find()
            .filter(db::entities::project::Column::Uuid.eq(project_id))
            .one(&db.pool)
            .await
            .unwrap()
            .expect("project record");
        let mut active: db::entities::project::ActiveModel = record.into();
        active.mcp_auto_executor_policy_mode =
            sea_orm::ActiveValue::Set(db::types::ProjectMcpExecutorPolicyMode::AllowList);
        active.mcp_auto_executor_policy_allow_list_json =
            sea_orm::ActiveValue::Set(Some(serde_json::json!([{
              "executor": "CLAUDE_CODE",
              "variant": null
            }])));
        let _ = active.update(&db.pool).await.unwrap();

        let yaml = export_projects_yaml(&db.pool).await.expect("export yaml");
        assert!(yaml.contains("projects:"));
        assert!(yaml.contains("mcp_auto_executor_policy_mode: allow_list"));

        let config_path = temp_root.join("config.yaml");
        let projects_path = temp_root.join("projects.yaml");
        std::fs::write(&config_path, "{}\n").unwrap();
        std::fs::write(&projects_path, &yaml).unwrap();

        let loaded = config::try_load_config_from_file(&config_path).expect("load config");
        assert_eq!(loaded.projects.len(), 1);
        let exported_project = &loaded.projects[0];
        assert_eq!(exported_project.id, Some(project_id));
        assert_eq!(exported_project.repos.len(), 1);
    }

    #[tokio::test]
    async fn install_writes_migrated_file_without_overwriting_projects_yaml() {
        let temp_root =
            std::env::temp_dir().join(format!("vk-export-install-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let _env_guard = TestEnvGuard::new(&temp_root, db_url);

        let db = db::DBService::new().await.expect("db service");

        let repo_path = temp_root.join("repo");
        std::fs::create_dir_all(&repo_path).unwrap();

        let project_id = uuid::Uuid::new_v4();
        let _project = db::models::project::Project::create(
            &db.pool,
            &db::models::project::CreateProject {
                name: "Export me".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .expect("create project");

        let repo_path_str = repo_path.to_string_lossy().to_string();
        let _repo = db::models::project_repo::ProjectRepo::add_repo_to_project(
            &db.pool,
            project_id,
            &repo_path_str,
            "Repo",
        )
        .await
        .expect("add repo");

        run_export_db_projects_yaml(ExportDbProjectsYamlArgs {
            install: true,
            out: None,
            dry_run: false,
            print_paths: false,
            config_dir_override: Some(_env_guard.config_dir.clone()),
        })
        .await
        .expect("run export-db-projects-yaml --install");

        let config_dir = &_env_guard.config_dir;
        assert!(!config_dir.join("projects.yaml").exists());

        let migrated = std::fs::read_dir(config_dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with("projects.migrated.") && name.ends_with(".yaml")
                    })
            })
            .collect::<Vec<_>>();
        assert_eq!(migrated.len(), 1);

        let raw = std::fs::read_to_string(&migrated[0]).expect("read migrated file");
        assert!(raw.contains("yaml-language-server"));
        assert!(raw.contains("projects:"));
    }
}
