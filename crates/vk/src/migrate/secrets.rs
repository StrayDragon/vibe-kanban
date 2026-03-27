use std::collections::HashMap;

use anyhow::{Context, Result};
use executors_protocol::{BaseCodingAgent, ExecutorProfileId};

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
    if trimmed.is_empty() {
        return false;
    }
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
                } else {
                    rewrite_secrets_in_env_objects(value, secrets);
                }
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

pub fn config_json_to_yaml_fragment(
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

pub fn profiles_json_to_executor_profiles_yaml(
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

pub fn secret_env_to_string(tool_name: &str, vars: &HashMap<String, String>) -> String {
    let mut keys = vars.keys().cloned().collect::<Vec<_>>();
    keys.sort();

    let mut out = String::new();
    out.push_str(&format!(
        "# Generated by {tool_name} at {}\n",
        chrono::Utc::now().to_rfc3339()
    ));
    for key in keys {
        let value = vars.get(&key).expect("key exists");
        out.push_str(&format!("{key}={value}\n"));
    }
    out
}

pub fn validate_yaml_with_secret_env(yaml: &str, secret_env: Option<&str>) -> Result<()> {
    let validate_dir =
        std::env::temp_dir().join(format!("vk-asset-export-validate-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&validate_dir).context("Failed to create validation temp dir")?;
    let config_path = validate_dir.join("config.yaml");
    std::fs::write(&config_path, yaml).context("Failed to write validation config.yaml")?;
    if let Some(secret_env) = secret_env {
        let secret_path = validate_dir.join("secret.env");
        std::fs::write(&secret_path, secret_env)
            .context("Failed to write validation secret.env")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&secret_path, std::fs::Permissions::from_mode(0o600));
        }
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
