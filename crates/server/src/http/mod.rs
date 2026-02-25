use axum::{Router, middleware::from_fn_with_state, routing::get};

use crate::{DeploymentImpl, routes};

mod auth;
mod frontend;

pub fn router(deployment: DeploymentImpl) -> Router {
    let api_routes = Router::new()
        .merge(routes::config::router())
        .merge(routes::containers::router(&deployment))
        .merge(routes::projects::router(&deployment))
        .merge(routes::tasks::router(&deployment))
        .merge(routes::task_groups::router(&deployment))
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
        .layer(from_fn_with_state(
            deployment.clone(),
            auth::require_api_auth,
        ));

    Router::new()
        .route("/health", get(routes::health::health_check))
        .route("/", get(frontend::serve_frontend_root))
        .route("/{*path}", get(frontend::serve_frontend))
        .nest("/api", api_routes)
        .with_state(deployment)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        net::{IpAddr, Ipv4Addr, SocketAddr},
    };

    use axum::{
        body::{Body, to_bytes},
        extract::ConnectInfo,
        http::{Request, StatusCode, header},
    };
    use deployment::Deployment;
    use services::services::config::AccessControlMode;
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
    async fn filesystem_directory_rejects_path_outside_workspace_dir() {
        let (_env_guard, deployment) = setup_deployment().await;
        let allowed_root =
            std::env::temp_dir().join(format!("vk-fs-allowed-{}", Uuid::new_v4()));
        let outside_root =
            std::env::temp_dir().join(format!("vk-fs-outside-{}", Uuid::new_v4()));
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
        let allowed_root =
            std::env::temp_dir().join(format!("vk-fs-allowed-{}", Uuid::new_v4()));
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
        assert!(current_path.starts_with(allowed_root.to_string_lossy().as_ref()));

        let _ = fs::remove_dir_all(&allowed_root);
    }

    #[tokio::test]
    async fn filesystem_git_repo_discovery_stays_within_workspace_dir() {
        let (_env_guard, deployment) = setup_deployment().await;
        let allowed_root =
            std::env::temp_dir().join(format!("vk-fs-allowed-{}", Uuid::new_v4()));
        let allowed_repo = allowed_root.join("repo-in").join(".git");
        let outside_root =
            std::env::temp_dir().join(format!("vk-fs-outside-{}", Uuid::new_v4()));
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
}
