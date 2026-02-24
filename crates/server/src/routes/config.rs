use std::collections::HashMap;

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http,
    response::{Json as ResponseJson, Response},
    routing::{get, post, put},
};
use deployment::{Deployment, DeploymentError};
use executors::{
    agent_command::{AgentCommandResolution, agent_command_resolver},
    executors::{
        AvailabilityInfo, BaseAgentCapability, BaseCodingAgent, CodingAgent,
        StandardCodingAgentExecutor,
    },
    llman,
    mcp_config::{McpConfig, read_agent_config, write_agent_config},
    profile::{ExecutorConfigs, ExecutorProfileId, canonical_variant_key},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use services::services::{
    config::{
        Config, ConfigError, SoundFile,
        editor::{EditorConfig, EditorType},
        save_config_to_file,
    },
    github::GitHubService,
};
use tokio::fs;
use ts_rs::TS;
use utils::{assets::config_path, response::ApiResponse};

use crate::{DeploymentImpl, error::ApiError};

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/info", get(get_user_system_info))
        .route("/config", put(update_config))
        .route("/sounds/{sound}", get(get_sound))
        .route("/mcp-config", get(get_mcp_servers).post(update_mcp_servers))
        .route("/profiles", get(get_profiles).put(update_profiles))
        .route("/profiles/llman-path", get(resolve_llman_path))
        .route("/profiles/import-llman", post(import_llman_profiles))
        .route(
            "/editors/check-availability",
            get(check_editor_availability),
        )
        .route("/agents/check-availability", get(check_agent_availability))
        .route("/preflight/cli", get(cli_dependency_preflight))
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct Environment {
    pub os_type: String,
    pub os_version: String,
    pub os_architecture: String,
    pub bitness: String,
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment {
    pub fn new() -> Self {
        let info = os_info::get();
        Environment {
            os_type: info.os_type().to_string(),
            os_version: info.version().to_string(),
            os_architecture: info.architecture().unwrap_or("unknown").to_string(),
            bitness: info.bitness().to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct UserSystemInfo {
    pub config: Config,
    #[serde(flatten)]
    pub profiles: ExecutorConfigs,
    pub environment: Environment,
    /// Capabilities supported per executor (e.g., { "CLAUDE_CODE": ["SESSION_FORK"] })
    pub capabilities: HashMap<String, Vec<BaseAgentCapability>>,
    /// Resolved command source/version per executor
    pub agent_command_resolutions: HashMap<String, AgentCommandResolution>,
}

// TODO: update frontend, BE schema has changed, this replaces GET /config and /config/constants
#[axum::debug_handler]
async fn get_user_system_info(
    State(deployment): State<DeploymentImpl>,
) -> ResponseJson<ApiResponse<UserSystemInfo>> {
    let config = deployment.config().read().await;
    let mut redacted_config = config.clone();
    redacted_config.access_control.token = None;

    let user_system_info = UserSystemInfo {
        config: redacted_config,
        profiles: ExecutorConfigs::get_cached(),
        environment: Environment::new(),
        capabilities: {
            let mut caps: HashMap<String, Vec<BaseAgentCapability>> = HashMap::new();
            let profs = ExecutorConfigs::get_cached();
            for key in profs.executors.keys() {
                if let Some(agent) = profs.get_coding_agent(&ExecutorProfileId::new(*key)) {
                    caps.insert(key.to_string(), agent.capabilities());
                }
            }
            caps
        },
        agent_command_resolutions: agent_command_resolver().snapshot().await,
    };

    ResponseJson(ApiResponse::success(user_system_info))
}

async fn update_config(
    State(deployment): State<DeploymentImpl>,
    Json(new_config): Json<Config>,
) -> Result<ResponseJson<ApiResponse<Config>>, ApiError> {
    let config_path = config_path();

    // Validate git branch prefix
    if !utils::git::is_valid_branch_prefix(&new_config.git_branch_prefix) {
        return Err(ApiError::BadRequest(
            "Invalid git branch prefix. Must be a valid git branch name component without slashes."
                .to_string(),
        ));
    }

    let new_config = new_config.normalized();

    if matches!(
        new_config.access_control.mode,
        services::services::config::AccessControlMode::Token
    ) && new_config
        .access_control
        .token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .is_none()
    {
        return Err(ApiError::BadRequest(
            "accessControl.token is required when accessControl.mode=TOKEN".to_string(),
        ));
    }

    // Get old config state before updating
    let old_config = deployment.config().read().await.clone();

    save_config_to_file(&new_config, &config_path).await?;

    let mut config = deployment.config().write().await;
    *config = new_config.clone();
    drop(config);

    // Run side effects on config transitions
    handle_config_events(&deployment, &old_config, &new_config).await;

    let mut response_config = new_config;
    response_config.access_control.token = None;
    Ok(ResponseJson(ApiResponse::success(response_config)))
}

async fn handle_config_events(deployment: &DeploymentImpl, old: &Config, new: &Config) {
    if !old.disclaimer_acknowledged && new.disclaimer_acknowledged {
        // Spawn auto project setup as background task to avoid blocking config response
        let deployment_clone = deployment.clone();
        tokio::spawn(async move {
            deployment_clone.trigger_auto_project_setup().await;
        });
    }
}

async fn get_sound(Path(sound): Path<SoundFile>) -> Result<Response, ApiError> {
    let sound = sound.serve().await.map_err(DeploymentError::Other)?;
    let response = Response::builder()
        .status(http::StatusCode::OK)
        .header(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("audio/wav"),
        )
        .body(Body::from(sound.data.into_owned()))
        .unwrap();
    Ok(response)
}

#[derive(TS, Debug, Deserialize)]
pub struct McpServerQuery {
    executor: BaseCodingAgent,
}

#[derive(TS, Debug, Serialize, Deserialize)]
pub struct GetMcpServerResponse {
    // servers: HashMap<String, Value>,
    mcp_config: McpConfig,
    config_path: String,
}

#[derive(TS, Debug, Serialize, Deserialize)]
pub struct UpdateMcpServersBody {
    servers: HashMap<String, Value>,
}

async fn get_mcp_servers(
    State(_deployment): State<DeploymentImpl>,
    Query(query): Query<McpServerQuery>,
) -> Result<ResponseJson<ApiResponse<GetMcpServerResponse>>, ApiError> {
    let coding_agent = ExecutorConfigs::get_cached()
        .get_coding_agent(&ExecutorProfileId::new(query.executor))
        .ok_or(ConfigError::ValidationError(
            "Executor not found".to_string(),
        ))?;

    if !coding_agent.supports_mcp() {
        return Err(ApiError::BadRequest(
            "MCP not supported by this executor".to_string(),
        ));
    }

    // Resolve supplied config path or agent default
    let config_path = match coding_agent.default_mcp_config_path() {
        Some(path) => path,
        None => {
            return Err(ApiError::BadRequest(
                "Could not determine config file path".to_string(),
            ));
        }
    };

    let mut mcpc = coding_agent.get_mcp_config();
    let raw_config = read_agent_config(&config_path, &mcpc).await?;
    let servers = get_mcp_servers_from_config_path(&raw_config, &mcpc.servers_path);
    mcpc.set_servers(servers);
    Ok(ResponseJson(ApiResponse::success(GetMcpServerResponse {
        mcp_config: mcpc,
        config_path: config_path.to_string_lossy().to_string(),
    })))
}

async fn update_mcp_servers(
    State(_deployment): State<DeploymentImpl>,
    Query(query): Query<McpServerQuery>,
    Json(payload): Json<UpdateMcpServersBody>,
) -> Result<ResponseJson<ApiResponse<String>>, ApiError> {
    let profiles = ExecutorConfigs::get_cached();
    let agent = profiles
        .get_coding_agent(&ExecutorProfileId::new(query.executor))
        .ok_or(ConfigError::ValidationError(
            "Executor not found".to_string(),
        ))?;

    if !agent.supports_mcp() {
        return Err(ApiError::BadRequest(
            "This executor does not support MCP servers".to_string(),
        ));
    }

    // Resolve supplied config path or agent default
    let config_path = match agent.default_mcp_config_path() {
        Some(path) => path.to_path_buf(),
        None => {
            return Err(ApiError::BadRequest(
                "Could not determine config file path".to_string(),
            ));
        }
    };

    let mcpc = agent.get_mcp_config();
    match update_mcp_servers_in_config(&config_path, &mcpc, payload.servers).await {
        Ok(message) => Ok(ResponseJson(ApiResponse::success(message))),
        Err(e) => Err(ApiError::Internal(format!(
            "Failed to update MCP servers: {e}"
        ))),
    }
}

async fn update_mcp_servers_in_config(
    config_path: &std::path::Path,
    mcpc: &McpConfig,
    new_servers: HashMap<String, Value>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Ensure parent directory exists
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).await?;
    }
    // Read existing config (JSON or TOML depending on agent)
    let mut config = read_agent_config(config_path, mcpc).await?;

    // Get the current server count for comparison
    let old_servers = get_mcp_servers_from_config_path(&config, &mcpc.servers_path).len();

    // Set the MCP servers using the correct attribute path
    set_mcp_servers_in_config_path(&mut config, &mcpc.servers_path, &new_servers)?;

    // Write the updated config back to file (JSON or TOML depending on agent)
    write_agent_config(config_path, mcpc, &config).await?;

    let new_count = new_servers.len();
    let message = match (old_servers, new_count) {
        (0, 0) => "No MCP servers configured".to_string(),
        (0, n) => format!("Added {} MCP server(s)", n),
        (old, new) if old == new => format!("Updated MCP server configuration ({} server(s))", new),
        (old, new) => format!(
            "Updated MCP server configuration (was {}, now {})",
            old, new
        ),
    };

    Ok(message)
}

/// Helper function to get MCP servers from config using a path
fn get_mcp_servers_from_config_path(raw_config: &Value, path: &[String]) -> HashMap<String, Value> {
    let mut current = raw_config;
    for part in path {
        current = match current.get(part) {
            Some(val) => val,
            None => return HashMap::new(),
        };
    }
    // Extract the servers object
    match current.as_object() {
        Some(servers) => servers
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        None => HashMap::new(),
    }
}

/// Helper function to set MCP servers in config using a path
fn set_mcp_servers_in_config_path(
    raw_config: &mut Value,
    path: &[String],
    servers: &HashMap<String, Value>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if path.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "MCP servers path is empty",
        )
        .into());
    }

    // Ensure config is an object
    if !raw_config.is_object() {
        *raw_config = serde_json::json!({});
    }

    let mut current = raw_config;
    // Navigate/create the nested structure (all parts except the last)
    for part in &path[..path.len() - 1] {
        if !current.is_object() {
            *current = serde_json::json!({});
        }

        let obj = current.as_object_mut().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "MCP servers path traverses a non-object",
            )
        })?;

        let entry = obj
            .entry(part.to_string())
            .or_insert_with(|| serde_json::json!({}));
        if !entry.is_object() {
            *entry = serde_json::json!({});
        }
        current = entry;
    }

    // Set the final attribute
    if !current.is_object() {
        *current = serde_json::json!({});
    }

    let final_attr = path
        .last()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "empty path"))?;
    let obj = current.as_object_mut().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "MCP servers path traverses a non-object",
        )
    })?;
    obj.insert(final_attr.to_string(), serde_json::to_value(servers)?);

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfilesContent {
    pub content: String,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct ImportLlmanProfilesResponse {
    pub path: String,
    pub imported: usize,
    pub updated: usize,
    pub skipped: usize,
}

async fn get_profiles(
    State(_deployment): State<DeploymentImpl>,
) -> ResponseJson<ApiResponse<ProfilesContent>> {
    let profiles_path = utils::assets::profiles_path();

    // Use cached data to ensure consistency with runtime and PUT updates
    let profiles = ExecutorConfigs::get_cached();

    let content = serde_json::to_string_pretty(&profiles).unwrap_or_else(|e| {
        tracing::error!("Failed to serialize profiles to JSON: {}", e);
        serde_json::to_string_pretty(&ExecutorConfigs::from_defaults())
            .unwrap_or_else(|_| "{}".to_string())
    });

    ResponseJson(ApiResponse::success(ProfilesContent {
        content,
        path: profiles_path.display().to_string(),
    }))
}

async fn update_profiles(
    State(_deployment): State<DeploymentImpl>,
    body: String,
) -> Result<ResponseJson<ApiResponse<String>>, ApiError> {
    // Try to parse as ExecutorProfileConfigs format
    let executor_profiles = serde_json::from_str::<ExecutorConfigs>(&body)
        .map_err(|e| ApiError::BadRequest(format!("Invalid executor profiles format: {e}")))?;

    executor_profiles.save_overrides().map_err(|e| match e {
        executors::profile::ProfileError::Validation(msg) => ApiError::BadRequest(msg),
        executors::profile::ProfileError::CannotDeleteExecutor { executor } => {
            ApiError::BadRequest(format!("Built-in executor '{executor}' cannot be deleted"))
        }
        executors::profile::ProfileError::CannotDeleteBuiltInConfig { executor, variant } => {
            ApiError::BadRequest(format!(
                "Built-in configuration '{executor}:{variant}' cannot be deleted"
            ))
        }
        executors::profile::ProfileError::Io(err) => ApiError::Io(err),
        _ => ApiError::Internal(format!("Failed to save executor profiles: {e}")),
    })?;

    tracing::info!("Executor profiles saved successfully");
    ExecutorConfigs::reload();

    Ok(ResponseJson(ApiResponse::success(
        "Executor profiles updated successfully".to_string(),
    )))
}

#[derive(Debug, Default)]
struct LlmanImportSummary {
    imported: usize,
    updated: usize,
    skipped: usize,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct ResolveLlmanPathResponse {
    pub path: Option<String>,
}

async fn resolve_llman_path(
    State(deployment): State<DeploymentImpl>,
) -> ResponseJson<ApiResponse<ResolveLlmanPathResponse>> {
    let config = deployment.config().read().await;
    let path = llman::resolve_claude_code_config_path(config.llman_claude_code_path.as_deref());

    ResponseJson(ApiResponse::success(ResolveLlmanPathResponse {
        path: path.map(|path| path.display().to_string()),
    }))
}

async fn import_llman_profiles(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<ImportLlmanProfilesResponse>>, ApiError> {
    let config = deployment.config().read().await;
    let config_path =
        llman::resolve_claude_code_config_path(config.llman_claude_code_path.as_deref());
    let Some(config_path) = config_path else {
        return Err(ApiError::BadRequest(
            "Could not resolve LLMAN config path".to_string(),
        ));
    };

    let groups = match llman::read_claude_code_groups(&config_path).await {
        Ok(groups) => groups,
        Err(e) => {
            return Err(ApiError::Internal(format!(
                "Failed to read LLMAN config: {e}"
            )));
        }
    };

    let mut profiles = ExecutorConfigs::get_cached();
    let summary = apply_llman_groups_to_profiles(&mut profiles, &groups);

    if let Err(e) = profiles.save_overrides() {
        return Err(ApiError::Internal(format!(
            "Failed to save executor profiles: {e}"
        )));
    }

    ExecutorConfigs::reload();

    Ok(ResponseJson(ApiResponse::success(
        ImportLlmanProfilesResponse {
            path: config_path.display().to_string(),
            imported: summary.imported,
            updated: summary.updated,
            skipped: summary.skipped,
        },
    )))
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct CheckEditorAvailabilityQuery {
    editor_type: EditorType,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct CheckEditorAvailabilityResponse {
    available: bool,
}

async fn check_editor_availability(
    State(_deployment): State<DeploymentImpl>,
    Query(query): Query<CheckEditorAvailabilityQuery>,
) -> ResponseJson<ApiResponse<CheckEditorAvailabilityResponse>> {
    // Construct a minimal EditorConfig for checking
    let editor_config = EditorConfig::new(
        query.editor_type,
        None, // custom_command
        None, // remote_ssh_host
        None, // remote_ssh_user
    );

    let available = editor_config.check_availability().await;
    ResponseJson(ApiResponse::success(CheckEditorAvailabilityResponse {
        available,
    }))
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct CheckAgentAvailabilityQuery {
    executor: BaseCodingAgent,
}

async fn check_agent_availability(
    State(_deployment): State<DeploymentImpl>,
    Query(query): Query<CheckAgentAvailabilityQuery>,
) -> ResponseJson<ApiResponse<AvailabilityInfo>> {
    let profiles = ExecutorConfigs::get_cached();
    let profile_id = ExecutorProfileId::new(query.executor);

    let info = match profiles.get_coding_agent(&profile_id) {
        Some(agent) => agent.get_availability_info(),
        None => AvailabilityInfo::NotFound,
    };

    ResponseJson(ApiResponse::success(info))
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct CliDependencyPreflightQuery {
    pub executor: BaseCodingAgent,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type", rename_all = "snake_case")]
pub enum GhCliPreflightStatus {
    Ready,
    NotInstalled,
    NotAuthenticated,
    Error { message: String },
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct CliDependencyPreflightResponse {
    pub agent: AvailabilityInfo,
    pub github_cli: GhCliPreflightStatus,
}

fn map_github_cli_preflight_status(
    err: &services::services::github::GitHubServiceError,
) -> GhCliPreflightStatus {
    match err {
        services::services::github::GitHubServiceError::GhCliNotInstalled(_) => {
            GhCliPreflightStatus::NotInstalled
        }
        services::services::github::GitHubServiceError::AuthFailed(_) => {
            GhCliPreflightStatus::NotAuthenticated
        }
        _ => GhCliPreflightStatus::Error {
            message: err.to_string(),
        },
    }
}

async fn cli_dependency_preflight(
    Query(query): Query<CliDependencyPreflightQuery>,
) -> ResponseJson<ApiResponse<CliDependencyPreflightResponse>> {
    let profiles = ExecutorConfigs::get_cached();
    let profile_id = ExecutorProfileId::new(query.executor);

    let agent = match profiles.get_coding_agent(&profile_id) {
        Some(agent) => agent.get_availability_info(),
        None => AvailabilityInfo::NotFound,
    };

    let github_cli = match GitHubService::new() {
        Ok(service) => match service.check_token().await {
            Ok(_) => GhCliPreflightStatus::Ready,
            Err(err) => map_github_cli_preflight_status(&err),
        },
        Err(err) => GhCliPreflightStatus::Error {
            message: err.to_string(),
        },
    };

    ResponseJson(ApiResponse::success(CliDependencyPreflightResponse {
        agent,
        github_cli,
    }))
}

fn apply_llman_groups_to_profiles(
    profiles: &mut ExecutorConfigs,
    groups: &HashMap<String, HashMap<String, String>>,
) -> LlmanImportSummary {
    let mut summary = LlmanImportSummary::default();
    let Some(claude_profile) = profiles.executors.get_mut(&BaseCodingAgent::ClaudeCode) else {
        return summary;
    };

    let default_config = claude_profile.get_default().cloned();

    for (group_name, env) in groups {
        let variant_key = format!("LLMAN_{}", canonical_variant_key(group_name));

        if let Some(existing) = claude_profile.configurations.get_mut(&variant_key) {
            if let CodingAgent::ClaudeCode(config) = existing {
                config.cmd.env = Some(env.clone());
                summary.updated += 1;
            } else {
                summary.skipped += 1;
            }
            continue;
        }

        let Some(mut new_config) = default_config.clone() else {
            summary.skipped += 1;
            continue;
        };

        if let CodingAgent::ClaudeCode(config) = &mut new_config {
            config.cmd.env = Some(env.clone());
            claude_profile
                .configurations
                .insert(variant_key, new_config);
            summary.imported += 1;
        } else {
            summary.skipped += 1;
        }
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_updates_existing_and_adds_new_variants() {
        let mut profiles = ExecutorConfigs::from_defaults();
        let claude = profiles
            .executors
            .get_mut(&BaseCodingAgent::ClaudeCode)
            .expect("claude profile");

        let mut existing = claude
            .get_default()
            .cloned()
            .expect("default claude config");
        if let CodingAgent::ClaudeCode(config) = &mut existing {
            config.model = Some("keep-model".to_string());
            config.cmd.env = Some(HashMap::from([("OLD_KEY".to_string(), "old".to_string())]));
        }
        claude
            .configurations
            .insert("LLMAN_MINIMAX".to_string(), existing);

        let groups = HashMap::from([
            (
                "minimax".to_string(),
                HashMap::from([("NEW_KEY".to_string(), "new".to_string())]),
            ),
            (
                "glm-cost".to_string(),
                HashMap::from([("TOKEN".to_string(), "abc".to_string())]),
            ),
        ]);

        let summary = apply_llman_groups_to_profiles(&mut profiles, &groups);
        assert_eq!(summary.updated, 1);
        assert_eq!(summary.imported, 1);
        assert_eq!(summary.skipped, 0);

        let claude = profiles
            .executors
            .get(&BaseCodingAgent::ClaudeCode)
            .expect("claude profile");

        let updated = claude
            .configurations
            .get("LLMAN_MINIMAX")
            .expect("updated variant");
        if let CodingAgent::ClaudeCode(config) = updated {
            assert_eq!(config.model.as_deref(), Some("keep-model"));
            let env = config.cmd.env.as_ref().expect("env map");
            assert_eq!(env.get("NEW_KEY"), Some(&"new".to_string()));
            assert!(!env.contains_key("OLD_KEY"));
        } else {
            panic!("expected ClaudeCode variant");
        }

        let added = claude
            .configurations
            .get("LLMAN_GLM_COST")
            .expect("added variant");
        if let CodingAgent::ClaudeCode(config) = added {
            let env = config.cmd.env.as_ref().expect("env map");
            assert_eq!(env.get("TOKEN"), Some(&"abc".to_string()));
        } else {
            panic!("expected ClaudeCode variant");
        }
    }

    #[test]
    fn github_cli_preflight_maps_statuses() {
        use services::services::github::{GhCliError, GitHubServiceError};

        let not_installed = GitHubServiceError::GhCliNotInstalled(GhCliError::NotAvailable);
        assert!(matches!(
            map_github_cli_preflight_status(&not_installed),
            GhCliPreflightStatus::NotInstalled
        ));

        let not_authenticated =
            GitHubServiceError::AuthFailed(GhCliError::AuthFailed("auth failed".to_string()));
        assert!(matches!(
            map_github_cli_preflight_status(&not_authenticated),
            GhCliPreflightStatus::NotAuthenticated
        ));

        let other = GitHubServiceError::Repository("unexpected".to_string());
        match map_github_cli_preflight_status(&other) {
            GhCliPreflightStatus::Error { message } => {
                assert!(message.contains("unexpected"));
            }
            other => panic!("expected error mapping, got {other:?}"),
        }
    }

    #[test]
    fn set_mcp_servers_in_config_path_rejects_empty_path() {
        let mut raw_config = serde_json::json!({});
        let result = set_mcp_servers_in_config_path(&mut raw_config, &[], &HashMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn set_mcp_servers_in_config_path_overwrites_non_object_nodes() {
        let mut raw_config = serde_json::json!({
            "outer": "nope"
        });
        let mut servers = HashMap::new();
        servers.insert(
            "local".to_string(),
            serde_json::json!({ "command": "tool" }),
        );

        set_mcp_servers_in_config_path(
            &mut raw_config,
            &["outer".to_string(), "mcpServers".to_string()],
            &servers,
        )
        .expect("should coerce non-object nodes");

        let outer = raw_config
            .get("outer")
            .and_then(Value::as_object)
            .expect("outer object");
        let mcp_servers = outer
            .get("mcpServers")
            .and_then(Value::as_object)
            .expect("mcpServers object");
        assert!(mcp_servers.contains_key("local"));
    }
}
