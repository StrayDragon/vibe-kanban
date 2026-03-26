use std::collections::HashMap;

use app_runtime::{Deployment, DeploymentError};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http,
    response::{Json as ResponseJson, Response},
    routing::{get, post, put},
};
use config::{
    Config, ConfigError, SoundFile,
    editor::{EditorConfig, EditorType},
};
use execution::github::GitHubService;
use executors::{
    agent_command::{AgentCommandResolution, agent_command_resolver},
    executors::{AvailabilityInfo, BaseAgentCapability, CodingAgent, StandardCodingAgentExecutor},
    llman,
    mcp_config::{McpConfig, read_agent_config},
    profile::ExecutorConfigs,
};
use executors_protocol::{BaseCodingAgent, ExecutorProfileId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;
use utils_core::response::ApiResponse;

use crate::{DeploymentImpl, error::ApiError};

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/info", get(get_user_system_info))
        .route("/config/status", get(get_config_status))
        .route("/config/reload", post(reload_config))
        .route("/config/open", post(open_config_target))
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
        .route(
            "/agents/check-compatibility",
            get(check_agent_compatibility),
        )
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
    redacted_config.github.pat = None;
    redacted_config.github.oauth_token = None;

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

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct ConfigStatusResponse {
    pub config_dir: String,
    pub config_path: String,
    pub secret_env_path: String,
    pub schema_path: String,
    pub loaded_at_unix_ms: u64,
    pub last_error: Option<String>,
}

fn to_unix_ms(time: std::time::SystemTime) -> u64 {
    time.duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[axum::debug_handler]
async fn get_config_status(
    State(deployment): State<DeploymentImpl>,
) -> ResponseJson<ApiResponse<ConfigStatusResponse>> {
    let status = deployment.config_status().read().await.clone();
    let response = ConfigStatusResponse {
        config_dir: status.config_dir.to_string_lossy().to_string(),
        config_path: status.config_path.to_string_lossy().to_string(),
        secret_env_path: status.secret_env_path.to_string_lossy().to_string(),
        schema_path: utils_core::vk_config_schema_path()
            .to_string_lossy()
            .to_string(),
        loaded_at_unix_ms: to_unix_ms(status.loaded_at),
        last_error: status.last_error,
    };

    ResponseJson(ApiResponse::success(response))
}

#[axum::debug_handler]
async fn reload_config(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<ConfigStatusResponse>>, ApiError> {
    if let Err(err) = deployment.reload_user_config().await {
        return Err(ApiError::BadRequest(format!("Config reload failed: {err}")));
    }

    deployment.sync_config_projects_to_db().await?;

    let status = deployment.config_status().read().await.clone();
    let response = ConfigStatusResponse {
        config_dir: status.config_dir.to_string_lossy().to_string(),
        config_path: status.config_path.to_string_lossy().to_string(),
        secret_env_path: status.secret_env_path.to_string_lossy().to_string(),
        schema_path: utils_core::vk_config_schema_path()
            .to_string_lossy()
            .to_string(),
        loaded_at_unix_ms: to_unix_ms(status.loaded_at),
        last_error: status.last_error,
    };

    Ok(ResponseJson(ApiResponse::success(response)))
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OpenConfigTarget {
    ConfigDir,
    ConfigYaml,
    SecretEnv,
    Schema,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct OpenConfigTargetRequest {
    pub target: OpenConfigTarget,
    pub editor_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct OpenConfigTargetResponse {
    pub url: Option<String>,
}

#[axum::debug_handler]
async fn open_config_target(
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<OpenConfigTargetRequest>,
) -> Result<ResponseJson<ApiResponse<OpenConfigTargetResponse>>, ApiError> {
    let status = deployment.config_status().read().await.clone();
    let path = match payload.target {
        OpenConfigTarget::ConfigDir => status.config_dir,
        OpenConfigTarget::ConfigYaml => status.config_path,
        OpenConfigTarget::SecretEnv => status.secret_env_path,
        OpenConfigTarget::Schema => utils_core::vk_config_schema_path(),
    };

    let editor_config = {
        let config = deployment.config().read().await;
        if config.editor.is_integration_disabled() {
            return Err(ApiError::BadRequest(
                "Editor integration is disabled".to_string(),
            ));
        }
        config.editor.with_override(payload.editor_type.as_deref())
    };

    let url = editor_config.open_file(&path).await?;
    Ok(ResponseJson(ApiResponse::success(
        OpenConfigTargetResponse { url },
    )))
}

fn settings_write_disabled() -> (http::StatusCode, ResponseJson<ApiResponse<()>>) {
    (
        http::StatusCode::METHOD_NOT_ALLOWED,
        ResponseJson(ApiResponse::<()>::error(
            "已禁用通过 API 写入 settings：请编辑 `config.yaml` + reload（POST /api/config/reload）。",
        )),
    )
}

async fn update_config() -> (http::StatusCode, ResponseJson<ApiResponse<()>>) {
    settings_write_disabled()
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

async fn update_mcp_servers() -> (http::StatusCode, ResponseJson<ApiResponse<()>>) {
    settings_write_disabled()
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
    let profiles_path = utils_core::vk_config_yaml_path();

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

async fn update_profiles() -> (http::StatusCode, ResponseJson<ApiResponse<()>>) {
    settings_write_disabled()
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
    State(_deployment): State<DeploymentImpl>,
) -> (http::StatusCode, ResponseJson<ApiResponse<()>>) {
    settings_write_disabled()
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
pub struct CheckAgentCompatibilityQuery {
    executor: BaseCodingAgent,
    #[serde(default)]
    variant: Option<String>,
    #[serde(default)]
    refresh: Option<bool>,
}

async fn check_agent_compatibility(
    State(_deployment): State<DeploymentImpl>,
    Query(query): Query<CheckAgentCompatibilityQuery>,
) -> Result<
    ResponseJson<
        ApiResponse<executors::executors::codex::compatibility::CodexProtocolCompatibility>,
    >,
    ApiError,
> {
    if query.executor != BaseCodingAgent::Codex {
        return Err(ApiError::BadRequest(
            "Compatibility check is only supported for Codex".to_string(),
        ));
    }

    let profiles = ExecutorConfigs::get_cached();
    let profile_id = ExecutorProfileId {
        executor: query.executor,
        variant: query.variant.clone(),
    };

    let Some(coding_agent) = profiles.get_coding_agent(&profile_id) else {
        return Err(ApiError::BadRequest("Executor not found".to_string()));
    };

    let CodingAgent::Codex(config) = coding_agent else {
        return Err(ApiError::BadRequest("Executor is not Codex".to_string()));
    };

    let compat = executors::executors::codex::compatibility::check_codex_protocol_compatibility(
        &config.cmd,
        config.oss.unwrap_or(false),
        query.refresh.unwrap_or(false),
    )
    .await;

    Ok(ResponseJson(ApiResponse::success(compat)))
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
    err: &execution::github::GitHubServiceError,
) -> GhCliPreflightStatus {
    match err {
        execution::github::GitHubServiceError::GhCliNotInstalled(_) => {
            GhCliPreflightStatus::NotInstalled
        }
        execution::github::GitHubServiceError::AuthFailed(_) => {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_cli_preflight_maps_statuses() {
        use execution::github::{GhCliError, GitHubServiceError};

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
}
