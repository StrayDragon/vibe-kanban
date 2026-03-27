use axum::{
    Router,
    http::StatusCode,
    middleware::from_fn_with_state,
    routing::{any, get},
};

use crate::{DeploymentImpl, routes};

mod auth;
mod frontend;

pub fn router(deployment: DeploymentImpl) -> Router {
    let api_routes = Router::new()
        .merge(routes::config::router())
        .merge(routes::containers::router(&deployment))
        .merge(routes::projects::router(&deployment))
        .merge(routes::tasks::router(&deployment))
        .merge(routes::archived_kanbans::router(&deployment))
        .merge(routes::milestones::router(&deployment))
        .merge(routes::task_attempts::router(&deployment))
        .merge(routes::execution_processes::router(&deployment))
        .merge(routes::tags::router(&deployment))
        .merge(routes::filesystem::router())
        .merge(routes::repo::router())
        .merge(routes::events::router(&deployment))
        .merge(routes::approvals::router())
        .merge(routes::scratch::router(&deployment))
        .merge(routes::sessions::router(&deployment))
        .merge(routes::translation::router())
        .nest("/images", routes::images::routes())
        .route("/{*path}", any(|| async { StatusCode::NOT_FOUND }))
        .layer(from_fn_with_state(
            deployment.clone(),
            auth::require_api_auth,
        ));

    Router::new()
        .route("/health", get(routes::health::health_check))
        .nest("/api", api_routes)
        .route("/", get(frontend::serve_frontend_root))
        .route("/{*path}", get(frontend::serve_frontend))
        .with_state(deployment)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        net::{IpAddr, Ipv4Addr, SocketAddr},
    };

    use app_runtime::Deployment;
    use axum::{
        body::{Body, to_bytes},
        extract::ConnectInfo,
        http::{Request, StatusCode, header},
    };
    use config::AccessControlMode;
    use tower::ServiceExt;
    use uuid::Uuid;

    use crate::{DeploymentImpl, test_support::TestEnvGuard};

    async fn setup_deployment() -> (TestEnvGuard, DeploymentImpl) {
        let temp_root = std::env::temp_dir().join(format!("vk-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let env_guard = TestEnvGuard::new(&temp_root, db_url);

        let deployment = DeploymentImpl::new().await.unwrap();

        (env_guard, deployment)
    }

    async fn set_token_boundary(
        deployment: &DeploymentImpl,
        token: &str,
        allow_localhost_bypass: bool,
    ) {
        let mut config = deployment.config().write().await;
        config.access_control.mode = AccessControlMode::Token;
        config.access_control.token = Some(token.to_string());
        config.access_control.allow_localhost_bypass = allow_localhost_bypass;
    }

    async fn set_misconfigured_token_boundary(deployment: &DeploymentImpl) {
        let mut config = deployment.config().write().await;
        config.access_control.mode = AccessControlMode::Token;
        config.access_control.token = None;
        config.access_control.allow_localhost_bypass = false;
    }

    async fn set_workspace_dir(deployment: &DeploymentImpl, workspace_dir: &std::path::Path) {
        let mut config = deployment.config().write().await;
        config.workspace_dir = Some(workspace_dir.to_string_lossy().to_string());
    }

    fn loopback_connect_info() -> ConnectInfo<SocketAddr> {
        ConnectInfo(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            12345,
        ))
    }

    #[tokio::test]
    async fn health_remains_public_in_token_mode() {
        let (_env_guard, deployment) = setup_deployment().await;
        set_token_boundary(&deployment, "sekrit", false).await;

        let app = super::router(deployment);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_info_requires_token_when_enabled() {
        let (_env_guard, deployment) = setup_deployment().await;
        set_token_boundary(&deployment, "sekrit", false).await;

        let app = super::router(deployment);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/info")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.get("success").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(
            json.get("message").and_then(|v| v.as_str()),
            Some("Unauthorized")
        );
    }

    #[tokio::test]
    async fn api_requests_fail_closed_when_token_mode_is_misconfigured() {
        let (_env_guard, deployment) = setup_deployment().await;
        set_misconfigured_token_boundary(&deployment).await;

        let app = super::router(deployment);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/info")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.get("success").and_then(|v| v.as_bool()), Some(false));
        assert!(
            json.get("message")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .contains("misconfigured")
        );
    }

    #[tokio::test]
    async fn api_info_accepts_authorization_header_and_redacts_token() {
        let (_env_guard, deployment) = setup_deployment().await;
        set_token_boundary(&deployment, "sekrit", false).await;

        let app = super::router(deployment);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/info")
                    .header(header::AUTHORIZATION, "Bearer sekrit")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.get("success").and_then(|v| v.as_bool()), Some(true));
        let token_value = json.pointer("/data/config/access_control/token");
        assert!(token_value.is_some());
        assert!(token_value.unwrap().is_null());
    }

    #[tokio::test]
    async fn api_info_allows_localhost_bypass_when_enabled() {
        let (_env_guard, deployment) = setup_deployment().await;
        set_token_boundary(&deployment, "sekrit", true).await;

        let app = super::router(deployment);

        let mut request = Request::builder()
            .uri("/api/info")
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(loopback_connect_info());

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn config_status_endpoint_returns_runtime_metadata() {
        let (_env_guard, deployment) = setup_deployment().await;
        let expected_dir = deployment
            .config_status()
            .read()
            .await
            .config_dir
            .to_string_lossy()
            .to_string();

        let app = super::router(deployment);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/config/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.get("success").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            json.pointer("/data/config_dir")
                .and_then(|v| v.as_str())
                .unwrap_or_default(),
            expected_dir
        );
        assert!(
            json.pointer("/data/loaded_at_unix_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or_default()
                > 0
        );
    }

    #[tokio::test]
    async fn removed_pr_endpoints_return_not_found() {
        let (_env_guard, deployment) = setup_deployment().await;
        let app = super::router(deployment);

        let attempt_id = Uuid::new_v4();
        let pr_routes = [
            (
                axum::http::Method::POST,
                format!("/api/task-attempts/{attempt_id}/pr"),
            ),
            (
                axum::http::Method::POST,
                format!("/api/task-attempts/{attempt_id}/pr/attach"),
            ),
            (
                axum::http::Method::GET,
                format!("/api/task-attempts/{attempt_id}/pr/comments?repo_id={attempt_id}"),
            ),
        ];

        for (method, uri) in pr_routes {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method.clone())
                        .uri(uri.as_str())
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert!(
                matches!(response.status(), StatusCode::NOT_FOUND | StatusCode::GONE),
                "expected 404/410 for removed PR endpoint {} {}, got {}",
                method,
                uri,
                response.status(),
            );
        }
    }

    #[tokio::test]
    async fn server_starts_with_read_only_config_dir() {
        let temp_root = std::env::temp_dir().join(format!("vk-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let _env_guard = TestEnvGuard::new(&temp_root, db_url);

        let config_dir = utils_core::vk_config_dir();
        let config_path = utils_core::vk_config_yaml_path();
        std::fs::write(&config_path, "{}\n").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&config_dir, std::fs::Permissions::from_mode(0o500))
                .expect("chmod config dir");
        }

        let deployment = DeploymentImpl::new()
            .await
            .expect("server should start with read-only config dir");

        // Runtime no longer writes schemas on startup.
        assert!(!utils_core::vk_config_schema_path().exists());
        assert!(!utils_core::vk_projects_schema_path().exists());

        // Sanity: the deployment should report the same config directory.
        assert_eq!(
            deployment.config_status().read().await.config_dir,
            config_dir
        );
    }

    #[tokio::test]
    async fn config_reload_failure_sets_last_error_and_keeps_config() {
        let (_env_guard, deployment) = setup_deployment().await;
        let config_path = deployment.config_status().read().await.config_path.clone();

        let git_branch_prefix_before = deployment.config().read().await.git_branch_prefix.clone();
        let loaded_at_before = deployment.config_status().read().await.loaded_at;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&config_path, "not: [valid").unwrap();

        let app = super::router(deployment.clone());

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/config/reload")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(deployment.config_status().read().await.last_error.is_some());
        assert_eq!(
            deployment.config_status().read().await.loaded_at,
            loaded_at_before
        );
        assert_eq!(
            deployment.config().read().await.git_branch_prefix,
            git_branch_prefix_before
        );
    }

    #[tokio::test]
    async fn settings_write_endpoints_return_405_with_guidance() {
        let (_env_guard, deployment) = setup_deployment().await;
        let app = super::router(deployment);

        async fn assert_method_not_allowed(app: &axum::Router, request: Request<Body>) {
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);

            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(json.get("success").and_then(|v| v.as_bool()), Some(false));
            let message = json
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            assert!(message.contains("config.yaml") || message.contains("projects.yaml"));
        }

        assert_method_not_allowed(
            &app,
            Request::builder()
                .method("PUT")
                .uri("/api/config")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_method_not_allowed(
            &app,
            Request::builder()
                .method("PUT")
                .uri("/api/profiles")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_method_not_allowed(
            &app,
            Request::builder()
                .method("POST")
                .uri("/api/projects")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_method_not_allowed(
            &app,
            Request::builder()
                .method("PUT")
                .uri("/api/projects/00000000-0000-0000-0000-000000000000")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_method_not_allowed(
            &app,
            Request::builder()
                .method("DELETE")
                .uri("/api/projects/00000000-0000-0000-0000-000000000000")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    }

    #[tokio::test]
    async fn events_require_token_and_accept_query_param() {
        let (_env_guard, deployment) = setup_deployment().await;
        set_token_boundary(&deployment, "sekrit", false).await;

        let app = super::router(deployment.clone());

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/events?token=sekrit")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        assert!(content_type.contains("text/event-stream"));
    }

    #[tokio::test]
    async fn events_fail_closed_when_token_mode_is_misconfigured() {
        let (_env_guard, deployment) = setup_deployment().await;
        set_misconfigured_token_boundary(&deployment).await;

        let app = super::router(deployment);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn websocket_upgrade_requires_token() {
        let (_env_guard, deployment) = setup_deployment().await;
        set_token_boundary(&deployment, "sekrit", false).await;

        let app = super::router(deployment);

        let make_ws_request = |uri: &'static str| {
            Request::builder()
                .method("GET")
                .uri(uri)
                .version(axum::http::Version::HTTP_11)
                .header(header::HOST, "localhost")
                .header(header::CONNECTION, "Upgrade")
                .header(header::UPGRADE, "websocket")
                .header("sec-websocket-version", "13")
                .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
                .body(Body::empty())
                .unwrap()
        };

        let response = app
            .clone()
            .oneshot(make_ws_request("/api/tasks/stream/ws"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = app
            .oneshot(make_ws_request("/api/tasks/stream/ws?token=sekrit"))
            .await
            .unwrap();

        // `oneshot` requests don't include Hyper's `OnUpgrade` extension, so axum
        // rejects WebSocket upgrades with 426 even when the handshake headers are
        // otherwise valid. We still assert this isn't a 401 to confirm auth passed.
        assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED);
    }

    #[tokio::test]
    async fn websocket_upgrade_fails_closed_when_token_mode_is_misconfigured() {
        let (_env_guard, deployment) = setup_deployment().await;
        set_misconfigured_token_boundary(&deployment).await;

        let app = super::router(deployment);

        let request = Request::builder()
            .method("GET")
            .uri("/api/tasks/stream/ws")
            .version(axum::http::Version::HTTP_11)
            .header(header::HOST, "localhost")
            .header(header::CONNECTION, "Upgrade")
            .header(header::UPGRADE, "websocket")
            .header("sec-websocket-version", "13")
            .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn filesystem_directory_rejects_path_outside_workspace_dir() {
        let (_env_guard, deployment) = setup_deployment().await;
        let allowed_root = std::env::temp_dir().join(format!("vk-fs-allowed-{}", Uuid::new_v4()));
        let outside_root = std::env::temp_dir().join(format!("vk-fs-outside-{}", Uuid::new_v4()));
        fs::create_dir_all(&allowed_root).unwrap();
        fs::create_dir_all(&outside_root).unwrap();
        set_workspace_dir(&deployment, &allowed_root).await;

        let app = super::router(deployment);
        let uri = format!(
            "/api/filesystem/directory?path={}",
            outside_root.to_string_lossy()
        );
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let _ = fs::remove_dir_all(&allowed_root);
        let _ = fs::remove_dir_all(&outside_root);
    }

    #[tokio::test]
    async fn filesystem_directory_allows_path_inside_workspace_dir() {
        let (_env_guard, deployment) = setup_deployment().await;
        let allowed_root = std::env::temp_dir().join(format!("vk-fs-allowed-{}", Uuid::new_v4()));
        let nested = allowed_root.join("project-a");
        fs::create_dir_all(&nested).unwrap();
        set_workspace_dir(&deployment, &allowed_root).await;

        let app = super::router(deployment);
        let uri = format!(
            "/api/filesystem/directory?path={}",
            nested.to_string_lossy()
        );
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.get("success").and_then(|v| v.as_bool()), Some(true));
        let current_path = json
            .pointer("/data/current_path")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let allowed_root_canon =
            std::fs::canonicalize(&allowed_root).unwrap_or_else(|_| allowed_root.clone());
        let current_path_canon =
            std::fs::canonicalize(current_path).unwrap_or_else(|_| current_path.into());
        assert!(current_path_canon.starts_with(&allowed_root_canon));

        let _ = fs::remove_dir_all(&allowed_root);
    }

    #[tokio::test]
    async fn filesystem_git_repo_discovery_stays_within_workspace_dir() {
        let (_env_guard, deployment) = setup_deployment().await;
        let allowed_root = std::env::temp_dir().join(format!("vk-fs-allowed-{}", Uuid::new_v4()));
        let allowed_repo = allowed_root.join("repo-in").join(".git");
        let outside_root = std::env::temp_dir().join(format!("vk-fs-outside-{}", Uuid::new_v4()));
        let outside_repo = outside_root.join("repo-out").join(".git");
        fs::create_dir_all(&allowed_repo).unwrap();
        fs::create_dir_all(&outside_repo).unwrap();
        set_workspace_dir(&deployment, &allowed_root).await;

        let app = super::router(deployment);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/filesystem/git-repos")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let paths = json
            .pointer("/data")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|entry| {
                entry
                    .get("path")
                    .and_then(|path| path.as_str())
                    .map(str::to_owned)
            })
            .collect::<Vec<_>>();

        assert!(
            paths.iter().any(|path| path.contains("repo-in")),
            "expected repo-in in {:?}",
            paths
        );
        assert!(
            paths.iter().all(|path| !path.contains("repo-out")),
            "unexpected repo-out in {:?}",
            paths
        );

        let _ = fs::remove_dir_all(&allowed_root);
        let _ = fs::remove_dir_all(&outside_root);
    }

    #[tokio::test]
    async fn check_agent_compatibility_rejects_non_codex_executor() {
        let (_env_guard, deployment) = setup_deployment().await;
        let app = super::router(deployment);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/agents/check-compatibility?executor=CLAUDE_CODE")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.get("success").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(
            json.get("message").and_then(|v| v.as_str()),
            Some("Compatibility check is only supported for Codex")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn check_agent_compatibility_returns_incompatible_with_message() {
        use std::os::unix::fs::PermissionsExt;

        let (_env_guard, deployment) = setup_deployment().await;

        let asset_dir = std::env::var("VIBE_ASSET_DIR").expect("VIBE_ASSET_DIR");
        let asset_dir = std::path::PathBuf::from(asset_dir);

        let fake_codex = asset_dir.join("fake-codex");
        std::fs::write(
            &fake_codex,
            r#"#!/bin/sh
set -eu

if [ "${1:-}" = "--version" ]; then
  echo "codex-cli 0.0.0-test"
  exit 0
fi

if [ "${1:-}" = "--oss" ]; then
  shift
fi

if [ "${1:-}" = "app-server" ] && [ "${2:-}" = "generate-json-schema" ]; then
  out=""
  while [ "$#" -gt 0 ]; do
    if [ "$1" = "--out" ]; then
      out="$2"
      shift 2
      continue
    fi
    shift
  done
  if [ -z "$out" ]; then
    echo "missing --out" >&2
    exit 1
  fi
  mkdir -p "$out"
  echo '{"vk":"mismatch"}' > "$out/codex_app_server_protocol.v2.schemas.json"
  exit 0
fi

echo "unexpected args: $*" >&2
exit 1
"#,
        )
        .expect("write fake codex");

        let mut perms = std::fs::metadata(&fake_codex)
            .expect("metadata")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&fake_codex, perms).expect("chmod");

        let vk_config_dir = std::env::var("VK_CONFIG_DIR").expect("VK_CONFIG_DIR");
        let config_path = std::path::PathBuf::from(vk_config_dir).join("config.yaml");
        let fake_codex_path = fake_codex
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        let config_yaml = format!(
            "executor_profiles:\n  executors:\n    CODEX:\n      DEFAULT:\n        CODEX:\n          base_command_override: \"{fake_codex_path}\"\n"
        );
        std::fs::write(&config_path, config_yaml).expect("write config.yaml");
        deployment
            .reload_user_config()
            .await
            .expect("reload config");

        let app = super::router(deployment);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/agents/check-compatibility?executor=CODEX&refresh=true")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.get("success").and_then(|v| v.as_bool()), Some(true));
        let data = json.get("data").expect("data");
        assert_eq!(
            data.get("status").and_then(|v| v.as_str()),
            Some("incompatible")
        );
        assert!(
            data.get("expected_v2_schema_sha256")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty())
        );
        assert!(
            data.get("runtime_v2_schema_sha256")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty())
        );

        let message = data.get("message").and_then(|v| v.as_str()).unwrap_or("");
        assert!(message.contains("Codex protocol is incompatible"));
        assert!(message.contains("Expected protocol fingerprint"));
        assert!(message.contains("Base command:"));
    }
}
