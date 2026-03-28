use std::{
    collections::HashMap,
    sync::{LazyLock, RwLock},
};

use app_runtime::{Deployment, DeploymentError};
use axum::{
    Router,
    body::Body,
    extract::{Path, Query, State},
    http,
    response::{Json as ResponseJson, Response},
    routing::{get, post, put},
};
use config::{
    Config, SoundFile,
    editor::{EditorConfig, EditorType},
};
use executors::{
    agent_command::{AgentCommandResolution, agent_command_resolver},
    executors::{AvailabilityInfo, BaseAgentCapability, CodingAgent, StandardCodingAgentExecutor},
    llman,
    profile::ExecutorConfigs,
};
use executors_protocol::{BaseCodingAgent, ExecutorProfileId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;
use utils_core::response::ApiResponse;

use crate::{DeploymentImpl, error::ApiError};

fn is_sensitive_env_key(key: &str) -> bool {
    let upper = key.trim().to_ascii_uppercase();
    upper.contains("TOKEN")
        || upper.contains("PASSWORD")
        || upper.contains("PASSWD")
        || upper.contains("SECRET")
        || upper.contains("PAT")
        || upper.ends_with("_KEY")
        || upper.contains("API_KEY")
        || upper.contains("ACCESS_KEY")
        || upper.contains("PRIVATE_KEY")
}

fn redact_secrets_in_env_objects(value: &mut Value) {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        Value::Array(items) => {
            for item in items {
                redact_secrets_in_env_objects(item);
            }
        }
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                // Avoid leaking executor command overrides (they may contain tokens, secrets, etc.).
                if key == "base_command_override" && matches!(value, Value::String(_)) {
                    *value = Value::String("<redacted>".to_string());
                }
                if key == "additional_params" {
                    match value {
                        Value::Array(items) => {
                            for item in items.iter_mut() {
                                if matches!(item, Value::String(_)) {
                                    *item = Value::String("<redacted>".to_string());
                                }
                            }
                        }
                        Value::String(_) => {
                            *value = Value::String("<redacted>".to_string());
                        }
                        _ => {}
                    }
                }

                if key == "env"
                    && let Value::Object(env) = value
                {
                    for (env_key, env_value) in env.iter_mut() {
                        if is_sensitive_env_key(env_key) {
                            *env_value = Value::String("<redacted>".to_string());
                        }
                    }
                }
                redact_secrets_in_env_objects(value);
            }
        }
    }
}

fn redacted_executor_configs_for_api(profiles: &ExecutorConfigs) -> ExecutorConfigs {
    let mut value = match serde_json::to_value(profiles) {
        Ok(value) => value,
        Err(err) => {
            tracing::error!("Failed to serialize profiles to JSON for redaction: {err}");
            return ExecutorConfigs::from_defaults();
        }
    };
    redact_secrets_in_env_objects(&mut value);

    serde_json::from_value::<ExecutorConfigs>(value).unwrap_or_else(|err| {
        tracing::error!("Failed to deserialize redacted profiles JSON: {err}");
        ExecutorConfigs::from_defaults()
    })
}

static REDACTED_EXECUTOR_CONFIGS_CACHE: LazyLock<RwLock<Option<(u64, ExecutorConfigs)>>> =
    LazyLock::new(|| RwLock::new(None));

