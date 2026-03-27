use std::{
    fs,
    path::{Path as StdPath, PathBuf},
};

use app_runtime::Deployment;
use axum::{
    Router,
    extract::{Path as AxumPath, State},
    response::Json as ResponseJson,
    routing::{get, post},
};
use db::models::repo::Repo;
use repos::git::{GitBranch, GitServiceError};
use serde::Deserialize;
use ts_rs::TS;
use utils_core::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct RegisterRepoRequest {
    pub path: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct InitRepoRequest {
    pub parent_path: String,
    pub folder_name: String,
}

fn canonicalize_existing_directory(path: &StdPath) -> Result<PathBuf, ApiError> {
    if !path.exists() {
        return Err(ApiError::BadRequest(format!(
            "Path does not exist: {}",
            path.display()
        )));
    }

    if !path.is_dir() {
        return Err(ApiError::BadRequest(format!(
            "Path is not a directory: {}",
            path.display()
        )));
    }

    fs::canonicalize(path).map_err(ApiError::Io)
}

fn resolve_repo_request_directory(path: &str, roots: &[PathBuf]) -> Result<PathBuf, ApiError> {
    let fallback_root = roots.first().ok_or_else(|| {
        ApiError::Forbidden("No allowed workspace roots are available".to_string())
    })?;

    let requested = {
        let path = path.trim();
        if path.is_empty() {
            return Err(ApiError::BadRequest("Path is required".to_string()));
        }
        let path = PathBuf::from(path);
        if path.is_absolute() {
            path
        } else {
            fallback_root.join(path)
        }
    };

    let canonical = canonicalize_existing_directory(&requested)?;
    if roots.iter().any(|root| canonical.starts_with(root)) {
        return Ok(canonical);
    }

    Err(ApiError::Forbidden(
        "Path is outside configured workspace roots".to_string(),
    ))
}

pub async fn register_repo(
    State(deployment): State<DeploymentImpl>,
    ResponseJson(payload): ResponseJson<RegisterRepoRequest>,
) -> Result<ResponseJson<ApiResponse<Repo>>, ApiError> {
    let roots = crate::routes::filesystem::allowed_workspace_roots(&deployment).await?;
    let canonical_path = resolve_repo_request_directory(&payload.path, &roots)?;
    let canonical_path = canonical_path.to_string_lossy().to_string();

    let repo = deployment
        .repo()
        .register(
            &deployment.db().pool,
            &canonical_path,
            payload.display_name.as_deref(),
        )
        .await?;

    Ok(ResponseJson(ApiResponse::success(repo)))
}

pub async fn init_repo(
    State(deployment): State<DeploymentImpl>,
    ResponseJson(payload): ResponseJson<InitRepoRequest>,
) -> Result<ResponseJson<ApiResponse<Repo>>, ApiError> {
    let roots = crate::routes::filesystem::allowed_workspace_roots(&deployment).await?;
    let canonical_parent = resolve_repo_request_directory(&payload.parent_path, &roots)?;
    let canonical_parent = canonical_parent.to_string_lossy().to_string();

    let repo = deployment
        .repo()
        .init_repo(
            &deployment.db().pool,
            deployment.git(),
            &canonical_parent,
            &payload.folder_name,
        )
        .await?;

    Ok(ResponseJson(ApiResponse::success(repo)))
}

pub async fn get_repo_branches(
    State(deployment): State<DeploymentImpl>,
    AxumPath(repo_id): AxumPath<Uuid>,
) -> Result<ResponseJson<ApiResponse<Vec<GitBranch>>>, ApiError> {
    let repo = deployment
        .repo()
        .get_by_id(&deployment.db().pool, repo_id)
        .await?;

    let branches = deployment
        .git()
        .get_all_branches(&repo.path)
        .map_err(GitServiceError::from)?;
    Ok(ResponseJson(ApiResponse::success(branches)))
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/repos", post(register_repo))
        .route("/repos/init", post(init_repo))
        .route("/repos/{repo_id}/branches", get(get_repo_branches))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use app_runtime::Deployment;
    use axum::{
        body::Body,
        http::{Request, StatusCode, header},
    };
    use serde_json::json;
    use test_support::TestEnv;
    use tower::ServiceExt;

    use crate::{DeploymentImpl, http};

    async fn setup() -> (TestEnv, DeploymentImpl, std::path::PathBuf) {
        let env_guard = TestEnv::new("vk-test-");
        let deployment = DeploymentImpl::new().await.unwrap();

        let workspace_dir = env_guard.temp_root().join("workspace");
        fs::create_dir_all(&workspace_dir).unwrap();
        deployment.config().write().await.workspace_dir =
            Some(workspace_dir.to_string_lossy().to_string());

        (env_guard, deployment, workspace_dir)
    }

    #[tokio::test]
    async fn register_repo_is_scoped_to_workspace_roots() {
        let (_env_guard, deployment, workspace_dir) = setup().await;

        let inside_repo = workspace_dir.join("inside-repo");
        fs::create_dir_all(inside_repo.join(".git")).unwrap();

        let outside_repo = workspace_dir.parent().unwrap().join("outside-repo");
        fs::create_dir_all(outside_repo.join(".git")).unwrap();

        let app = http::router(deployment);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/repos")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "path": inside_repo.to_string_lossy(),
                            "display_name": null
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/repos")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "path": outside_repo.to_string_lossy(),
                            "display_name": null
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let traversal_path = format!("{}/../outside-repo", workspace_dir.to_string_lossy());
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/repos")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "path": traversal_path,
                            "display_name": null
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        #[cfg(unix)]
        {
            let symlink_repo = workspace_dir.join("symlink-repo");
            std::os::unix::fs::symlink(&outside_repo, &symlink_repo).unwrap();

            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/repos")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(
                            serde_json::to_vec(&json!({
                                "path": symlink_repo.to_string_lossy(),
                                "display_name": null
                            }))
                            .unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
    }

    #[tokio::test]
    async fn init_repo_is_scoped_to_workspace_roots() {
        let (_env_guard, deployment, workspace_dir) = setup().await;

        let parent_inside = workspace_dir.join("parent");
        fs::create_dir_all(&parent_inside).unwrap();

        let parent_outside = workspace_dir.parent().unwrap().join("outside-parent");
        fs::create_dir_all(&parent_outside).unwrap();

        let app = http::router(deployment);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/repos/init")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "parent_path": parent_inside.to_string_lossy(),
                            "folder_name": "newrepo"
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/repos/init")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "parent_path": parent_outside.to_string_lossy(),
                            "folder_name": "newrepo2"
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let traversal_parent = format!("{}/../outside-parent", workspace_dir.to_string_lossy());
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/repos/init")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "parent_path": traversal_parent,
                            "folder_name": "newrepo3"
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        #[cfg(unix)]
        {
            let symlink_parent = workspace_dir.join("symlink-parent");
            std::os::unix::fs::symlink(&parent_outside, &symlink_parent).unwrap();

            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/repos/init")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(
                            serde_json::to_vec(&json!({
                                "parent_path": symlink_parent.to_string_lossy(),
                                "folder_name": "newrepo4"
                            }))
                            .unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
    }
}
