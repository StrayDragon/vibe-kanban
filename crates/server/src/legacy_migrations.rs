use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
    str::FromStr,
};

use anyhow::{Context, Result};
use chrono::Utc;
use executors_protocol::{BaseCodingAgent, ExecutorProfileId};
use serde::Serialize;
use utils_assets::asset_dir;

#[derive(Debug, Default, Clone)]
pub struct ExportDbProjectsYamlArgs {
    pub out: Option<PathBuf>,
    pub install: bool,
    pub dry_run: bool,
    pub print_paths: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportDbProjectsYamlParseResult {
    Help,
    Run,
}

pub const EXPORT_DB_PROJECTS_YAML_DEPRECATION_WARNING: &str = "DEPRECATED: DB-backed projects/repos settings are legacy. This migration tool will be removed in a future release.";

#[derive(Debug, Default, Clone)]
pub struct ExportAssetConfigYamlArgs {
    pub out: Option<PathBuf>,
    pub install: bool,
    pub dry_run: bool,
    pub print_paths: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportAssetConfigYamlParseResult {
    Help,
    Run,
}

pub const EXPORT_ASSET_CONFIG_YAML_DEPRECATION_WARNING: &str = "DEPRECATED: asset_dir()/config.json and asset_dir()/profiles.json are legacy. This migration tool will be removed in a future release.";

pub fn export_db_projects_yaml_help() -> &'static str {
    r#"Export DB-backed project/repo settings to a YAML snippet.

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
  - Install target is resolved the same way as VK:
    - Prefer `VK_CONFIG_DIR`
    - Otherwise OS config dir (~/.config/vk on Linux/macOS, %APPDATA%\vk on Windows)
"#
}

pub fn export_asset_config_yaml_help() -> &'static str {
    r#"Export legacy asset config (config.json + profiles.json) into YAML config fields.

DEPRECATED:
  asset_dir()/config.json and asset_dir()/profiles.json are legacy. This migration tool will be removed in a future release.

Usage:
  export-asset-config-yaml --out <path>
  export-asset-config-yaml --out -
  export-asset-config-yaml --install
  export-asset-config-yaml --install --dry-run
  export-asset-config-yaml --print-paths

Notes:
  - Output is a YAML mapping containing selected fields from legacy `config.json` and `profiles.json`
  - Secrets are NOT written into YAML; they are written into `secret.env` and referenced via `{{secret.NAME}}`
  - Only supported executors (default: CLAUDE_CODE + CODEX) are migrated from `profiles.json`
  - Install target is resolved the same way as VK:
    - Prefer `VK_CONFIG_DIR`
    - Otherwise OS config dir (~/.config/vk on Linux/macOS, %APPDATA%\vk on Windows)
  - Legacy source paths are resolved the same way as VK:
    - `${VIBE_ASSET_DIR:-default_asset_dir}/config.json`
    - `${VIBE_ASSET_DIR:-default_asset_dir}/profiles.json`
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
        },
        ExportDbProjectsYamlParseResult::Run,
    ))
}

pub fn parse_export_asset_config_yaml_args(
    args: impl IntoIterator<Item = String>,
) -> Result<(ExportAssetConfigYamlArgs, ExportAssetConfigYamlParseResult)> {
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
                ExportAssetConfigYamlArgs::default(),
                ExportAssetConfigYamlParseResult::Help,
            ));
        }

        anyhow::bail!("Unknown argument: {arg}");
    }

    if out.is_some() && install {
        anyhow::bail!("--out and --install are mutually exclusive");
    }

    Ok((
        ExportAssetConfigYamlArgs {
            out,
            install,
            dry_run,
            print_paths,
        },
        ExportAssetConfigYamlParseResult::Run,
    ))
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
            let db_path = asset_dir().join("db.sqlite");
            Ok(format!("sqlite://{}?mode=rwc", db_path.to_string_lossy()))
        }
        Err(err) => anyhow::bail!("Failed to read DATABASE_URL: {err}"),
    }
}

fn sqlite_path_from_url(database_url: &str) -> Option<PathBuf> {
    let trimmed = database_url.trim();
    if !trimmed.starts_with("sqlite:") {
        return None;
    }
    let mut rest = &trimmed["sqlite:".len()..];
    if rest.starts_with("//") {
        rest = &rest[2..];
    }
    let path_part = rest.split('?').next().unwrap_or(rest);
    if path_part.is_empty() || path_part == ":memory:" {
        return None;
    }
    Some(PathBuf::from(path_part))
}

fn timestamp_slug() -> String {
    Utc::now().format("%Y%m%dT%H%M%SZ").to_string()
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
        eprintln!("warning: project {project_id} has invalid {label}, dropping it: {err}");
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

fn atomic_write_out(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create output directory {}",
                parent.to_string_lossy()
            )
        })?;
    }

    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "vk-export.yaml".to_string());
    let tmp_path = path.with_file_name(format!("{file_name}.tmp-{}", std::process::id()));

    std::fs::write(&tmp_path, contents).with_context(|| {
        format!(
            "Failed to write temporary output file {}",
            tmp_path.to_string_lossy()
        )
    })?;

    if let Err(err) = std::fs::rename(&tmp_path, path) {
        if path.exists() {
            let _ = std::fs::remove_file(path);
            std::fs::rename(&tmp_path, path).with_context(|| {
                format!("Failed to replace output file {}", path.to_string_lossy())
            })?;
            return Ok(());
        }
        return Err(err.into());
    }

    Ok(())
}

