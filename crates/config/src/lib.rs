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

pub fn parse_secret_env_contents(raw: &str) -> Result<HashMap<String, String>, ConfigError> {
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

pub fn try_load_secret_env(secret_env_path: &Path) -> Result<HashMap<String, String>, ConfigError> {
    load_secret_env(secret_env_path)
}

fn load_secret_env(secret_env_path: &std::path::Path) -> Result<HashMap<String, String>, ConfigError>
{
    let metadata = match std::fs::symlink_metadata(secret_env_path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(err) => return Err(err.into()),
    };

    if metadata.file_type().is_symlink() {
        return Err(ConfigError::ValidationError(format!(
            "Invalid secret.env: must not be a symlink ({})",
            secret_env_path.display()
        )));
    }
    if !metadata.file_type().is_file() {
        return Err(ConfigError::ValidationError(format!(
            "Invalid secret.env: must be a regular file ({})",
            secret_env_path.display()
        )));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let mode = metadata.permissions().mode() & 0o777;
        if (mode & 0o077) != 0 {
            return Err(ConfigError::ValidationError(format!(
                "Insecure permissions on secret.env (mode {mode:03o}). Expected 0600. Fix with: chmod 600 {}",
                secret_env_path.display()
            )));
        }

        let uid = metadata.uid();
        // SAFETY: libc call has no side effects.
        let euid = unsafe { libc::geteuid() };
        // If running as root, require a root-owned secret.env. This prevents accidentally loading
        // secrets from an untrusted user-owned file in a shared directory.
        if euid == 0 {
            if uid != 0 {
                return Err(ConfigError::ValidationError(format!(
                    "Insecure ownership on secret.env (uid {uid}). Expected uid 0 (root). Fix with: chown root:root {}",
                    secret_env_path.display()
                )));
            }
        } else if uid != euid {
            return Err(ConfigError::ValidationError(format!(
                "Insecure ownership on secret.env (uid {uid}). Expected uid {euid}. Fix with: chown {euid} {}",
                secret_env_path.display()
            )));
        }
    }

    let raw = std::fs::read_to_string(secret_env_path)?;
    parse_secret_env_contents(&raw)
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct ConfigDiskInputs {
    secret_env: HashMap<String, String>,
    config_raw: Option<String>,
    projects_raw: Option<String>,
    projects_extra: Vec<(PathBuf, Option<String>)>,
}

fn list_projects_extra_paths(projects_dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(entries) = std::fs::read_dir(projects_dir) {
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
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths
}

fn read_config_disk_inputs_once(
    config_path: &PathBuf,
    include_secret_env: bool,
) -> Result<ConfigDiskInputs, ConfigError> {
    let config_dir = config_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(utils_core::vk_config_dir);

    let secret_env_path = config_dir.join("secret.env");
    let secret_env = if include_secret_env {
        load_secret_env(&secret_env_path)?
    } else {
        HashMap::new()
    };

    let projects_path = config_dir.join("projects.yaml");
    let projects_dir = config_dir.join("projects.d");

    let projects_extra_paths = list_projects_extra_paths(&projects_dir);

    let config_raw = read_optional_file(config_path)?;
    let projects_raw = read_optional_file(&projects_path)?;

    let mut projects_extra = Vec::with_capacity(projects_extra_paths.len());
    for path in projects_extra_paths {
        let raw = read_optional_file(&path)?;
        projects_extra.push((path, raw));
    }

    Ok(ConfigDiskInputs {
        secret_env,
        config_raw,
        projects_raw,
        projects_extra,
    })
}

fn read_stable_with_retries<T, F>(mut read_once: F) -> Result<T, ConfigError>
where
    T: PartialEq,
    F: FnMut() -> Result<T, ConfigError>,
{
    const MAX_ATTEMPTS: usize = 3;

    for _ in 0..MAX_ATTEMPTS {
        let first = read_once()?;
        let second = read_once()?;
        if first == second {
            return Ok(first);
        }
    }

    Err(ConfigError::ValidationError(
        "Config files changed during load. Please retry.".to_string(),
    ))
}

fn read_config_disk_inputs_stable(
    config_path: &PathBuf,
    include_secret_env: bool,
) -> Result<ConfigDiskInputs, ConfigError> {
    read_stable_with_retries(|| read_config_disk_inputs_once(config_path, include_secret_env))
}

fn yaml_from_raw_with_templates(
    raw: &str,
    env: &TemplateEnv,
) -> Result<serde_yaml::Value, ConfigError> {
    let mut value = serde_yaml::from_str::<serde_yaml::Value>(raw)?;
    resolve_yaml_templates(&mut value, env)?;
    Ok(value)
}

/// Load config.yaml + optional projects.yaml/projects.d without resolving `{{secret.*}}`/`{{env.*}}`
/// templates. This is intended for *display* and API responses to avoid leaking expanded secrets.
pub fn try_load_public_config_from_file(config_path: &PathBuf) -> Result<Config, ConfigError> {
    let disk = read_config_disk_inputs_stable(config_path, false)?;

    let mut config = match disk.config_raw.as_deref() {
        Some(raw) => serde_yaml::from_str::<Config>(raw)?,
        None => Config::default(),
    };

    // If projects.yaml (or projects.d/*.yaml) exist, they become the canonical source of
    // projects configuration. Otherwise fall back to inline `projects` in config.yaml.
    let mut projects_override = Vec::new();
    let mut has_projects_override = false;

    if let Some(raw) = disk.projects_raw.as_deref() {
        let file = serde_yaml::from_str::<ProjectsFile>(raw)?;
        projects_override.extend(file.projects);
        has_projects_override = true;
    }

    for (_path, raw) in disk.projects_extra.into_iter() {
        let Some(raw) = raw else { continue };
        let file = serde_yaml::from_str::<ProjectsFile>(&raw)?;
        projects_override.extend(file.projects);
        has_projects_override = true;
    }

    if has_projects_override {
        config.projects = projects_override;
    }

    let config = config.normalized();
    config
        .validate_config_version()
        .map_err(ConfigError::ValidationError)?;
    Ok(config)
}

pub fn try_load_config_from_file(config_path: &PathBuf) -> Result<Config, ConfigError> {
    let disk = read_config_disk_inputs_stable(config_path, true)?;
    let env = TemplateEnv {
        secret: disk.secret_env,
    };

    let mut config = match disk.config_raw.as_deref() {
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

    if let Some(raw) = disk.projects_raw.as_deref() {
        let value = yaml_from_raw_with_templates(raw, &env)?;
        let file = serde_yaml::from_value::<ProjectsFile>(value)?;
        projects_override.extend(file.projects);
        has_projects_override = true;
    }

    for (_path, raw) in disk.projects_extra.into_iter() {
        let Some(raw) = raw else { continue };
        let value = yaml_from_raw_with_templates(&raw, &env)?;
        let file = serde_yaml::from_value::<ProjectsFile>(value)?;
        projects_override.extend(file.projects);
        has_projects_override = true;
    }

    if has_projects_override {
        config.projects = projects_override;
    }

    let config = config.normalized();
    config
        .validate_config_version()
        .map_err(ConfigError::ValidationError)?;

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

    struct EnvVarGuard {
        key: &'static str,
        prev: Option<std::ffi::OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: Option<&str>) -> Self {
            let prev = std::env::var_os(key);
            // SAFETY: tests using EnvVarGuard are serialized by env_lock().
            unsafe {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
            Self { key, prev }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            // SAFETY: tests using EnvVarGuard are serialized by env_lock().
            unsafe {
                match &self.prev {
                    Some(value) => std::env::set_var(self.key, value),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }

    fn write_file(path: &std::path::Path, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create dir");
        }
        std::fs::write(path, contents).expect("write file");
    }

    #[test]
    fn stable_read_retries_until_two_consecutive_reads_match() {
        let mut calls = 0;
        let got = read_stable_with_retries(|| {
            calls += 1;
            Ok(match calls {
                1 => "a",
                2 => "b",
                3 => "c",
                4 => "c",
                _ => unreachable!("unexpected call count"),
            })
        })
        .expect("stable value");

        assert_eq!(got, "c");
    }

    #[test]
    fn stable_read_errors_if_inputs_never_stabilize() {
        let mut calls = 0;
        let err = read_stable_with_retries(|| {
            calls += 1;
            Ok(calls)
        })
        .expect_err("should error");

        assert!(matches!(err, ConfigError::ValidationError(_)));
    }

    #[test]
    fn secret_env_overrides_system_env() {
        let _guard = env_lock().lock().unwrap();
        let _env = EnvVarGuard::set("GITHUB_PAT", Some("from_system"));

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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&secret_path, std::fs::Permissions::from_mode(0o600))
                .expect("chmod secret.env");
        }

        let loaded = try_load_config_from_file(&config_path).expect("load config");
        assert_eq!(loaded.github.pat.as_deref(), Some("from_secret"));
    }

    #[test]
    fn template_default_is_used_when_missing() {
        let _guard = env_lock().lock().unwrap();
        let _env = EnvVarGuard::set("GITHUB_PAT", None);

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
    }

    #[test]
    fn missing_template_var_without_default_is_validation_error() {
        let _guard = env_lock().lock().unwrap();
        let _env = EnvVarGuard::set("GITHUB_PAT", None);

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
    }

    #[test]
    fn reload_keeps_last_known_good_on_error() {
        let _guard = env_lock().lock().unwrap();
        let _env = EnvVarGuard::set("GITHUB_PAT", None);

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
