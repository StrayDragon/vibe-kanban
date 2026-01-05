use std::{collections::HashMap, path::{Path, PathBuf}};

use tokio::fs;

use crate::executors::ExecutorError;

pub fn default_claude_code_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("llman").join("claude-code.toml"))
}

pub fn resolve_claude_code_config_path(override_path: Option<&str>) -> Option<PathBuf> {
    let resolved = override_path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(expand_tilde);

    resolved.or_else(default_claude_code_config_path)
}

pub async fn read_claude_code_groups(
    path: &Path,
) -> Result<HashMap<String, HashMap<String, String>>, ExecutorError> {
    let contents = fs::read_to_string(path)
        .await
        .map_err(ExecutorError::Io)?;
    parse_claude_code_groups(&contents)
}

pub fn parse_claude_code_groups(
    contents: &str,
) -> Result<HashMap<String, HashMap<String, String>>, ExecutorError> {
    let parsed: toml::Value = toml::from_str(contents)?;
    let mut groups = HashMap::new();

    let Some(groups_table) = parsed.get("groups").and_then(|v| v.as_table()) else {
        return Ok(groups);
    };

    for (group_name, group_value) in groups_table {
        let Some(group_table) = group_value.as_table() else {
            tracing::warn!(
                "LLMAN group '{group_name}' is not a table; skipping"
            );
            continue;
        };

        let mut env = HashMap::new();
        for (key, value) in group_table {
            if let Some(value_str) = value.as_str() {
                env.insert(key.to_string(), value_str.to_string());
            } else {
                tracing::warn!(
                    "LLMAN group '{group_name}' key '{key}' is not a string; skipping"
                );
            }
        }

        groups.insert(group_name.to_string(), env);
    }

    Ok(groups)
}

fn expand_tilde(path: &str) -> PathBuf {
    if path == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(path));
    }

    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }

    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_groups_with_strings_and_non_strings() {
        let contents = r#"
[groups.minimax]
ANTHROPIC_AUTH_TOKEN = "token"
API_TIMEOUT_MS = "3000"
MAX_RETRIES = 3

[groups.empty]
# no entries

[groups.nested]
child = { foo = "bar" }
"#;

        let groups = parse_claude_code_groups(contents).expect("parse should succeed");
        let minimax = groups.get("minimax").expect("minimax group");
        assert_eq!(minimax.get("ANTHROPIC_AUTH_TOKEN"), Some(&"token".to_string()));
        assert_eq!(minimax.get("API_TIMEOUT_MS"), Some(&"3000".to_string()));
        assert!(!minimax.contains_key("MAX_RETRIES"));

        let empty = groups.get("empty").expect("empty group");
        assert!(empty.is_empty());

        let nested = groups.get("nested").expect("nested group");
        assert!(nested.is_empty());
    }

    #[test]
    fn resolve_path_prefers_override() {
        let path = resolve_claude_code_config_path(Some("/tmp/llman.toml"))
            .expect("path");
        assert_eq!(path, PathBuf::from("/tmp/llman.toml"));
    }
}