fn install_projects_into_user_config(
    config_path: &Path,
    projects_yaml: &str,
    dry_run: bool,
) -> Result<()> {
    let slug = timestamp_slug();
    let config_dir = config_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(utils_core::vk_config_dir);

    let exported_value: serde_yaml::Value =
        serde_yaml::from_str(projects_yaml).context("Failed to parse exported YAML")?;
    let exported_projects = match exported_value {
        serde_yaml::Value::Mapping(map) => map
            .get(&serde_yaml::Value::String("projects".to_string()))
            .cloned()
            .context("Exported YAML did not contain a 'projects' key")?,
        _ => anyhow::bail!("Exported YAML must be a mapping with a 'projects' key"),
    };

    if !config_path.exists() {
        let header = "# yaml-language-server: $schema=./config.schema.json\n";
        let generated = format!(
            "# Generated by export-db-projects-yaml at {}\n",
            Utc::now().to_rfc3339()
        );

        let mut body_map = serde_yaml::Mapping::new();
        body_map.insert(
            serde_yaml::Value::String("projects".to_string()),
            exported_projects,
        );
        let mut contents = String::new();
        contents.push_str(header);
        contents.push_str(&generated);
        contents.push_str(
            &serde_yaml::to_string(&serde_yaml::Value::Mapping(body_map))
                .context("Failed to serialize config.yaml")?,
        );
        if !contents.ends_with('\n') {
            contents.push('\n');
        }

        if dry_run {
            println!(
                "dry-run: would write new config.yaml to {}",
                config_path.display()
            );
            return Ok(());
        }

        atomic_write_out(config_path, &contents)?;
        println!("Installed projects into {}", config_path.display());
        return Ok(());
    }

    let existing_raw = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;

    let existing_value: serde_yaml::Value = match serde_yaml::from_str(&existing_raw) {
        Ok(value) => value,
        Err(err) => {
            let fallback_path = config_dir.join(format!("projects.db-export.{slug}.yaml"));
            let contents = projects_yaml;
            if dry_run {
                println!(
                    "dry-run: existing config.yaml is invalid YAML; would write exported projects to {}",
                    fallback_path.display()
                );
                return Ok(());
            }
            atomic_write_out(&fallback_path, contents)?;
            println!(
                "Existing config.yaml is invalid YAML ({err}); wrote exported projects to {}",
                fallback_path.display()
            );
            println!(
                "Merge {} into {} manually, then reload.",
                fallback_path.display(),
                config_path.display()
            );
            return Ok(());
        }
    };

    let mut existing_map = match existing_value {
        serde_yaml::Value::Null => serde_yaml::Mapping::new(),
        serde_yaml::Value::Mapping(map) => map,
        other => {
            let fallback_path = config_dir.join(format!("projects.db-export.{slug}.yaml"));
            if dry_run {
                println!(
                    "dry-run: existing config.yaml is not a YAML mapping ({:?}); would write exported projects to {}",
                    other,
                    fallback_path.display()
                );
                return Ok(());
            }
            atomic_write_out(&fallback_path, projects_yaml)?;
            println!(
                "Existing config.yaml is not a YAML mapping; wrote exported projects to {}",
                fallback_path.display()
            );
            println!(
                "Merge {} into {} manually, then reload.",
                fallback_path.display(),
                config_path.display()
            );
            return Ok(());
        }
    };

    existing_map.insert(
        serde_yaml::Value::String("projects".to_string()),
        exported_projects,
    );

    let header = "# yaml-language-server: $schema=./config.schema.json\n";
    let generated = format!(
        "# Updated by export-db-projects-yaml at {}\n",
        Utc::now().to_rfc3339()
    );

    let mut new_contents = String::new();
    new_contents.push_str(header);
    new_contents.push_str(&generated);
    new_contents.push_str(
        &serde_yaml::to_string(&serde_yaml::Value::Mapping(existing_map))
            .context("Failed to serialize merged config.yaml")?,
    );
    if !new_contents.ends_with('\n') {
        new_contents.push('\n');
    }

    let backup_path = config_dir.join(format!("config.yaml.bak.{slug}"));
    if dry_run {
        println!(
            "dry-run: would backup existing config.yaml to {}",
            backup_path.display()
        );
        println!(
            "dry-run: would update config.yaml projects in {}",
            config_path.display()
        );
        return Ok(());
    }

    atomic_write_out(&backup_path, &existing_raw)?;
    atomic_write_out(config_path, &new_contents)?;

    println!(
        "Updated projects in {} (backup at {})",
        config_path.display(),
        backup_path.display()
    );
    Ok(())
}