fn redacted_executor_configs_for_api_cached(
    loaded_at_unix_ms: u64,
    profiles: &ExecutorConfigs,
) -> ExecutorConfigs {
    {
        let cache = REDACTED_EXECUTOR_CONFIGS_CACHE.read().unwrap();
        if let Some((cached_loaded_at, cached_redacted)) = cache.as_ref()
            && *cached_loaded_at == loaded_at_unix_ms
        {
            return cached_redacted.clone();
        }
    }

    let redacted = redacted_executor_configs_for_api(profiles);
    let mut cache = REDACTED_EXECUTOR_CONFIGS_CACHE.write().unwrap();
    *cache = Some((loaded_at_unix_ms, redacted.clone()));
    redacted
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/info", get(get_user_system_info))
        .route("/config/status", get(get_config_status))
        .route("/config/reload", post(reload_config))
        .route("/config", put(update_config))
        .route("/sounds/{sound}", get(get_sound))
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

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
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

    pub fn cached() -> Self {
        static ENV: LazyLock<Environment> = LazyLock::new(Environment::new);
        ENV.clone()
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
    // Use the in-memory non-templated view of config for API responses to avoid leaking expanded
    // secrets and to keep the response consistent with the last successfully loaded runtime config.
    let mut redacted_config = deployment.public_config().read().await.clone();
    redacted_config.access_control.token = None;
    redacted_config.github.pat = None;
    redacted_config.github.oauth_token = None;

    let loaded_at_unix_ms = to_unix_ms(deployment.config_status().read().await.loaded_at);
    let profiles = ExecutorConfigs::get_cached();
    let redacted_profiles = redacted_executor_configs_for_api_cached(loaded_at_unix_ms, &profiles);

    let user_system_info = UserSystemInfo {
        config: redacted_config,
        profiles: redacted_profiles,
        environment: Environment::cached(),
        capabilities: {
            let mut caps: HashMap<String, Vec<BaseAgentCapability>> = HashMap::new();
            for key in profiles.executors.keys() {
                if let Some(agent) = profiles.get_coding_agent(&ExecutorProfileId::new(*key)) {
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
    pub projects_path: String,
    pub projects_dir: String,
    pub secret_env_path: String,
    pub schema_path: String,
    pub projects_schema_path: String,
    #[ts(type = "number")]
    pub loaded_at_unix_ms: u64,
    pub last_error: Option<String>,
    pub dirty: bool,
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
        projects_path: utils_core::vk_projects_yaml_path()
            .to_string_lossy()
            .to_string(),
        projects_dir: utils_core::vk_projects_dir().to_string_lossy().to_string(),
        secret_env_path: status.secret_env_path.to_string_lossy().to_string(),
        schema_path: utils_core::vk_config_schema_path()
            .to_string_lossy()
            .to_string(),
        projects_schema_path: utils_core::vk_projects_schema_path()
            .to_string_lossy()
            .to_string(),
        loaded_at_unix_ms: to_unix_ms(status.loaded_at),
        last_error: status.last_error,
        dirty: status.dirty,
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

    let status = deployment.config_status().read().await.clone();
    let response = ConfigStatusResponse {
        config_dir: status.config_dir.to_string_lossy().to_string(),
        config_path: status.config_path.to_string_lossy().to_string(),
        projects_path: utils_core::vk_projects_yaml_path()
            .to_string_lossy()
            .to_string(),
        projects_dir: utils_core::vk_projects_dir().to_string_lossy().to_string(),
        secret_env_path: status.secret_env_path.to_string_lossy().to_string(),
        schema_path: utils_core::vk_config_schema_path()
            .to_string_lossy()
            .to_string(),
        projects_schema_path: utils_core::vk_projects_schema_path()
            .to_string_lossy()
            .to_string(),
        loaded_at_unix_ms: to_unix_ms(status.loaded_at),
        last_error: status.last_error,
        dirty: status.dirty,
    };

    Ok(ResponseJson(ApiResponse::success(response)))
}

fn settings_write_disabled() -> (http::StatusCode, ResponseJson<ApiResponse<()>>) {
    (
        http::StatusCode::METHOD_NOT_ALLOWED,
        ResponseJson(ApiResponse::<()>::error(
            "已禁用通过 API 写入 settings：请编辑 `config.yaml` / `projects.yaml` + reload（POST /api/config/reload）。",
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
    State(deployment): State<DeploymentImpl>,
) -> ResponseJson<ApiResponse<ProfilesContent>> {
    let profiles_path = utils_core::vk_config_yaml_path();

    // Use cached data to ensure consistency with runtime and PUT updates
    let loaded_at_unix_ms = to_unix_ms(deployment.config_status().read().await.loaded_at);
    let profiles = ExecutorConfigs::get_cached();
    let redacted = redacted_executor_configs_for_api_cached(loaded_at_unix_ms, &profiles);

    let content = serde_json::to_string_pretty(&redacted).unwrap_or_else(|e| {
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

    let mut compat = compat;
    // This response is used for UI diagnostics. Do not leak the resolved base command, which may
    // embed tokens (e.g. in overrides/params). Keep the message but redact the base command line.
    compat.base_command = "<redacted>".to_string();
    if let Some(message) = compat.message.as_mut() {
        let redacted = message
            .lines()
            .map(|line| {
                if line.starts_with("Base command: ") {
                    "Base command: <redacted>"
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        *message = redacted;
    }

    Ok(ResponseJson(ApiResponse::success(compat)))
}

fn now_unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn program_runnable(program: &str, args: &[&str], path_override: Option<&std::ffi::OsStr>) -> bool {
    let mut cmd = std::process::Command::new(program);
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(path) = path_override {
        cmd.env("PATH", path);
    }
    cmd.output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn git_availability(path_override: Option<&std::ffi::OsStr>) -> AvailabilityInfo {
    if program_runnable("git", &["--version"], path_override) {
        AvailabilityInfo::InstallationFound
    } else {
        AvailabilityInfo::NotFound
    }
}

fn gh_availability(path_override: Option<&std::ffi::OsStr>) -> AvailabilityInfo {
    if !program_runnable("gh", &["--version"], path_override) {
        return AvailabilityInfo::NotFound;
    }

    // Treat "installed but unauthenticated" as InstallationFound.
    // When authenticated, report LoginDetected with best-effort timestamp.
    if program_runnable("gh", &["auth", "status"], path_override) {
        AvailabilityInfo::LoginDetected {
            last_auth_timestamp: now_unix_timestamp(),
        }
    } else {
        AvailabilityInfo::InstallationFound
    }
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct CliDependencyPreflightQuery {
    pub executor: BaseCodingAgent,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct CliDependencyPreflightResponse {
    pub agent: AvailabilityInfo,
    pub git: AvailabilityInfo,
    pub gh: AvailabilityInfo,
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

    ResponseJson(ApiResponse::success(CliDependencyPreflightResponse {
        agent,
        git: git_availability(None),
        gh: gh_availability(None),
    }))
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, fs};

    use test_support::TestEnv;

    use super::*;

    #[test]
    fn profiles_endpoint_redacts_sensitive_env_values() {
        let profiles: ExecutorConfigs = serde_json::from_value(serde_json::json!({
          "executors": {
            "CLAUDE_CODE": {
              "DEFAULT": {
                "CLAUDE_CODE": {
                  "env": {
                    "ANTHROPIC_AUTH_TOKEN": "sk-test",
                    "GITHUB_PAT": "ghp_test",
                    "PLAIN_FLAG": "1"
                  }
                }
              }
            }
          }
        }))
        .expect("deserialize profiles");

        let redacted = redacted_executor_configs_for_api(&profiles);
        let json = serde_json::to_value(redacted).expect("serialize redacted");

        assert_eq!(
            json.pointer("/executors/CLAUDE_CODE/DEFAULT/CLAUDE_CODE/env/ANTHROPIC_AUTH_TOKEN")
                .and_then(|v| v.as_str()),
            Some("<redacted>")
        );
        assert_eq!(
            json.pointer("/executors/CLAUDE_CODE/DEFAULT/CLAUDE_CODE/env/GITHUB_PAT")
                .and_then(|v| v.as_str()),
            Some("<redacted>")
        );
        assert_eq!(
            json.pointer("/executors/CLAUDE_CODE/DEFAULT/CLAUDE_CODE/env/PLAIN_FLAG")
                .and_then(|v| v.as_str()),
            Some("1")
        );
    }

    #[test]
    fn cli_preflight_reports_git_unavailable_when_not_on_path() {
        let env_guard = TestEnv::new("vk-test-");
        let empty_dir = env_guard.temp_root().join("empty-path");
        fs::create_dir_all(&empty_dir).unwrap();

        let path = OsString::from(empty_dir.as_os_str());
        assert!(matches!(
            git_availability(Some(&path)),
            AvailabilityInfo::NotFound
        ));
    }

    #[test]
    fn cli_preflight_reports_gh_not_found_when_missing() {
        let env_guard = TestEnv::new("vk-test-");
        let empty_dir = env_guard.temp_root().join("empty-path-gh");
        fs::create_dir_all(&empty_dir).unwrap();

        let path = OsString::from(empty_dir.as_os_str());
        assert!(matches!(
            gh_availability(Some(&path)),
            AvailabilityInfo::NotFound
        ));
    }

    #[test]
    fn cli_preflight_reports_gh_unauthenticated_when_installed_but_not_logged_in() {
        let env_guard = TestEnv::new("vk-test-");
        let bin_dir = env_guard.temp_root().join("fake-gh-unauth");
        fs::create_dir_all(&bin_dir).unwrap();

        #[cfg(windows)]
        let gh_path = bin_dir.join("gh.bat");
        #[cfg(not(windows))]
        let gh_path = bin_dir.join("gh");

        #[cfg(windows)]
        fs::write(
            &gh_path,
            "@echo off\r\nif \"%1\"==\"--version\" exit /b 0\r\nif \"%1\"==\"auth\" exit /b 1\r\nexit /b 0\r\n",
        )
        .unwrap();

        #[cfg(not(windows))]
        {
            fs::write(
                &gh_path,
                "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then exit 0; fi\nif [ \"$1\" = \"auth\" ]; then exit 1; fi\nexit 0\n",
            )
            .unwrap();
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&gh_path).unwrap().permissions();
            perms.set_mode(0o700);
            fs::set_permissions(&gh_path, perms).unwrap();
        }

        let path = OsString::from(bin_dir.as_os_str());
        assert!(matches!(
            gh_availability(Some(&path)),
            AvailabilityInfo::InstallationFound
        ));
    }

    #[test]
    fn cli_preflight_reports_gh_authenticated_when_logged_in() {
        let env_guard = TestEnv::new("vk-test-");
        let bin_dir = env_guard.temp_root().join("fake-gh-auth");
        fs::create_dir_all(&bin_dir).unwrap();

        #[cfg(windows)]
        let gh_path = bin_dir.join("gh.bat");
        #[cfg(not(windows))]
        let gh_path = bin_dir.join("gh");

        #[cfg(windows)]
        fs::write(
            &gh_path,
            "@echo off\r\nif \"%1\"==\"--version\" exit /b 0\r\nif \"%1\"==\"auth\" exit /b 0\r\nexit /b 0\r\n",
        )
        .unwrap();

        #[cfg(not(windows))]
        {
            fs::write(
                &gh_path,
                "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then exit 0; fi\nif [ \"$1\" = \"auth\" ]; then exit 0; fi\nexit 0\n",
            )
            .unwrap();
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&gh_path).unwrap().permissions();
            perms.set_mode(0o700);
            fs::set_permissions(&gh_path, perms).unwrap();
        }

        let path = OsString::from(bin_dir.as_os_str());
        assert!(matches!(
            gh_availability(Some(&path)),
            AvailabilityInfo::LoginDetected { .. }
        ));
    }

    #[tokio::test]
    async fn cli_preflight_endpoint_includes_git_and_gh() {
        let ResponseJson(resp) = cli_dependency_preflight(Query(CliDependencyPreflightQuery {
            executor: BaseCodingAgent::Codex,
        }))
        .await;
        assert!(resp.is_success());
        let data = resp.into_data().expect("data");
        assert!(matches!(data.git, AvailabilityInfo::InstallationFound));
        assert!(matches!(
            data.gh,
            AvailabilityInfo::NotFound
                | AvailabilityInfo::InstallationFound
                | AvailabilityInfo::LoginDetected { .. }
        ));
    }
}
