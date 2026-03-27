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
    ProjectRepoConfig, ProjectsFile, ShowcaseState, SoundFile, ThemeMode, UiLanguage,
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

fn load_secret_env(
    secret_env_path: &std::path::Path,
) -> Result<HashMap<String, String>, ConfigError> {
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

const MAX_TEMPLATE_REPLACEMENTS: usize = 128;
const MAX_TEMPLATE_OUTPUT_BYTES: usize = 64 * 1024;

fn resolve_templates_in_string(input: &str, env: &TemplateEnv) -> Result<String, ConfigError> {
    if !input.contains("{{") {
        return Ok(input.to_string());
    }

    fn ensure_room(output: &str, additional: &str) -> Result<(), ConfigError> {
        if output.len().saturating_add(additional.len()) > MAX_TEMPLATE_OUTPUT_BYTES {
            return Err(ConfigError::ValidationError(format!(
                "Template expansion exceeds max output length ({MAX_TEMPLATE_OUTPUT_BYTES} bytes)"
            )));
        }
        Ok(())
    }

    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    let mut replacements = 0usize;
    while let Some(start_rel) = input[cursor..].find("{{") {
        if replacements >= MAX_TEMPLATE_REPLACEMENTS {
            return Err(ConfigError::ValidationError(format!(
                "Template expansion exceeds max replacements ({MAX_TEMPLATE_REPLACEMENTS})"
            )));
        }

        let start = cursor + start_rel;
        let prefix = &input[cursor..start];
        ensure_room(&output, prefix)?;
        output.push_str(prefix);

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

        if namespace == "secret" && default.is_some() {
            return Err(ConfigError::ValidationError(
                "Invalid template: secret placeholders do not support default values".to_string(),
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

        ensure_room(&output, &resolved)?;
        output.push_str(&resolved);
        replacements += 1;
        cursor = end + 2;
    }

    let suffix = &input[cursor..];
    ensure_room(&output, suffix)?;
    output.push_str(suffix);
    Ok(output)
}

#[derive(Clone, Debug)]
enum TemplatePathSegment {
    Key(String),
    Index(usize),
}

fn template_path_to_string(path: &[TemplatePathSegment]) -> String {
    let mut out = String::new();
    for segment in path {
        match segment {
            TemplatePathSegment::Key(key) => {
                if out.is_empty() {
                    out.push_str(key);
                } else {
                    out.push('.');
                    out.push_str(key);
                }
            }
            TemplatePathSegment::Index(index) => {
                out.push('[');
                out.push_str(&index.to_string());
                out.push(']');
            }
        }
    }
    out
}

fn is_template_allowed_for_path(path: &[TemplatePathSegment]) -> bool {
    use TemplatePathSegment::{Index, Key};

    match path {
        [Key(a), Key(b)] if a == "github" && b == "pat" => true,
        [Key(a), Key(b)] if a == "github" && b == "oauth_token" => true,
        [Key(a), Key(b)] if a == "access_control" && b == "token" => true,

        [Key(a), Index(_), Key(b)] if a == "projects" && b == "dev_script" => true,
        [Key(a), Index(_), Key(b), Index(_), Key(c)]
            if a == "projects"
                && b == "repos"
                && matches!(c.as_str(), "setup_script" | "cleanup_script") =>
        {
            true
        }
        [Key(a), Index(_), Key(b), Key(c)]
            if a == "projects"
                && matches!(b.as_str(), "after_prepare_hook" | "before_cleanup_hook")
                && c == "command" =>
        {
            true
        }

        [
            Key(a),
            Key(b),
            Key(_executor),
            Key(_variant),
            Key(_agent),
            Key(c),
            Key(_env_key),
        ] if a == "executor_profiles" && b == "executors" && c == "env" => true,

        _ => false,
    }
}

fn collect_template_paths(
    value: &serde_json::Value,
    path: &mut Vec<TemplatePathSegment>,
    found: &mut Vec<Vec<TemplatePathSegment>>,
) {
    match value {
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
        serde_json::Value::String(s) => {
            if s.contains("{{") {
                found.push(path.clone());
            }
        }
        serde_json::Value::Array(items) => {
            for (idx, item) in items.iter().enumerate() {
                path.push(TemplatePathSegment::Index(idx));
                collect_template_paths(item, path, found);
                path.pop();
            }
        }
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                path.push(TemplatePathSegment::Key(k.clone()));
                collect_template_paths(v, path, found);
                path.pop();
            }
        }
    }
}

const TEMPLATE_WHITELIST_DOCS: &str = concat!(
    "Allowed template fields:\n",
    "- github.pat\n",
    "- github.oauth_token\n",
    "- access_control.token\n",
    "- projects[*].dev_script\n",
    "- projects[*].repos[*].setup_script\n",
    "- projects[*].repos[*].cleanup_script\n",
    "- projects[*].after_prepare_hook.command\n",
    "- projects[*].before_cleanup_hook.command\n",
    "- executor_profiles.executors.<EXECUTOR>.<VARIANT>.<EXECUTOR>.env.<NAME>\n",
);

fn validate_templates_are_whitelisted(config: &Config) -> Result<(), ConfigError> {
    let value = serde_json::to_value(config).map_err(|err| {
        ConfigError::ValidationError(format!(
            "Failed to serialize config for template validation: {err}"
        ))
    })?;

    let mut all_template_paths = Vec::new();
    collect_template_paths(&value, &mut Vec::new(), &mut all_template_paths);

    let mut non_whitelisted = all_template_paths
        .iter()
        .filter(|path| !is_template_allowed_for_path(path))
        .map(|path| template_path_to_string(path))
        .collect::<Vec<_>>();

    non_whitelisted.sort();
    non_whitelisted.dedup();

    let Some(first) = non_whitelisted.first() else {
        return Ok(());
    };

    Err(ConfigError::ValidationError(format!(
        "Template syntax is only allowed in specific fields, but was found at '{first}'.\n\n{TEMPLATE_WHITELIST_DOCS}\nMigration hint: move secrets into secret.env and reference them via {{{{secret.NAME}}}} in a whitelisted field (or pass them via executor profile env)."
    )))
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

#[derive(Debug, Clone)]
pub struct ConfigPair {
    pub runtime: Config,
    pub public: Config,
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
    config_path: &Path,
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
    config_path: &Path,
    include_secret_env: bool,
) -> Result<ConfigDiskInputs, ConfigError> {
    read_stable_with_retries(|| read_config_disk_inputs_once(config_path, include_secret_env))
}

fn resolve_templates_in_option_string(
    value: &mut Option<String>,
    env: &TemplateEnv,
) -> Result<(), ConfigError> {
    let Some(input) = value.as_deref() else {
        return Ok(());
    };
    *value = Some(resolve_templates_in_string(input, env)?);
    Ok(())
}

fn resolve_templates_in_executor_profiles_env(
    profiles: &mut executors::profile::ExecutorConfigs,
    env: &TemplateEnv,
) -> Result<(), ConfigError> {
    for executor_config in profiles.executors.values_mut() {
        for agent in executor_config.configurations.values_mut() {
            let Some(env_map) = agent.cmd_env_mut() else {
                continue;
            };
            for value in env_map.values_mut() {
                *value = resolve_templates_in_string(value, env)?;
            }
        }
    }
    Ok(())
}

fn resolve_whitelisted_templates(
    config: &mut Config,
    env: &TemplateEnv,
) -> Result<(), ConfigError> {
    resolve_templates_in_option_string(&mut config.github.pat, env)?;
    resolve_templates_in_option_string(&mut config.github.oauth_token, env)?;
    resolve_templates_in_option_string(&mut config.access_control.token, env)?;

    if let Some(profiles) = config.executor_profiles.as_mut() {
        resolve_templates_in_executor_profiles_env(profiles, env)?;
    }

    for project in config.projects.iter_mut() {
        resolve_templates_in_option_string(&mut project.dev_script, env)?;

        if let Some(hook) = project.after_prepare_hook.as_mut() {
            hook.command = resolve_templates_in_string(&hook.command, env)?;
        }
        if let Some(hook) = project.before_cleanup_hook.as_mut() {
            hook.command = resolve_templates_in_string(&hook.command, env)?;
        }

        for repo in project.repos.iter_mut() {
            resolve_templates_in_option_string(&mut repo.setup_script, env)?;
            resolve_templates_in_option_string(&mut repo.cleanup_script, env)?;
        }
    }

    Ok(())
}

/// Load config.yaml + optional projects.yaml/projects.d without resolving `{{secret.*}}`/`{{env.*}}`
/// templates. This is intended for *display* and API responses to avoid leaking expanded secrets.
pub fn try_load_public_config_from_file(config_path: &Path) -> Result<Config, ConfigError> {
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

pub fn try_load_config_from_file(config_path: &Path) -> Result<Config, ConfigError> {
    let disk = read_config_disk_inputs_stable(config_path, true)?;
    let env = TemplateEnv {
        secret: disk.secret_env,
    };

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

    validate_templates_are_whitelisted(&config)?;
    resolve_whitelisted_templates(&mut config, &env)?;

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

fn build_public_config_from_disk(disk: &ConfigDiskInputs) -> Result<Config, ConfigError> {
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

    for (_path, raw) in disk.projects_extra.iter() {
        let Some(raw) = raw.as_deref() else {
            continue;
        };
        let file = serde_yaml::from_str::<ProjectsFile>(raw)?;
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

fn build_runtime_config_from_disk(
    disk: &ConfigDiskInputs,
    env: &TemplateEnv,
) -> Result<Config, ConfigError> {
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

    for (_path, raw) in disk.projects_extra.iter() {
        let Some(raw) = raw.as_deref() else {
            continue;
        };
        let file = serde_yaml::from_str::<ProjectsFile>(raw)?;
        projects_override.extend(file.projects);
        has_projects_override = true;
    }

    if has_projects_override {
        config.projects = projects_override;
    }

    validate_templates_are_whitelisted(&config)?;
    resolve_whitelisted_templates(&mut config, env)?;

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

pub fn try_load_config_pair_from_file(config_path: &Path) -> Result<ConfigPair, ConfigError> {
    let disk = read_config_disk_inputs_stable(config_path, true)?;
    let env = TemplateEnv {
        secret: disk.secret_env.clone(),
    };

    Ok(ConfigPair {
        runtime: build_runtime_config_from_disk(&disk, &env)?,
        public: build_public_config_from_disk(&disk)?,
    })
}

pub fn reload_config_keep_last_known_good(
    current: &Config,
    config_path: &Path,
) -> (Config, Option<String>) {
    match try_load_config_from_file(config_path) {
        Ok(config) => (config, None),
        Err(err) => (current.clone(), Some(err.to_string())),
    }
}

/// Will always return config, falling back to defaults on missing/invalid files.
pub async fn load_config_from_file(config_path: &Path) -> Config {
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
    fn config_pair_runtime_expands_templates_but_public_does_not() {
        let _guard = env_lock().lock().unwrap();
        let _env = EnvVarGuard::set("GITHUB_PAT", Some("from_system"));

        let dir = std::env::temp_dir().join(format!("vk-config-test-{}", uuid::Uuid::new_v4()));
        let config_path = dir.join("config.yaml");
        let secret_path = dir.join("secret.env");
        let projects_path = dir.join("projects.yaml");

        write_file(
            &config_path,
            r#"
github:
  pat: "{{env.GITHUB_PAT}}"
projects:
  - id: 00000000-0000-0000-0000-000000000001
    name: "inline"
    repos:
      - path: "/tmp/inline"
"#,
        );

        write_file(&secret_path, "GITHUB_PAT=from_secret\nPROJECT_ROOT=/tmp\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&secret_path, std::fs::Permissions::from_mode(0o600))
                .expect("chmod secret.env");
        }

        write_file(
            &projects_path,
            r#"
projects:
  - id: 00000000-0000-0000-0000-000000000002
    name: "from_projects_yaml"
    repos:
      - path: "/tmp/repo"
        setup_script: "echo {{env.PROJECT_ROOT}}/repo"
"#,
        );

        let pair = try_load_config_pair_from_file(&config_path).expect("load config pair");
        assert_eq!(pair.runtime.github.pat.as_deref(), Some("from_secret"));
        assert_eq!(
            pair.public.github.pat.as_deref(),
            Some("{{env.GITHUB_PAT}}")
        );

        assert_eq!(pair.runtime.projects.len(), 1);
        assert_eq!(pair.runtime.projects[0].name, "from_projects_yaml");
        assert_eq!(pair.runtime.projects[0].repos.len(), 1);
        assert_eq!(pair.runtime.projects[0].repos[0].path, "/tmp/repo");
        assert_eq!(
            pair.runtime.projects[0].repos[0].setup_script.as_deref(),
            Some("echo /tmp/repo")
        );

        assert_eq!(pair.public.projects.len(), 1);
        assert_eq!(pair.public.projects[0].name, "from_projects_yaml");
        assert_eq!(pair.public.projects[0].repos.len(), 1);
        assert_eq!(pair.public.projects[0].repos[0].path, "/tmp/repo");
        assert_eq!(
            pair.public.projects[0].repos[0].setup_script.as_deref(),
            Some("echo {{env.PROJECT_ROOT}}/repo")
        );
    }

    #[test]
    fn template_in_non_whitelisted_field_is_rejected() {
        let _guard = env_lock().lock().unwrap();
        let _env = EnvVarGuard::set("PROJECT_ROOT", Some("/tmp"));

        let dir = std::env::temp_dir().join(format!("vk-config-test-{}", uuid::Uuid::new_v4()));
        let config_path = dir.join("config.yaml");

        write_file(
            &config_path,
            r#"
projects:
  - id: 00000000-0000-0000-0000-000000000001
    name: "test"
    repos:
      - path: "{{env.PROJECT_ROOT}}/repo"
"#,
        );

        let err = try_load_config_from_file(&config_path).expect_err("expected error");
        match err {
            ConfigError::ValidationError(message) => {
                assert!(message.contains("projects[0].repos[0].path"));
                assert!(message.contains("Migration hint"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn executor_profile_env_templates_are_resolved() {
        let _guard = env_lock().lock().unwrap();
        let _env = EnvVarGuard::set("OPENAI_API_KEY", Some("from_system"));

        let dir = std::env::temp_dir().join(format!("vk-config-test-{}", uuid::Uuid::new_v4()));
        let config_path = dir.join("config.yaml");
        let secret_path = dir.join("secret.env");

        write_file(
            &config_path,
            r#"
executor_profiles:
  executors:
    CLAUDE_CODE:
      DEFAULT:
        CLAUDE_CODE:
          env:
            OPENAI_API_KEY: "{{env.OPENAI_API_KEY}}"
"#,
        );
        write_file(&secret_path, "OPENAI_API_KEY=from_secret\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&secret_path, std::fs::Permissions::from_mode(0o600))
                .expect("chmod secret.env");
        }

        let loaded = try_load_config_from_file(&config_path).expect("load config");
        let profiles = loaded
            .executor_profiles
            .as_ref()
            .expect("executor profiles");
        let executor = profiles
            .executors
            .get(&executors_protocol::BaseCodingAgent::ClaudeCode)
            .expect("CLAUDE_CODE config");
        let default = executor.configurations.get("DEFAULT").expect("DEFAULT");
        match default {
            executors::executors::CodingAgent::ClaudeCode(cfg) => {
                let env_map = cfg.cmd.env.as_ref().expect("env map");
                assert_eq!(env_map.get("OPENAI_API_KEY").unwrap(), "from_secret");
            }
            other => panic!("unexpected agent: {other:?}"),
        }
    }

    #[test]
    fn executor_profile_non_env_template_is_rejected() {
        let _guard = env_lock().lock().unwrap();
        let _env = EnvVarGuard::set("OPENAI_MODEL", Some("model"));

        let dir = std::env::temp_dir().join(format!("vk-config-test-{}", uuid::Uuid::new_v4()));
        let config_path = dir.join("config.yaml");

        write_file(
            &config_path,
            r#"
executor_profiles:
  executors:
    CLAUDE_CODE:
      DEFAULT:
        CLAUDE_CODE:
          model: "{{env.OPENAI_MODEL}}"
"#,
        );

        let err = try_load_config_from_file(&config_path).expect_err("expected error");
        match err {
            ConfigError::ValidationError(message) => {
                assert!(
                    message.contains(
                        "executor_profiles.executors.CLAUDE_CODE.DEFAULT.CLAUDE_CODE.model"
                    )
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn template_replacement_limit_is_enforced() {
        let _guard = env_lock().lock().unwrap();
        let _env = EnvVarGuard::set("GITHUB_PAT", Some("x"));

        let dir = std::env::temp_dir().join(format!("vk-config-test-{}", uuid::Uuid::new_v4()));
        let config_path = dir.join("config.yaml");

        let pat = "{{env.GITHUB_PAT}}".repeat(MAX_TEMPLATE_REPLACEMENTS + 1);
        write_file(&config_path, &format!("github:\n  pat: \"{pat}\"\n"));

        let err = try_load_config_from_file(&config_path).expect_err("expected error");
        match err {
            ConfigError::ValidationError(message) => {
                assert!(message.contains("max replacements"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn template_output_limit_is_enforced() {
        let _guard = env_lock().lock().unwrap();
        let huge = "a".repeat(MAX_TEMPLATE_OUTPUT_BYTES + 1);
        let _env = EnvVarGuard::set("GITHUB_PAT", Some(&huge));

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
                assert!(message.contains("max output length"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
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
