//! Utilities for reading and writing external agent config files (not the server's own config).
//!
//! These helpers abstract over JSON vs TOML formats used by different agents.

use std::{collections::HashMap, sync::LazyLock};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::fs;
use ts_rs::TS;

use crate::executors::{CodingAgent, ExecutorError};

static DEFAULT_MCP_JSON: &str = include_str!("../default_mcp.json");
pub static PRECONFIGURED_MCP_SERVERS: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str::<Value>(DEFAULT_MCP_JSON).expect("Failed to parse default MCP JSON")
});

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct McpConfig {
    servers: HashMap<String, serde_json::Value>,
    pub servers_path: Vec<String>,
    pub template: serde_json::Value,
    pub preconfigured: serde_json::Value,
    pub is_toml_config: bool,
}

impl McpConfig {
    pub fn new(
        servers_path: Vec<String>,
        template: serde_json::Value,
        preconfigured: serde_json::Value,
        is_toml_config: bool,
    ) -> Self {
        Self {
            servers: HashMap::new(),
            servers_path,
            template,
            preconfigured,
            is_toml_config,
        }
    }
    pub fn set_servers(&mut self, servers: HashMap<String, serde_json::Value>) {
        self.servers = servers;
    }
}

/// Read an agent's external config file (JSON or TOML) and normalize it to serde_json::Value.
pub async fn read_agent_config(
    config_path: &std::path::Path,
    mcp_config: &McpConfig,
) -> Result<Value, ExecutorError> {
    if let Ok(file_content) = fs::read_to_string(config_path).await {
        if mcp_config.is_toml_config {
            // Parse TOML then convert to JSON Value
            if file_content.trim().is_empty() {
                return Ok(serde_json::json!({}));
            }
            let toml_val: toml::Value = toml::from_str(&file_content)?;
            let json_string = serde_json::to_string(&toml_val)?;
            Ok(serde_json::from_str(&json_string)?)
        } else {
            Ok(serde_json::from_str(&file_content)?)
        }
    } else {
        Ok(mcp_config.template.clone())
    }
}

/// Write an agent's external config (as serde_json::Value) back to disk in the agent's format (JSON or TOML).
pub async fn write_agent_config(
    config_path: &std::path::Path,
    mcp_config: &McpConfig,
    config: &Value,
) -> Result<(), ExecutorError> {
    if mcp_config.is_toml_config {
        // Convert JSON Value back to TOML
        let toml_value: toml::Value = serde_json::from_str(&serde_json::to_string(config)?)?;
        let toml_content = toml::to_string_pretty(&toml_value)?;
        fs::write(config_path, toml_content).await?;
    } else {
        let json_content = serde_json::to_string_pretty(config)?;
        fs::write(config_path, json_content).await?;
    }
    Ok(())
}

type ServerMap = Map<String, Value>;

fn is_http_server(s: &Map<String, Value>) -> bool {
    matches!(s.get("type").and_then(Value::as_str), Some("http"))
}

fn is_stdio(s: &Map<String, Value>) -> bool {
    !is_http_server(s) && s.get("command").is_some()
}

fn extract_meta(mut obj: ServerMap) -> (ServerMap, Option<Value>) {
    let meta = obj.remove("meta");
    (obj, meta)
}

fn attach_meta(mut obj: ServerMap, meta: Option<Value>) -> Value {
    if let Some(m) = meta {
        obj.insert("meta".to_string(), m);
    }
    Value::Object(obj)
}

fn ensure_header(headers: &mut Map<String, Value>, key: &str, val: &str) {
    match headers.get_mut(key) {
        Some(Value::String(_)) => {}
        _ => {
            headers.insert(key.to_string(), Value::String(val.to_string()));
        }
    }
}

fn transform_http_servers<F>(mut servers: ServerMap, mut f: F) -> ServerMap
where
    F: FnMut(Map<String, Value>) -> Map<String, Value>,
{
    for (_k, v) in servers.iter_mut() {
        if let Value::Object(s) = v
            && is_http_server(s)
        {
            let taken = std::mem::take(s);
            *s = f(taken);
        }
    }
    servers
}

// --- Adapters ---------------------------------------------------------------

fn adapt_passthrough(servers: ServerMap, meta: Option<Value>) -> Value {
    attach_meta(servers, meta)
}

fn adapt_gemini(servers: ServerMap, meta: Option<Value>) -> Value {
    let servers = transform_http_servers(servers, |mut s| {
        let url = s
            .remove("url")
            .unwrap_or_else(|| Value::String(String::new()));
        let mut headers = s
            .remove("headers")
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();

        ensure_header(
            &mut headers,
            "Accept",
            "application/json, text/event-stream",
        );
        Map::from_iter([
            ("httpUrl".to_string(), url),
            ("headers".to_string(), Value::Object(headers)),
        ])
    });
    attach_meta(servers, meta)
}

fn adapt_cursor(servers: ServerMap, meta: Option<Value>) -> Value {
    let servers = transform_http_servers(servers, |mut s| {
        let url = s
            .remove("url")
            .unwrap_or_else(|| Value::String(String::new()));
        let headers = s
            .remove("headers")
            .unwrap_or_else(|| Value::Object(Default::default()));
        Map::from_iter([("url".to_string(), url), ("headers".to_string(), headers)])
    });
    attach_meta(servers, meta)
}