pub async fn run_export_db_projects_yaml(args: ExportDbProjectsYamlArgs) -> Result<()> {
    eprintln!("{EXPORT_DB_PROJECTS_YAML_DEPRECATION_WARNING}");

    let database_url = resolved_database_url()?;
    let config_dir = utils_core::vk_config_dir();
    let config_path = utils_core::vk_config_yaml_path();
    let secret_env_path = utils_core::vk_secret_env_path();

    if args.print_paths {
        println!("database_url: {database_url}");
        if let Some(path) = sqlite_path_from_url(&database_url) {
            println!("database_file: {}", path.display());
        }
        println!("config_dir: {}", config_dir.display());
        println!("config_yaml: {}", config_path.display());
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

    // Validation: ensure exported YAML is loadable by the config loader.
    let validate_dir =
        std::env::temp_dir().join(format!("vk-export-validate-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&validate_dir).context("Failed to create validation temp dir")?;
    let validate_path = validate_dir.join("config.yaml");
    std::fs::write(&validate_path, &yaml).context("Failed to write validation temp file")?;
    match config::try_load_config_from_file(&validate_path) {
        Ok(_) => {
            let _ = std::fs::remove_dir_all(&validate_dir);
        }
        Err(err) => {
            anyhow::bail!(
                "Exported YAML is not loadable by VK config loader (validation file: {}): {}",
                validate_path.display(),
                err
            );
        }
    }

    if args.install {
        install_projects_into_user_config(&config_path, &yaml, args.dry_run)?;
        return Ok(());
    }

    let out_path = args.out.expect("checked above");
    if out_path.as_os_str() == "-" {
        print!("{yaml}");
        return Ok(());
    }

    if args.dry_run {
        println!("dry-run: would write export to {}", out_path.display());
        return Ok(());
    }

    atomic_write_out(&out_path, &yaml)?;
    println!("Exported projects YAML to {}", out_path.display());
    Ok(())
}

fn load_secret_env_for_merge(secret_env_path: &Path) -> Result<HashMap<String, String>> {
    let raw = match std::fs::read_to_string(secret_env_path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(err) => return Err(err.into()),
    };

    let mut vars = HashMap::new();
    for (idx, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        let Some((key, value)) = line.split_once('=') else {
            anyhow::bail!("Invalid secret.env line {}: expected KEY=VALUE", idx + 1);
        };

        let key = key.trim();
        if key.is_empty() {
            anyhow::bail!("Invalid secret.env line {}: empty key", idx + 1);
        }

        let mut value = value.trim().to_string();
        if (value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\''))
        {
            value = value[1..value.len().saturating_sub(1)].to_string();
        }
        vars.insert(key.to_string(), value);
    }

    Ok(vars)
}

fn is_supported_executor_key(key: &str) -> bool {
    matches!(key, "CLAUDE_CODE" | "CODEX")
}

fn is_sensitive_env_key(key: &str, value: &str) -> bool {
    let upper = key.trim().to_ascii_uppercase();
    if upper.contains("TOKEN")
        || upper.contains("PASSWORD")
        || upper.contains("PASSWD")
        || upper.contains("SECRET")
        || upper.contains("PAT")
        || upper.ends_with("_KEY")
        || upper.contains("API_KEY")
        || upper.contains("ACCESS_KEY")
        || upper.contains("PRIVATE_KEY")
    {
        return true;
    }

    let trimmed = value.trim();
    trimmed.starts_with("sk-") || trimmed.starts_with("ghp_")
}

fn rewrite_secret_env_values_in_place(
    env: &mut serde_json::Map<String, serde_json::Value>,
    secrets: &mut HashMap<String, String>,
) {
    let keys = env.keys().cloned().collect::<Vec<_>>();
    for key in keys {
        let Some(value) = env.get_mut(&key) else {
            continue;
        };
        let Some(raw) = value.as_str().map(|s| s.trim()).filter(|s| !s.is_empty()) else {
            continue;
        };

        if !is_sensitive_env_key(&key, raw) {
            continue;
        }

        match secrets.get(&key) {
            Some(existing) if existing != raw => {
                eprintln!(
                    "warning: secret env key '{key}' has multiple values; keeping the first one"
                );
            }
            Some(_) => {}
            None => {
                secrets.insert(key.clone(), raw.to_string());
            }
        }

        *value = serde_json::Value::String(format!("{{{{secret.{key}}}}}"));
    }
}

fn rewrite_secrets_in_env_objects(
    value: &mut serde_json::Value,
    secrets: &mut HashMap<String, String>,
) {
    match value {
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {}
        serde_json::Value::Array(items) => {
            for item in items {
                rewrite_secrets_in_env_objects(item, secrets);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                if key == "env" {
                    if let serde_json::Value::Object(env) = value {
                        rewrite_secret_env_values_in_place(env, secrets);
                    }
                }
                rewrite_secrets_in_env_objects(value, secrets);
            }
        }
    }
}

fn extract_and_template_secret(
    root: &mut serde_json::Value,
    pointer: &str,
    secret_key: &str,
    secrets: &mut HashMap<String, String>,
) {
    let Some(value) = root.pointer_mut(pointer) else {
        return;
    };
    let Some(raw) = value.as_str().map(|s| s.trim()).filter(|s| !s.is_empty()) else {
        return;
    };

    secrets
        .entry(secret_key.to_string())
        .or_insert_with(|| raw.to_string());
    *value = serde_json::Value::String(format!("{{{{secret.{secret_key}}}}}"));
}

fn config_json_to_yaml_fragment(
    mut config_json: serde_json::Value,
    secrets: &mut HashMap<String, String>,
) -> Result<serde_yaml::Mapping> {
    extract_and_template_secret(
        &mut config_json,
        "/access_control/token",
        "VK_ACCESS_TOKEN",
        secrets,
    );
    extract_and_template_secret(
        &mut config_json,
        "/accessControl/token",
        "VK_ACCESS_TOKEN",
        secrets,
    );
    extract_and_template_secret(&mut config_json, "/github/pat", "GITHUB_PAT", secrets);
    extract_and_template_secret(
        &mut config_json,
        "/github/oauth_token",
        "GITHUB_OAUTH_TOKEN",
        secrets,
    );
    extract_and_template_secret(
        &mut config_json,
        "/github/oauthToken",
        "GITHUB_OAUTH_TOKEN",
        secrets,
    );

    let obj = config_json
        .as_object()
        .context("legacy config.json must be a JSON object")?;

    let allowed_keys: [&str; 21] = [
        "config_version",
        "theme",
        "executor_profile",
        "executor_profiles",
        "disclaimer_acknowledged",
        "onboarding_acknowledged",
        "notifications",
        "editor",
        "github",
        "workspace_dir",
        "last_app_version",
        "show_release_notes",
        "language",
        "git_branch_prefix",
        "git_no_verify",
        "showcases",
        "pr_auto_description_enabled",
        "pr_auto_description_prompt",
        "llman_claude_code_path",
        "diff_preview_guard",
        "access_control",
    ];

    let mut fragment = serde_yaml::Mapping::new();

    for key in allowed_keys {
        let Some(value) = obj.get(key) else {
            continue;
        };
        if value.is_null() {
            continue;
        }

        if key == "executor_profile" {
            let parsed = serde_json::from_value::<ExecutorProfileId>(value.clone());
            match parsed {
                Ok(profile_id) => {
                    if !matches!(
                        profile_id.executor,
                        BaseCodingAgent::ClaudeCode | BaseCodingAgent::Codex
                    ) {
                        eprintln!(
                            "warning: legacy config.json executor_profile '{}' is not supported by this build; skipping it",
                            profile_id
                        );
                        continue;
                    }
                }
                Err(err) => {
                    eprintln!(
                        "warning: legacy config.json executor_profile is invalid ({err}); skipping it"
                    );
                    continue;
                }
            }
        }

        // If legacy config.json includes executor_profiles, do a best-effort filter to supported
        // executors to avoid deserialization failures when default features are trimmed.
        let yaml_value = if key == "executor_profiles" {
            let mut filtered = value.clone();
            if let Some(executors) = filtered
                .get_mut("executors")
                .and_then(|v| v.as_object_mut())
            {
                let executor_keys = executors.keys().cloned().collect::<Vec<_>>();
                for executor_key in executor_keys {
                    if !is_supported_executor_key(&executor_key) {
                        executors.remove(&executor_key);
                    }
                }
            }
            rewrite_secrets_in_env_objects(&mut filtered, secrets);
            serde_yaml::to_value(filtered).context("Failed to convert executor_profiles to YAML")?
        } else {
            serde_yaml::to_value(value)
                .with_context(|| format!("Failed to convert {key} to YAML"))?
        };

        fragment.insert(serde_yaml::Value::String(key.to_string()), yaml_value);
    }

    Ok(fragment)
}

fn profiles_json_to_executor_profiles_yaml(
    mut profiles_json: serde_json::Value,
    secrets: &mut HashMap<String, String>,
) -> Result<serde_yaml::Value> {
    let executors = profiles_json
        .get_mut("executors")
        .context("legacy profiles.json missing top-level 'executors' key")?;
    let executors_obj = executors
        .as_object_mut()
        .context("legacy profiles.json 'executors' must be a JSON object")?;

    let keys = executors_obj.keys().cloned().collect::<Vec<_>>();
    let mut dropped = Vec::new();
    for key in keys {
        if !is_supported_executor_key(&key) {
            executors_obj.remove(&key);
            dropped.push(key);
        }
    }
    if !dropped.is_empty() {
        dropped.sort();
        eprintln!(
            "warning: dropping unsupported executors from legacy profiles.json: {}",
            dropped.join(", ")
        );
    }

    rewrite_secrets_in_env_objects(&mut profiles_json, secrets);

    let yaml =
        serde_yaml::to_value(profiles_json).context("Failed to convert profiles.json to YAML")?;
    Ok(yaml)
}

fn secret_env_to_string(tool_name: &str, vars: &HashMap<String, String>) -> String {
    let mut keys = vars.keys().cloned().collect::<Vec<_>>();
    keys.sort();

    let mut out = String::new();
    out.push_str(&format!(
        "# Generated by {tool_name} at {}\n",
        Utc::now().to_rfc3339()
    ));
    for key in keys {
        let value = vars.get(&key).expect("key exists");
        out.push_str(&format!("{key}={value}\n"));
    }
    out
}

fn validate_yaml_with_secret_env(yaml: &str, secret_env: Option<&str>) -> Result<()> {
    let validate_dir =
        std::env::temp_dir().join(format!("vk-asset-export-validate-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&validate_dir).context("Failed to create validation temp dir")?;
    let config_path = validate_dir.join("config.yaml");
    std::fs::write(&config_path, yaml).context("Failed to write validation config.yaml")?;
    if let Some(secret_env) = secret_env {
        let secret_path = validate_dir.join("secret.env");
        std::fs::write(&secret_path, secret_env)
            .context("Failed to write validation secret.env")?;
    }

    match config::try_load_config_from_file(&config_path) {
        Ok(_) => {
            let _ = std::fs::remove_dir_all(&validate_dir);
            Ok(())
        }
        Err(err) => Err(anyhow::anyhow!(
            "Exported YAML is not loadable by VK config loader (validation file: {}): {}",
            config_path.display(),
            err
        )),
    }
}

fn install_yaml_fragment_into_user_config(
    config_path: &Path,
    fragment: serde_yaml::Mapping,
    tool_name: &str,
    dry_run: bool,
) -> Result<()> {
    let slug = timestamp_slug();
    let config_dir = config_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(utils_core::vk_config_dir);

    let header = "# yaml-language-server: $schema=./config.schema.json\n";
    let generated = format!("# Updated by {tool_name} at {}\n", Utc::now().to_rfc3339());

    if !config_path.exists() {
        let mut new_contents = String::new();
        new_contents.push_str(header);
        new_contents.push_str(&generated);
        new_contents.push_str(
            &serde_yaml::to_string(&serde_yaml::Value::Mapping(fragment))
                .context("Failed to serialize config.yaml")?,
        );
        if !new_contents.ends_with('\n') {
            new_contents.push('\n');
        }

        if dry_run {
            println!(
                "dry-run: would write new config.yaml to {}",
                config_path.display()
            );
            return Ok(());
        }

        atomic_write_out(config_path, &new_contents)?;
        println!(
            "Installed legacy asset config into {}",
            config_path.display()
        );
        return Ok(());
    }

    let existing_raw = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;

    let existing_value: serde_yaml::Value = match serde_yaml::from_str(&existing_raw) {
        Ok(value) => value,
        Err(err) => {
            let fallback_path = config_dir.join(format!("asset-config.export.{slug}.yaml"));
            let contents = serde_yaml::to_string(&serde_yaml::Value::Mapping(fragment))
                .context("Failed to serialize export as YAML")?;
            if dry_run {
                println!(
                    "dry-run: existing config.yaml is invalid YAML; would write exported asset config to {}",
                    fallback_path.display()
                );
                return Ok(());
            }
            atomic_write_out(&fallback_path, &contents)?;
            println!(
                "Existing config.yaml is invalid YAML ({err}); wrote exported asset config to {}",
                fallback_path.display()
            );
            println!(
                "Merge {} into {} manually, then reload.",
                fallback_path.display(),
                config_path.display()
            );
            return Ok(());
        }
    };

    let mut existing_map = match existing_value {
        serde_yaml::Value::Null => serde_yaml::Mapping::new(),
        serde_yaml::Value::Mapping(map) => map,
        other => {
            let fallback_path = config_dir.join(format!("asset-config.export.{slug}.yaml"));
            let contents = serde_yaml::to_string(&serde_yaml::Value::Mapping(fragment))
                .context("Failed to serialize export as YAML")?;
            if dry_run {
                println!(
                    "dry-run: existing config.yaml is not a YAML mapping ({:?}); would write exported asset config to {}",
                    other,
                    fallback_path.display()
                );
                return Ok(());
            }
            atomic_write_out(&fallback_path, &contents)?;
            println!(
                "Existing config.yaml is not a YAML mapping; wrote exported asset config to {}",
                fallback_path.display()
            );
            println!(
                "Merge {} into {} manually, then reload.",
                fallback_path.display(),
                config_path.display()
            );
            return Ok(());
        }
    };

    for (k, v) in fragment {
        existing_map.insert(k, v);
    }

    let mut new_contents = String::new();
    new_contents.push_str(header);
    new_contents.push_str(&generated);
    new_contents.push_str(
        &serde_yaml::to_string(&serde_yaml::Value::Mapping(existing_map))
            .context("Failed to serialize merged config.yaml")?,
    );
    if !new_contents.ends_with('\n') {
        new_contents.push('\n');
    }

    let backup_path = config_dir.join(format!("config.yaml.bak.{slug}"));
    if dry_run {
        println!(
            "dry-run: would backup existing config.yaml to {}",
            backup_path.display()
        );
        println!(
            "dry-run: would update config.yaml in {}",
            config_path.display()
        );
        return Ok(());
    }

    atomic_write_out(&backup_path, &existing_raw)?;
    atomic_write_out(config_path, &new_contents)?;

    println!(
        "Updated config.yaml in {} (backup at {})",
        config_path.display(),
        backup_path.display()
    );
    Ok(())
}

fn install_secret_env(
    secret_env_path: &Path,
    new_secrets: HashMap<String, String>,
    tool_name: &str,
    dry_run: bool,
) -> Result<()> {
    if new_secrets.is_empty() {
        return Ok(());
    }

    let slug = timestamp_slug();
    let config_dir = secret_env_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(utils_core::vk_config_dir);

    let mut merged = new_secrets;
    let existing = load_secret_env_for_merge(secret_env_path)?;
    for (k, v) in existing {
        merged.insert(k, v);
    }

    let contents = secret_env_to_string(tool_name, &merged);

    if secret_env_path.exists() {
        let existing_raw = std::fs::read_to_string(secret_env_path)
            .with_context(|| format!("Failed to read {}", secret_env_path.display()))?;
        let backup_path = config_dir.join(format!("secret.env.bak.{slug}"));
        if dry_run {
            println!(
                "dry-run: would backup existing secret.env to {}",
                backup_path.display()
            );
            println!(
                "dry-run: would update secret.env in {}",
                secret_env_path.display()
            );
            return Ok(());
        }

        atomic_write_out(&backup_path, &existing_raw)?;
        atomic_write_out(secret_env_path, &contents)?;
    } else {
        if dry_run {
            println!(
                "dry-run: would write new secret.env to {}",
                secret_env_path.display()
            );
            return Ok(());
        }
        atomic_write_out(secret_env_path, &contents)?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(secret_env_path, std::fs::Permissions::from_mode(0o600));
    }

    Ok(())
}

pub async fn run_export_asset_config_yaml(args: ExportAssetConfigYamlArgs) -> Result<()> {
    eprintln!("{EXPORT_ASSET_CONFIG_YAML_DEPRECATION_WARNING}");

    let config_dir = utils_core::vk_config_dir();
    let config_path = utils_core::vk_config_yaml_path();
    let secret_env_path = utils_core::vk_secret_env_path();

    let legacy_config_json = asset_dir().join("config.json");
    let legacy_profiles_json = asset_dir().join("profiles.json");

    if args.print_paths {
        println!("asset_dir: {}", asset_dir().display());
        println!("legacy_config_json: {}", legacy_config_json.display());
        println!("legacy_profiles_json: {}", legacy_profiles_json.display());
        println!("config_dir: {}", config_dir.display());
        println!("config_yaml: {}", config_path.display());
        println!("secret_env: {}", secret_env_path.display());
        return Ok(());
    }

    if !args.install && args.out.is_none() {
        anyhow::bail!("Missing output mode: use --out <path> or --install (see --help)");
    }

    let mut fragment = serde_yaml::Mapping::new();
    let mut secrets = HashMap::<String, String>::new();

    if legacy_config_json.exists() {
        let raw = std::fs::read_to_string(&legacy_config_json)
            .with_context(|| format!("Failed to read {}", legacy_config_json.display()))?;
        let value = serde_json::from_str::<serde_json::Value>(&raw)
            .with_context(|| format!("Failed to parse {}", legacy_config_json.display()))?;
        let cfg_fragment = config_json_to_yaml_fragment(value, &mut secrets)?;
        for (k, v) in cfg_fragment {
            fragment.insert(k, v);
        }
    } else {
        eprintln!(
            "warning: legacy config.json not found at {}, skipping",
            legacy_config_json.display()
        );
    }

    if legacy_profiles_json.exists() {
        let raw = std::fs::read_to_string(&legacy_profiles_json)
            .with_context(|| format!("Failed to read {}", legacy_profiles_json.display()))?;
        let value = serde_json::from_str::<serde_json::Value>(&raw)
            .with_context(|| format!("Failed to parse {}", legacy_profiles_json.display()))?;
        let executor_profiles = profiles_json_to_executor_profiles_yaml(value, &mut secrets)?;
        fragment.insert(
            serde_yaml::Value::String("executor_profiles".to_string()),
            executor_profiles,
        );
    } else {
        eprintln!(
            "warning: legacy profiles.json not found at {}, skipping",
            legacy_profiles_json.display()
        );
    }

    let fragment_yaml = serde_yaml::to_string(&serde_yaml::Value::Mapping(fragment.clone()))
        .context("Failed to serialize export as YAML")?;

    let validation_secret_env = if secrets.is_empty() {
        None
    } else {
        Some(secret_env_to_string("export-asset-config-yaml", &secrets))
    };
    validate_yaml_with_secret_env(&fragment_yaml, validation_secret_env.as_deref())?;

    if args.install {
        // Merge into current config.yaml + secret.env and validate the merged result before writing.
        let existing_raw =
            std::fs::read_to_string(&config_path).unwrap_or_else(|_| "{}".to_string());
        let existing_value: serde_yaml::Value = serde_yaml::from_str(&existing_raw)
            .unwrap_or(serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
        let mut existing_map = match existing_value {
            serde_yaml::Value::Null => serde_yaml::Mapping::new(),
            serde_yaml::Value::Mapping(map) => map,
            _ => serde_yaml::Mapping::new(),
        };
        for (k, v) in fragment.clone() {
            existing_map.insert(k, v);
        }
        let header = "# yaml-language-server: $schema=./config.schema.json\n";
        let generated = format!(
            "# Updated by export-asset-config-yaml at {}\n",
            Utc::now().to_rfc3339()
        );
        let mut merged_yaml = String::new();
        merged_yaml.push_str(header);
        merged_yaml.push_str(&generated);
        merged_yaml.push_str(
            &serde_yaml::to_string(&serde_yaml::Value::Mapping(existing_map))
                .context("Failed to serialize merged config.yaml")?,
        );
        if !merged_yaml.ends_with('\n') {
            merged_yaml.push('\n');
        }

        let merged_secret_env = if secrets.is_empty() {
            None
        } else {
            let existing_secrets = load_secret_env_for_merge(&secret_env_path)?;
            let mut merged = secrets.clone();
            for (k, v) in existing_secrets {
                merged.insert(k, v);
            }
            Some(secret_env_to_string("export-asset-config-yaml", &merged))
        };

        validate_yaml_with_secret_env(&merged_yaml, merged_secret_env.as_deref())?;

        // Write secret.env first to avoid transient reload failures when config.yaml references
        // `{{secret.*}}` placeholders.
        install_secret_env(
            &secret_env_path,
            secrets,
            "export-asset-config-yaml",
            args.dry_run,
        )?;
        install_yaml_fragment_into_user_config(
            &config_path,
            fragment,
            "export-asset-config-yaml",
            args.dry_run,
        )?;

        return Ok(());
    }

    let out_path = args.out.expect("checked above");
    if out_path.as_os_str() == "-" {
        print!("{fragment_yaml}");
        if !secrets.is_empty() {
            eprintln!(
                "note: export references secret.env keys (run with --install to write secret.env): {}",
                secrets.keys().cloned().collect::<Vec<_>>().join(", ")
            );
        }
        return Ok(());
    }

    if args.dry_run {
        println!("dry-run: would write export to {}", out_path.display());
        return Ok(());
    }

    atomic_write_out(&out_path, &fragment_yaml)?;
    println!("Exported asset config YAML to {}", out_path.display());
    if !secrets.is_empty() {
        eprintln!(
            "note: export references secret.env keys; run with --install to write secret.env"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};

    use super::*;

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
        let _guard = env_lock().lock().unwrap_or_else(|err| err.into_inner());

        let temp_root =
            std::env::temp_dir().join(format!("vk-export-test-{}", uuid::Uuid::new_v4()));
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
        std::fs::write(&config_path, &yaml).unwrap();

        let loaded = config::try_load_config_from_file(&config_path).expect("load config");
        assert_eq!(loaded.projects.len(), 1);
        let exported_project = &loaded.projects[0];
        assert_eq!(exported_project.id, Some(project_id));
        assert_eq!(exported_project.repos.len(), 1);
    }

    #[test]
    fn install_writes_new_config_yaml() {
        let temp_root =
            std::env::temp_dir().join(format!("vk-export-install-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let repo_path = temp_root.join("repo");
        std::fs::create_dir_all(&repo_path).unwrap();

        let project_id = uuid::Uuid::new_v4();
        let yaml = format!(
            "projects:\n  - id: {project_id}\n    name: \"My Project\"\n    repos:\n      - path: \"{}\"\n",
            repo_path.to_string_lossy()
        );

        let config_path = temp_root.join("config.yaml");
        install_projects_into_user_config(&config_path, &yaml, false).expect("install");

        let raw = std::fs::read_to_string(&config_path).expect("read config.yaml");
        assert!(raw.contains("yaml-language-server"));

        let loaded = config::try_load_config_from_file(&config_path).expect("load config");
        assert_eq!(loaded.projects.len(), 1);
        assert_eq!(loaded.projects[0].id, Some(project_id));
    }

    #[test]
    fn install_merges_into_existing_mapping() {
        let temp_root =
            std::env::temp_dir().join(format!("vk-export-merge-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let repo_path = temp_root.join("repo");
        std::fs::create_dir_all(&repo_path).unwrap();

        let config_path = temp_root.join("config.yaml");
        std::fs::write(
            &config_path,
            "github:\n  pat: \"placeholder\"\nprojects:\n  - id: 00000000-0000-0000-0000-000000000000\n    name: \"Old\"\n    repos:\n      - path: \"/tmp/old\"\n",
        )
        .unwrap();

        let project_id = uuid::Uuid::new_v4();
        let yaml = format!(
            "projects:\n  - id: {project_id}\n    name: \"New\"\n    repos:\n      - path: \"{}\"\n",
            repo_path.to_string_lossy()
        );

        install_projects_into_user_config(&config_path, &yaml, false).expect("install merge");

        let merged: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        let map = merged.as_mapping().expect("mapping");
        assert!(map.contains_key(&serde_yaml::Value::String("github".to_string())));
        assert!(map.contains_key(&serde_yaml::Value::String("projects".to_string())));
    }

    #[test]
    fn install_does_not_overwrite_invalid_yaml() {
        let temp_root = std::env::temp_dir().join(format!(
            "vk-export-invalid-yaml-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_root).unwrap();

        let repo_path = temp_root.join("repo");
        std::fs::create_dir_all(&repo_path).unwrap();

        let config_path = temp_root.join("config.yaml");
        let invalid_yaml = "{unclosed: [1, 2}\n";
        std::fs::write(&config_path, invalid_yaml).unwrap();

        let project_id = uuid::Uuid::new_v4();
        let yaml = format!(
            "projects:\n  - id: {project_id}\n    name: \"New\"\n    repos:\n      - path: \"{}\"\n",
            repo_path.to_string_lossy()
        );

        install_projects_into_user_config(&config_path, &yaml, false).expect("install invalid");

        assert_eq!(std::fs::read_to_string(&config_path).unwrap(), invalid_yaml);

        let exported_files = std::fs::read_dir(&temp_root)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.starts_with("projects.db-export."))
            .collect::<Vec<_>>();
        assert_eq!(exported_files.len(), 1);

        let exported_path = temp_root.join(&exported_files[0]);
        let loaded = config::try_load_config_from_file(&exported_path).expect("load exported yaml");
        assert_eq!(loaded.projects.len(), 1);
        assert_eq!(loaded.projects[0].id, Some(project_id));
    }

    #[test]
    fn install_dry_run_does_not_write() {
        let temp_root =
            std::env::temp_dir().join(format!("vk-export-dry-run-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let repo_path = temp_root.join("repo");
        std::fs::create_dir_all(&repo_path).unwrap();

        let project_id = uuid::Uuid::new_v4();
        let yaml = format!(
            "projects:\n  - id: {project_id}\n    name: \"My Project\"\n    repos:\n      - path: \"{}\"\n",
            repo_path.to_string_lossy()
        );

        let config_path = temp_root.join("config.yaml");
        install_projects_into_user_config(&config_path, &yaml, true).expect("dry-run");
        assert!(!config_path.exists());
    }

    struct AssetEnvGuard {
        prev_vk_config_dir: Option<String>,
        prev_asset_dir: Option<String>,
    }

    impl AssetEnvGuard {
        fn new(vk_config_dir: &Path, asset_dir: &Path) -> Self {
            let prev_vk_config_dir = std::env::var("VK_CONFIG_DIR").ok();
            let prev_asset_dir = std::env::var("VIBE_ASSET_DIR").ok();

            unsafe {
                std::env::set_var("VK_CONFIG_DIR", vk_config_dir.as_os_str());
                std::env::set_var("VIBE_ASSET_DIR", asset_dir.as_os_str());
            }

            Self {
                prev_vk_config_dir,
                prev_asset_dir,
            }
        }
    }

    impl Drop for AssetEnvGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.prev_vk_config_dir {
                    Some(value) => std::env::set_var("VK_CONFIG_DIR", value),
                    None => std::env::remove_var("VK_CONFIG_DIR"),
                }
                match &self.prev_asset_dir {
                    Some(value) => std::env::set_var("VIBE_ASSET_DIR", value),
                    None => std::env::remove_var("VIBE_ASSET_DIR"),
                }
            }
        }
    }

    #[tokio::test]
    async fn export_asset_config_installs_yaml_and_secret_env() {
        let _guard = env_lock().lock().unwrap_or_else(|err| err.into_inner());

        let temp_root = std::env::temp_dir().join(format!(
            "vk-asset-export-install-test-{}",
            uuid::Uuid::new_v4()
        ));
        let asset_root = temp_root.join("asset");
        let config_root = temp_root.join("config");
        std::fs::create_dir_all(&asset_root).unwrap();
        std::fs::create_dir_all(&config_root).unwrap();

        let _env = AssetEnvGuard::new(&config_root, &asset_root);

        let legacy_config = serde_json::json!({
          "executor_profile": { "executor": "CODEX", "variant": "XHIGH" },
          "language": "ZH_HANS",
          "theme": "SYSTEM",
          "notifications": { "sound_enabled": false, "push_enabled": true, "sound_file": "COW_MOOING" },
          "editor": { "editor_type": "NONE", "custom_command": null, "remote_ssh_host": null, "remote_ssh_user": null },
          "access_control": { "mode": "TOKEN", "token": "tok_test_123", "allow_localhost_bypass": true },
          "github": { "pat": "ghp_test_123" }
        });
        std::fs::write(
            asset_root.join("config.json"),
            serde_json::to_string_pretty(&legacy_config).unwrap(),
        )
        .unwrap();

        let legacy_profiles = serde_json::json!({
          "executors": {
            "CLAUDE_CODE": {
              "LLMAN_ANYROUTER": {
                "CLAUDE_CODE": {
                  "env": {
                    "ANTHROPIC_BASE_URL": "https://example.com",
                    "ANTHROPIC_AUTH_TOKEN": "sk-test-anthropic"
                  }
                }
              }
            },
            "FAKE_AGENT": {
              "DEFAULT": {
                "FAKE_AGENT": {
                  "env": {
                    "FAKE_TOKEN": "should-not-export"
                  }
                }
              }
            }
          }
        });
        std::fs::write(
            asset_root.join("profiles.json"),
            serde_json::to_string_pretty(&legacy_profiles).unwrap(),
        )
        .unwrap();

        run_export_asset_config_yaml(ExportAssetConfigYamlArgs {
            out: None,
            install: true,
            dry_run: false,
            print_paths: false,
        })
        .await
        .expect("run export-asset-config-yaml --install");

        let config_path = config_root.join("config.yaml");
        let secret_path = config_root.join("secret.env");

        let config_raw = std::fs::read_to_string(&config_path).expect("read config.yaml");
        assert!(config_raw.contains("executor_profile"));
        assert!(config_raw.contains("executor_profiles"));
        assert!(config_raw.contains("{{secret.VK_ACCESS_TOKEN}}"));
        assert!(config_raw.contains("{{secret.GITHUB_PAT}}"));
        assert!(config_raw.contains("{{secret.ANTHROPIC_AUTH_TOKEN}}"));
        assert!(!config_raw.contains("tok_test_123"));
        assert!(!config_raw.contains("ghp_test_123"));
        assert!(!config_raw.contains("sk-test-anthropic"));
        assert!(!config_raw.contains("FAKE_AGENT"));

        let secret_raw = std::fs::read_to_string(&secret_path).expect("read secret.env");
        assert!(secret_raw.contains("VK_ACCESS_TOKEN=tok_test_123"));
        assert!(secret_raw.contains("GITHUB_PAT=ghp_test_123"));
        assert!(secret_raw.contains("ANTHROPIC_AUTH_TOKEN=sk-test-anthropic"));
        assert!(!secret_raw.contains("FAKE_TOKEN"));

        let loaded = config::try_load_config_from_file(&config_path).expect("load config");
        assert_eq!(
            loaded.executor_profile,
            ExecutorProfileId::with_variant(BaseCodingAgent::Codex, "XHIGH".to_string())
        );
    }
}
