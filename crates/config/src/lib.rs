use std::{collections::HashMap, path::PathBuf};

use thiserror::Error;

pub mod cache_budget;
pub mod editor;
mod schema;

pub use editor::{EditorConfig, EditorOpenError, EditorType};
pub use schema::{
    AccessControlConfig, AccessControlMode, CURRENT_CONFIG_VERSION, Config, DiffPreviewGuardPreset,
    GitHubConfig, NotificationConfig, ShowcaseState, SoundFile, ThemeMode, UiLanguage,
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

fn load_secret_env(secret_env_path: &std::path::Path) -> Result<HashMap<String, String>, ConfigError> {
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
    fn lookup(&self, name: &str) -> Option<String> {
        if let Some(value) = self.secret.get(name) {
            return Some(value.to_string());
        }
        std::env::var(name).ok()
    }
}

fn resolve_templates_in_string(input: &str, env: &TemplateEnv) -> Result<String, ConfigError> {
    if !input.contains("${") {
        return Ok(input.to_string());
    }

    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some(start_rel) = input[cursor..].find("${") {
        let start = cursor + start_rel;
        output.push_str(&input[cursor..start]);

        let after = start + 2;
        let Some(end_rel) = input[after..].find('}') else {
            return Err(ConfigError::ValidationError(
                "Invalid template: missing '}'".to_string(),
            ));
        };
        let end = after + end_rel;
        let inner = &input[after..end];

        let (name, default) = match inner.split_once(":-") {
            Some((name, default)) => (name.trim(), Some(default)),
            None => (inner.trim(), None),
        };

        if name.is_empty() {
            return Err(ConfigError::ValidationError(
                "Invalid template: empty variable name".to_string(),
            ));
        }

        let resolved = match env.lookup(name) {
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
        cursor = end + 1;
    }

    output.push_str(&input[cursor..]);
    Ok(output)
}

fn resolve_yaml_templates(value: &mut serde_yaml::Value, env: &TemplateEnv) -> Result<(), ConfigError> {
    match value {
        serde_yaml::Value::Null
        | serde_yaml::Value::Bool(_)
        | serde_yaml::Value::Number(_) => Ok(()),
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

pub fn try_load_config_from_file(config_path: &PathBuf) -> Result<Config, ConfigError> {
    let raw = match std::fs::read_to_string(config_path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(err) => return Err(err.into()),
    };

    let config_dir = config_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(utils_core::vk_config_dir);
    let secret_env_path = config_dir.join("secret.env");
    let secret = load_secret_env(&secret_env_path)?;

    let mut value = serde_yaml::from_str::<serde_yaml::Value>(&raw)?;
    resolve_yaml_templates(&mut value, &TemplateEnv { secret })?;
    let config = serde_yaml::from_value::<Config>(value)?;
    Ok(config.normalized())
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

/// Saves the config to the given path
pub async fn save_config_to_file(
    config: &Config,
    config_path: &PathBuf,
) -> Result<(), ConfigError> {
    let normalized = config.clone().normalized();
    let raw_config = serde_yaml::to_string(&normalized)?;
    std::fs::write(config_path, raw_config)?;
    Ok(())
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
  pat: "${GITHUB_PAT}"
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
  pat: "${GITHUB_PAT:-fallback}"
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
  pat: "${GITHUB_PAT}"
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
  pat: "${GITHUB_PAT}"
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
}
