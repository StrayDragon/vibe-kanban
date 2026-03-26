use std::{
    path::{Component, Path, PathBuf},
    str::FromStr,
};

use anyhow::{Context, Result};
use executors_protocol::{BaseCodingAgent, ExecutorProfileId};
use serde::Serialize;

fn parse_out_path() -> Result<PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--out" {
            let value = args.next().context("Missing value for --out")?;
            let trimmed = value.trim();
            if trimmed.is_empty() {
                anyhow::bail!("--out must not be empty");
            }
            return Ok(PathBuf::from(trimmed));
        }
        if arg == "--help" || arg == "-h" {
            println!(
                r#"Export DB-backed project/repo settings to a YAML snippet.

Usage:
  cargo run --bin export_db_projects_yaml -- --out <path>

Notes:
  - Output is a minimal YAML containing only `projects: ...`
  - Secrets are NOT exported
  - Result is sanitized to be loadable by VK's YAML config loader
"#
            );
            std::process::exit(0);
        }
    }

    anyhow::bail!("Missing required argument: --out <path>");
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

fn validate_single_command_text(label: &str, script: &str) -> Result<()> {
    let trimmed = script.trim();
    if trimmed.is_empty() {
        anyhow::bail!("{label} command cannot be empty");
    }

    let tokens = shlex::split(trimmed)
        .with_context(|| format!("{label} command must be valid shell-like command text"))?;

    if tokens.is_empty() {
        anyhow::bail!("{label} command must include an executable");
    }

    let has_forbidden = tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "|" | "||" | "&" | "&&" | ";" | ">" | ">>" | "<" | "<<"
        )
    });
    if has_forbidden {
        anyhow::bail!("{label} command must be a single command without shell operators");
    }

    Ok(())
}

fn sanitize_repo_script(
    value: Option<String>,
    label: &str,
    project_id: uuid::Uuid,
) -> Option<String> {
    let script = normalize_optional_string(value)?;
    if let Err(err) = validate_single_command_text(label, &script) {
        eprintln!(
            "warning: project {project_id} has invalid {label}, dropping it: {err}"
        );
        return None;
    }
    Some(script)
}

fn sanitize_workspace_relative_dir(
    value: Option<String>,
    label: &str,
    project_id: uuid::Uuid,
) -> Option<String> {
    let dir = normalize_optional_string(value)?;
    let path = Path::new(&dir);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        eprintln!("warning: project {project_id} has invalid {label}='{dir}', dropping it");
        return None;
    }
    Some(dir)
}

fn map_failure_policy(
    value: db::types::WorkspaceLifecycleHookFailurePolicy,
) -> config::WorkspaceLifecycleHookFailurePolicy {
    match value {
        db::types::WorkspaceLifecycleHookFailurePolicy::BlockStart => {
            config::WorkspaceLifecycleHookFailurePolicy::BlockStart
        }
        db::types::WorkspaceLifecycleHookFailurePolicy::WarnOnly => {
            config::WorkspaceLifecycleHookFailurePolicy::WarnOnly
        }
        db::types::WorkspaceLifecycleHookFailurePolicy::BlockCleanup => {
            config::WorkspaceLifecycleHookFailurePolicy::BlockCleanup
        }
    }
}

fn map_run_mode(
    value: db::types::WorkspaceLifecycleHookRunMode,
) -> config::WorkspaceLifecycleHookRunMode {
    match value {
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
        return None;
    }

    if let Err(err) = validate_single_command_text(phase, command) {
        eprintln!(
            "warning: project {project_id} has invalid {phase} hook command, dropping hook: {err}"
        );
        return None;
    }

    let mut failure_policy = map_failure_policy(hook.failure_policy);
    let mut run_mode = hook.run_mode.map(map_run_mode);

    match phase {
        "after_prepare_hook" => {
            if matches!(failure_policy, config::WorkspaceLifecycleHookFailurePolicy::BlockCleanup) {
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
            if matches!(failure_policy, config::WorkspaceLifecycleHookFailurePolicy::BlockStart) {
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

async fn export_projects_yaml(pool: &db::DbPool) -> Result<String> {
    let profiles = executors::profile::ExecutorConfigs::from_defaults();
    let db_projects = db::models::project::Project::find_all(pool)
        .await
        .context("Failed to list projects")?;

    let mut projects = Vec::new();
    for project in db_projects {
        let mut repos = Vec::new();
        let project_repos = db::models::project_repo::ProjectRepo::find_by_project_id_with_names(
            pool,
            project.id,
        )
        .await
        .with_context(|| format!("Failed to load project repos for project {}", project.id))?;

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

            let executor = match BaseCodingAgent::from_str(executor_raw) {
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

fn write_out(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!("Failed to create output directory {}", parent.to_string_lossy())
        })?;
    }

    std::fs::write(path, contents).with_context(|| {
        format!(
            "Failed to write output file {}",
            path.to_string_lossy()
        )
    })?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let out_path = parse_out_path()?;

    let db = db::DBService::new().await.context("Failed to open database")?;
    let yaml = export_projects_yaml(&db.pool).await?;
    write_out(&out_path, &yaml)?;

    println!("Exported projects YAML to {}", out_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{Mutex, OnceLock};

    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn env_lock() -> &'static Mutex<()> {
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvGuard {
        prev_database_url: Option<String>,
        prev_asset_dir: Option<String>,
    }

    impl EnvGuard {
        fn new(temp_root: &Path) -> Self {
            let prev_database_url = std::env::var("DATABASE_URL").ok();
            let prev_asset_dir = std::env::var("VIBE_ASSET_DIR").ok();

            unsafe {
                std::env::set_var("DATABASE_URL", "sqlite::memory:");
                std::env::set_var("VIBE_ASSET_DIR", temp_root);
            }

            Self {
                prev_database_url,
                prev_asset_dir,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.prev_database_url {
                    Some(value) => std::env::set_var("DATABASE_URL", value),
                    None => std::env::remove_var("DATABASE_URL"),
                }
                match &self.prev_asset_dir {
                    Some(value) => std::env::set_var("VIBE_ASSET_DIR", value),
                    None => std::env::remove_var("VIBE_ASSET_DIR"),
                }
            }
        }
    }

    #[tokio::test]
    async fn export_yaml_is_loadable_by_config_loader() {
        let _guard = env_lock().lock().unwrap();

        let temp_root = std::env::temp_dir().join(format!("vk-export-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _env = EnvGuard::new(&temp_root);

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
        active.mcp_auto_executor_policy_allow_list_json = sea_orm::ActiveValue::Set(Some(
            serde_json::json!([{
              "executor": "CLAUDE_CODE",
              "variant": null
            }]),
        ));
        let _ = active.update(&db.pool).await.unwrap();

        let yaml = export_projects_yaml(&db.pool).await.expect("export yaml");
        assert!(yaml.contains("projects:"));
        assert!(yaml.contains("mcp_auto_executor_policy_mode: allow_list"));

        let config_path = temp_root.join("config.yaml");
        std::fs::write(&config_path, &yaml).unwrap();

        let loaded = config::try_load_config_from_file(&config_path).expect("load config");
        assert_eq!(loaded.projects.len(), 1);
        let exported_project = &loaded.projects[0];
        assert_eq!(exported_project.id, Some(project_id));
        assert_eq!(exported_project.repos.len(), 1);
    }
}