fn adapt_codex(mut servers: ServerMap, mut meta: Option<Value>) -> Value {
    servers.retain(|_, v| v.as_object().map(is_stdio).unwrap_or(false));

    if let Some(Value::Object(ref mut m)) = meta {
        m.retain(|k, _| servers.contains_key(k));
        servers.insert("meta".to_string(), Value::Object(std::mem::take(m)));
        meta = None; // already attached above
    }
    attach_meta(servers, meta)
}

fn adapt_opencode(servers: ServerMap, meta: Option<Value>) -> Value {
    let mut servers = transform_http_servers(servers, |mut s| {
        let url = s
            .remove("url")
            .unwrap_or_else(|| Value::String(String::new()));

        let mut headers = s
            .remove("headers")
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();

        ensure_header(
            &mut headers,
            "Accept",
            "application/json, text/event-stream",
        );

        Map::from_iter([
            ("type".to_string(), Value::String("remote".to_string())),
            ("url".to_string(), url),
            ("headers".to_string(), Value::Object(headers)),
            ("enabled".to_string(), Value::Bool(true)),
        ])
    });

    for (_k, v) in servers.iter_mut() {
        if let Value::Object(s) = v
            && is_stdio(s)
        {
            let command_str = s
                .remove("command")
                .and_then(|v| match v {
                    Value::String(s) => Some(s),
                    _ => None,
                })
                .unwrap_or_default();

            let mut cmd_vec: Vec<Value> = Vec::new();
            if !command_str.is_empty() {
                cmd_vec.push(Value::String(command_str));
            }

            if let Some(arr) = s.remove("args").and_then(|v| match v {
                Value::Array(arr) => Some(arr),
                _ => None,
            }) {
                for a in arr {
                    match a {
                        Value::String(s) => cmd_vec.push(Value::String(s)),
                        other => cmd_vec.push(other), // fall back to raw value if not string
                    }
                }
            }

            let mut new_map = Map::new();
            new_map.insert("type".to_string(), Value::String("local".to_string()));
            new_map.insert("command".to_string(), Value::Array(cmd_vec));
            new_map.insert("enabled".to_string(), Value::Bool(true));
            *s = new_map;
        }
    }

    attach_meta(servers, meta)
}

fn adapt_copilot(mut servers: ServerMap, meta: Option<Value>) -> Value {
    for (_, value) in servers.iter_mut() {
        if let Value::Object(s) = value
            && !s.contains_key("tools")
        {
            s.insert(
                "tools".to_string(),
                Value::Array(vec![Value::String("*".to_string())]),
            );
        }
    }
    attach_meta(servers, meta)
}

enum Adapter {
    Passthrough,
    Gemini,
    Cursor,
    Codex,
    Opencode,
    Copilot,
}

fn apply_adapter(adapter: Adapter, canonical: Value) -> Value {
    let (servers_only, meta) = match canonical.as_object() {
        Some(map) => extract_meta(map.clone()),
        None => (ServerMap::new(), None),
    };

    match adapter {
        Adapter::Passthrough => adapt_passthrough(servers_only, meta),
        Adapter::Gemini => adapt_gemini(servers_only, meta),
        Adapter::Cursor => adapt_cursor(servers_only, meta),
        Adapter::Codex => adapt_codex(servers_only, meta),
        Adapter::Opencode => adapt_opencode(servers_only, meta),
        Adapter::Copilot => adapt_copilot(servers_only, meta),
    }
}

