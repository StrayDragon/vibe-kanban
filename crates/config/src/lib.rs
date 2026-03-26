use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use thiserror::Error;

pub mod cache_budget;
pub mod editor;
mod schema;
mod yaml_schema;

pub use editor::{EditorConfig, EditorOpenError, EditorType};
pub use schema::{
    AccessControlConfig, AccessControlMode, CURRENT_CONFIG_VERSION, Config, DiffPreviewGuardPreset,
    GitHubConfig, NotificationConfig, ProjectConfig, ProjectMcpExecutorPolicyMode,
    ProjectsFile,
    ProjectRepoConfig, ShowcaseState, SoundFile, ThemeMode, UiLanguage,
    WorkspaceLifecycleHookConfig, WorkspaceLifecycleHookFailurePolicy,
    WorkspaceLifecycleHookRunMode,
};
pub use yaml_schema::{
    ConfigSchemaError, generate_config_schema_json, generate_projects_schema_json,
    write_config_schema_json, write_projects_schema_json,
};

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
    #[error("Validation error: {0}")]
    ValidationError(String),
}

fn load_secret_env(
    secret_env_path: &std::path::Path,
) -> Result<HashMap<String, String>, ConfigError> {
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
            return Err(ConfigError::ValidationError(format!(
                "Invalid secret.env line {}: expected KEY=VALUE",
                idx + 1
            )));
        };

        let key = key.trim();
        if key.is_empty() {
            return Err(ConfigError::ValidationError(format!(
                "Invalid secret.env line {}: empty key",
                idx + 1
            )));
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

struct TemplateEnv {
    secret: HashMap<String, String>,
}

impl TemplateEnv {
    fn lookup_secret(&self, name: &str) -> Option<String> {
        self.secret.get(name).cloned()
    }

    fn lookup_env(&self, name: &str) -> Option<String> {
        self.secret
            .get(name)
            .cloned()
            .or_else(|| std::env::var(name).ok())
    }
}

fn resolve_templates_in_string(input: &str, env: &TemplateEnv) -> Result<String, ConfigError> {
    if !input.contains("{{") {
        return Ok(input.to_string());
    }

    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some(start_rel) = input[cursor..].find("{{") {
        let start = cursor + start_rel;
        output.push_str(&input[cursor..start]);

        let after = start + 2;
        let Some(end_rel) = input[after..].find("}}") else {
            return Err(ConfigError::ValidationError(
                "Invalid template: missing '}}'".to_string(),
            ));
        };
        let end = after + end_rel;
        let inner = &input[after..end];

        let (expr, default) = match inner.split_once(":-") {
            Some((expr, default)) => (expr.trim(), Some(default)),
            None => (inner.trim(), None),
        };

        if expr.is_empty() {
            return Err(ConfigError::ValidationError(
                "Invalid template: empty expression".to_string(),
            ));
        }

        let Some((namespace, name)) = expr.split_once('.') else {
            return Err(ConfigError::ValidationError(format!(
                "Invalid template: expected <namespace>.<name>, got '{expr}'"
            )));
        };
        let namespace = namespace.trim();
        let name = name.trim();
        if name.is_empty() {
            return Err(ConfigError::ValidationError(
                "Invalid template: empty variable name".to_string(),
            ));
        }

        let resolved = match namespace {
            // env placeholders use secret.env precedence over system env.
            "env" => env.lookup_env(name),
            "secret" => env.lookup_secret(name),
            _ => {
                return Err(ConfigError::ValidationError(format!(
                    "Invalid template: unknown namespace '{namespace}' (expected 'env' or 'secret')"
                )));
            }
        };

        let resolved = match resolved {
            Some(value) => value,
            None => match default {
                Some(default) => default.to_string(),
                None => {
                    return Err(ConfigError::ValidationError(format!(
                        "Missing required environment variable: {name}"
                    )));
                }
            },
        };

        output.push_str(&resolved);
        cursor = end + 2;
    }

    output.push_str(&input[cursor..]);
    Ok(output)
}

fn resolve_yaml_templates(
    value: &mut serde_yaml::Value,
    env: &TemplateEnv,
) -> Result<(), ConfigError> {
    match value {
        serde_yaml::Value::Null | serde_yaml::Value::Bool(_) | serde_yaml::Value::Number(_) => {
            Ok(())
        }
        serde_yaml::Value::String(s) => {
            let resolved = resolve_templates_in_string(s, env)?;
            *s = resolved;
            Ok(())
        }
        serde_yaml::Value::Sequence(items) => {
            for item in items {
                resolve_yaml_templates(item, env)?;
            }
            Ok(())
        }
        serde_yaml::Value::Mapping(map) => {
            for (_k, v) in map.iter_mut() {
                resolve_yaml_templates(v, env)?;
            }
            Ok(())
        }
        serde_yaml::Value::Tagged(tagged) => resolve_yaml_templates(&mut tagged.value, env),
    }
}

fn read_optional_file(path: &Path) -> Result<Option<String>, std::io::Error> {
    match std::fs::read_to_string(path) {
        Ok(raw) => Ok(Some(raw)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

fn yaml_from_raw_with_templates(
    raw: &str,
    env: &TemplateEnv,
) -> Result<serde_yaml::Value, ConfigError> {
    let mut value = serde_yaml::from_str::<serde_yaml::Value>(raw)?;
    resolve_yaml_templates(&mut value, env)?;
    Ok(value)
}

pub fn try_load_config_from_file(config_path: &PathBuf) -> Result<Config, ConfigError> {
    let config_dir = config_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(utils_core::vk_config_dir);
    let secret_env_path = config_dir.join("secret.env");
    let secret = load_secret_env(&secret_env_path)?;
    let env = TemplateEnv { secret };

    let projects_path = config_dir.join("projects.yaml");
    let projects_dir = config_dir.join("projects.d");

    let mut projects_extra_paths = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let is_yaml = matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("yaml") | Some("yml")
            );
            if is_yaml {
                projects_extra_paths.push(path);
            }
        }
    }
    projects_extra_paths.sort();

    let (raw, projects_raw, projects_extra_raws) = std::thread::scope(|scope| {
        let config_handle = scope.spawn(|| read_optional_file(config_path));
        let projects_handle = scope.spawn(|| read_optional_file(&projects_path));
        let extra_handles = projects_extra_paths
            .iter()
            .map(|path| scope.spawn(move || read_optional_file(path)))
            .collect::<Vec<_>>();

        let config_raw = config_handle
            .join()
            .expect("config file read thread panicked");
        let projects_raw = projects_handle
            .join()
            .expect("projects.yaml read thread panicked");
        let extra_raws = extra_handles
            .into_iter()
            .map(|handle| handle.join().expect("projects.d read thread panicked"))
            .collect::<Vec<_>>();

        (config_raw, projects_raw, extra_raws)
    });

    let raw = raw?;
    let projects_raw = projects_raw?;
    let projects_extra_raws = projects_extra_raws
        .into_iter()
        .collect::<Result<Vec<_>, std::io::Error>>()?;

    let mut config = match raw.as_deref() {
        Some(raw) => {
            let value = yaml_from_raw_with_templates(raw, &env)?;
            serde_yaml::from_value::<Config>(value)?
        }
        None => Config::default(),
    };

    // If projects.yaml (or projects.d/*.yaml) exist, they become the canonical source of
    // projects configuration. Otherwise fall back to inline `projects` in config.yaml.
    let mut projects_override = Vec::new();
    let mut has_projects_override = false;

    if let Some(raw) = projects_raw.as_deref() {
        let value = yaml_from_raw_with_templates(raw, &env)?;
        let file = serde_yaml::from_value::<ProjectsFile>(value)?;
        projects_override.extend(file.projects);
        has_projects_override = true;
    }

    for raw in projects_extra_raws.into_iter().flatten() {
        let value = yaml_from_raw_with_templates(&raw, &env)?;
        let file = serde_yaml::from_value::<ProjectsFile>(value)?;
        projects_override.extend(file.projects);
        has_projects_override = true;
    }

    if has_projects_override {
        config.projects = projects_override;
    }

    let config = config.normalized();

    let profiles = executors::profile::ExecutorConfigs::from_defaults_merged_with_overrides(
        config.executor_profiles.as_ref(),
    )
    .map_err(|err| ConfigError::ValidationError(err.to_string()))?;
    profiles
        .require_coding_agent(&config.executor_profile)
        .map_err(|err| ConfigError::ValidationError(err.to_string()))?;

    config
        .validate_projects(&profiles)
        .map_err(ConfigError::ValidationError)?;

    Ok(config)
}

pub fn reload_config_keep_last_known_good(
    current: &Config,
    config_path: &PathBuf,
) -> (Config, Option<String>) {
    match try_load_config_from_file(config_path) {
        Ok(config) => (config, None),
        Err(err) => (current.clone(), Some(err.to_string())),
    }
}

/// Will always return config, falling back to defaults on missing/invalid files.
pub async fn load_config_from_file(config_path: &PathBuf) -> Config {
    match try_load_config_from_file(config_path) {
        Ok(config) => config,
        Err(err) => {
            tracing::warn!("Failed to load config: {}", err);
            Config::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use super::*;

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn env_lock() -> &'static Mutex<()> {
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    fn write_file(path: &std::path::Path, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create dir");
        }
        std::fs::write(path, contents).expect("write file");
    }

    #[test]
    fn secret_env_overrides_system_env() {
        let _guard = env_lock().lock().unwrap();

        let prev = std::env::var_os("GITHUB_PAT");
        unsafe {
            std::env::set_var("GITHUB_PAT", "from_system");
        }

        let dir = std::env::temp_dir().join(format!("vk-config-test-{}", uuid::Uuid::new_v4()));
        let config_path = dir.join("config.yaml");
        let secret_path = dir.join("secret.env");

        write_file(
            &config_path,
            r#"
github:
  pat: "{{env.GITHUB_PAT}}"
"#,
        );
        write_file(&secret_path, "GITHUB_PAT=from_secret\n");

        let loaded = try_load_config_from_file(&config_path).expect("load config");
        assert_eq!(loaded.github.pat.as_deref(), Some("from_secret"));

        match prev {
            Some(value) => unsafe { std::env::set_var("GITHUB_PAT", value) },
            None => unsafe { std::env::remove_var("GITHUB_PAT") },
        }
    }

    #[test]
    fn template_default_is_used_when_missing() {
        let _guard = env_lock().lock().unwrap();

        let prev = std::env::var_os("GITHUB_PAT");
        unsafe {
            std::env::remove_var("GITHUB_PAT");
        }

        let dir = std::env::temp_dir().join(format!("vk-config-test-{}", uuid::Uuid::new_v4()));
        let config_path = dir.join("config.yaml");

        write_file(
            &config_path,
            r#"
github:
  pat: "{{env.GITHUB_PAT:-fallback}}"
"#,
        );

        let loaded = try_load_config_from_file(&config_path).expect("load config");
        assert_eq!(loaded.github.pat.as_deref(), Some("fallback"));

        match prev {
            Some(value) => unsafe { std::env::set_var("GITHUB_PAT", value) },
            None => unsafe { std::env::remove_var("GITHUB_PAT") },
        }
    }

    #[test]
    fn missing_template_var_without_default_is_validation_error() {
        let _guard = env_lock().lock().unwrap();

        let prev = std::env::var_os("GITHUB_PAT");
        unsafe {
            std::env::remove_var("GITHUB_PAT");
        }

        let dir = std::env::temp_dir().join(format!("vk-config-test-{}", uuid::Uuid::new_v4()));
        let config_path = dir.join("config.yaml");

        write_file(
            &config_path,
            r#"
github:
  pat: "{{env.GITHUB_PAT}}"
"#,
        );

        let err = try_load_config_from_file(&config_path).expect_err("expected error");
        match err {
            ConfigError::ValidationError(message) => {
                assert!(message.contains("GITHUB_PAT"));
            }
            other => panic!("unexpected error: {other:?}"),
        }

        match prev {
            Some(value) => unsafe { std::env::set_var("GITHUB_PAT", value) },
            None => unsafe { std::env::remove_var("GITHUB_PAT") },
        }
    }

    #[test]
    fn reload_keeps_last_known_good_on_error() {
        let _guard = env_lock().lock().unwrap();

        let prev = std::env::var_os("GITHUB_PAT");
        unsafe {
            std::env::remove_var("GITHUB_PAT");
        }

        let mut current = Config::default();
        current.github.pat = Some("old".to_string());

        let dir = std::env::temp_dir().join(format!("vk-config-test-{}", uuid::Uuid::new_v4()));
        let config_path = dir.join("config.yaml");
        write_file(
            &config_path,
            r#"
github:
  pat: "{{env.GITHUB_PAT}}"
"#,
        );

        let (reloaded, err) = reload_config_keep_last_known_good(&current, &config_path);
        assert_eq!(reloaded.github.pat.as_deref(), Some("old"));
        assert!(err.unwrap_or_default().contains("GITHUB_PAT"));

        match prev {
            Some(value) => unsafe { std::env::set_var("GITHUB_PAT", value) },
            None => unsafe { std::env::remove_var("GITHUB_PAT") },
        }
    }

    #[test]
    fn project_missing_id_is_validation_error() {
        let dir = std::env::temp_dir().join(format!("vk-config-test-{}", uuid::Uuid::new_v4()));
        let config_path = dir.join("config.yaml");

        write_file(
            &config_path,
            r#"
projects:
  - name: test
    repos:
      - path: /tmp/test-repo
"#,
        );

        let err = try_load_config_from_file(&config_path).expect_err("expected error");
        match err {
            ConfigError::ValidationError(message) => {
                assert!(message.contains("projects[0]"));
                assert!(message.to_lowercase().contains("missing id"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn duplicate_project_ids_are_rejected() {
        let dir = std::env::temp_dir().join(format!("vk-config-test-{}", uuid::Uuid::new_v4()));
        let config_path = dir.join("config.yaml");

        write_file(
            &config_path,
            r#"
projects:
  - id: 11111111-1111-1111-1111-111111111111
    name: a
    repos:
      - path: /tmp/repo-a
  - id: 11111111-1111-1111-1111-111111111111
    name: b
    repos:
      - path: /tmp/repo-b
"#,
        );

        let err = try_load_config_from_file(&config_path).expect_err("expected error");
        match err {
            ConfigError::ValidationError(message) => {
                assert!(message.contains("Duplicate project id"));
                assert!(message.contains("11111111-1111-1111-1111-111111111111"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn projects_yaml_overrides_inline_projects() {
        let dir = std::env::temp_dir().join(format!(
            "vk-config-projects-override-test-{}",
            uuid::Uuid::new_v4()
        ));

        let repo_a = dir.join("repo-a");
        let repo_b = dir.join("repo-b");
        std::fs::create_dir_all(&repo_a).unwrap();
        std::fs::create_dir_all(&repo_b).unwrap();

        let config_path = dir.join("config.yaml");
        let projects_path = dir.join("projects.yaml");

        let id_a = uuid::Uuid::new_v4();
        let id_b = uuid::Uuid::new_v4();

        write_file(
            &config_path,
            &format!(
                r#"
projects:
  - id: {id_a}
    name: A
    repos:
      - path: "{repo_a}"
"#,
                repo_a = repo_a.to_string_lossy()
            ),
        );
        write_file(
            &projects_path,
            &format!(
                r#"
projects:
  - id: {id_b}
    name: B
    repos:
      - path: "{repo_b}"
"#,
                repo_b = repo_b.to_string_lossy()
            ),
        );

        let loaded = try_load_config_from_file(&config_path).expect("load config");
        assert_eq!(loaded.projects.len(), 1);
        assert_eq!(loaded.projects[0].id, Some(id_b));
    }

    #[test]
    fn projects_dir_files_are_loaded_and_merged() {
        let dir = std::env::temp_dir().join(format!(
            "vk-config-projects-dir-test-{}",
            uuid::Uuid::new_v4()
        ));

        let repo_a = dir.join("repo-a");
        let repo_b = dir.join("repo-b");
        std::fs::create_dir_all(&repo_a).unwrap();
        std::fs::create_dir_all(&repo_b).unwrap();

        let config_path = dir.join("config.yaml");
        write_file(&config_path, "{}\n");

        let projects_dir = dir.join("projects.d");
        std::fs::create_dir_all(&projects_dir).unwrap();

        let id_a = uuid::Uuid::new_v4();
        let id_b = uuid::Uuid::new_v4();

        write_file(
            &projects_dir.join("a.yaml"),
            &format!(
                r#"
projects:
  - id: {id_a}
    name: A
    repos:
      - path: "{repo_a}"
"#,
                repo_a = repo_a.to_string_lossy()
            ),
        );
        write_file(
            &projects_dir.join("b.yaml"),
            &format!(
                r#"
projects:
  - id: {id_b}
    name: B
    repos:
      - path: "{repo_b}"
"#,
                repo_b = repo_b.to_string_lossy()
            ),
        );

        let loaded = try_load_config_from_file(&config_path).expect("load config");
        assert_eq!(loaded.projects.len(), 2);
    }
}