impl CodingAgent {
    pub fn preconfigured_mcp(&self) -> Value {
        use Adapter::*;

        let adapter = match self {
            CodingAgent::ClaudeCode(_) | CodingAgent::Amp(_) | CodingAgent::Droid(_) => Passthrough,
            CodingAgent::QwenCode(_) | CodingAgent::Gemini(_) => Gemini,
            CodingAgent::CursorAgent(_) => Cursor,
            CodingAgent::Codex(_) => Codex,
            CodingAgent::Opencode(_) => Opencode,
            CodingAgent::Copilot(..) => Copilot,
        };

        let canonical = PRECONFIGURED_MCP_SERVERS.clone();
        apply_adapter(adapter, canonical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Map};
    use uuid::Uuid;

    fn canonical_config() -> Value {
        json!({
            "stdio_server": {
                "command": "tool",
                "args": ["--flag"]
            },
            "http_server": {
                "type": "http",
                "url": "https://example.com/mcp",
                "headers": {
                    "Authorization": "token"
                }
            },
            "meta": {
                "stdio_server": { "name": "Local" },
                "http_server": { "name": "Remote" }
            }
        })
    }

    fn obj<'a>(value: &'a Value, key: &str) -> &'a Map<String, Value> {
        value
            .get(key)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("expected object for {key}"))
    }

    fn temp_file_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("mcp-config-test-{}-{name}", Uuid::new_v4()))
    }

    #[test]
    fn apply_adapter_passthrough_preserves_servers_and_meta() {
        let adapted = apply_adapter(Adapter::Passthrough, canonical_config());

        let meta = obj(&adapted, "meta");
        assert!(meta.contains_key("stdio_server"));
        assert!(meta.contains_key("http_server"));
        assert!(obj(&adapted, "stdio_server").contains_key("command"));
        assert!(obj(&adapted, "http_server").contains_key("url"));
    }

    #[test]
    fn apply_adapter_gemini_transforms_http_headers() {
        let adapted = apply_adapter(Adapter::Gemini, canonical_config());

        let http = obj(&adapted, "http_server");
        assert!(http.get("url").is_none());
        assert_eq!(
            http.get("httpUrl").and_then(Value::as_str),
            Some("https://example.com/mcp")
        );
        let headers = http
            .get("headers")
            .and_then(Value::as_object)
            .expect("headers should be object");
        assert_eq!(
            headers.get("Authorization").and_then(Value::as_str),
            Some("token")
        );
        assert_eq!(
            headers.get("Accept").and_then(Value::as_str),
            Some("application/json, text/event-stream")
        );

        let stdio = obj(&adapted, "stdio_server");
        assert_eq!(stdio.get("command").and_then(Value::as_str), Some("tool"));
    }

    #[test]
    fn apply_adapter_codex_filters_http_and_meta() {
        let adapted = apply_adapter(Adapter::Codex, canonical_config());

        assert!(adapted.get("http_server").is_none());
        assert!(adapted.get("stdio_server").is_some());

        let meta = obj(&adapted, "meta");
        assert!(meta.contains_key("stdio_server"));
        assert!(!meta.contains_key("http_server"));
    }

    #[test]
    fn apply_adapter_opencode_transforms_http_and_stdio() {
        let adapted = apply_adapter(Adapter::Opencode, canonical_config());

        let http = obj(&adapted, "http_server");
        assert_eq!(http.get("type").and_then(Value::as_str), Some("remote"));
        assert_eq!(
            http.get("url").and_then(Value::as_str),
            Some("https://example.com/mcp")
        );
        assert_eq!(http.get("enabled").and_then(Value::as_bool), Some(true));
        let headers = http
            .get("headers")
            .and_then(Value::as_object)
            .expect("headers should be object");
        assert_eq!(
            headers.get("Accept").and_then(Value::as_str),
            Some("application/json, text/event-stream")
        );

        let stdio = obj(&adapted, "stdio_server");
        assert_eq!(stdio.get("type").and_then(Value::as_str), Some("local"));
        assert_eq!(stdio.get("enabled").and_then(Value::as_bool), Some(true));
        let command = stdio
            .get("command")
            .and_then(Value::as_array)
            .expect("command should be array");
        assert_eq!(command.first().and_then(Value::as_str), Some("tool"));
        assert_eq!(command.get(1).and_then(Value::as_str), Some("--flag"));
    }

    #[test]
    fn apply_adapter_copilot_inserts_tools() {
        let adapted = apply_adapter(Adapter::Copilot, canonical_config());

        let http = obj(&adapted, "http_server");
        let tools = http
            .get("tools")
            .and_then(Value::as_array)
            .expect("tools should be array");
        assert_eq!(tools.first().and_then(Value::as_str), Some("*"));

        let stdio = obj(&adapted, "stdio_server");
        let tools = stdio
            .get("tools")
            .and_then(Value::as_array)
            .expect("tools should be array");
        assert_eq!(tools.first().and_then(Value::as_str), Some("*"));
    }

    #[tokio::test]
    async fn read_agent_config_returns_template_when_missing() {
        let config_path = temp_file_path("missing.json");
        let template = json!({ "mcpServers": {} });
        let mcp_config = McpConfig::new(vec![], template.clone(), json!({}), false);

        let loaded = read_agent_config(&config_path, &mcp_config)
            .await
            .expect("read config");
        assert_eq!(loaded, template);
    }

    #[tokio::test]
    async fn read_agent_config_empty_toml_returns_empty_object() {
        let config_path = temp_file_path("empty.toml");
        tokio::fs::write(&config_path, "").await.expect("write file");
        let mcp_config = McpConfig::new(vec![], json!({}), json!({}), true);

        let loaded = read_agent_config(&config_path, &mcp_config)
            .await
            .expect("read config");
        assert_eq!(loaded, json!({}));
        let _ = tokio::fs::remove_file(&config_path).await;
    }

    #[tokio::test]
    async fn write_agent_config_writes_json() {
        let config_path = temp_file_path("write.json");
        let mcp_config = McpConfig::new(vec![], json!({}), json!({}), false);
        let config = json!({
            "mcpServers": {
                "local": { "command": "tool" }
            }
        });

        write_agent_config(&config_path, &mcp_config, &config)
            .await
            .expect("write config");

        let content = tokio::fs::read_to_string(&config_path)
            .await
            .expect("read config");
        assert!(content.contains("\"mcpServers\""));
        assert!(content.contains("\"local\""));
        let _ = tokio::fs::remove_file(&config_path).await;
    }
}
