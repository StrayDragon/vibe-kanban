use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use axum::{
    Router,
    extract::{Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use deployment::Deployment;
use serde::Deserialize;
use services::services::{
    filesystem::{DirectoryEntry, DirectoryListResponse, FilesystemError},
    workspace_manager::WorkspaceManager,
};
use utils::response::ApiResponse;

use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Deserialize)]
pub struct ListDirectoryQuery {
    path: Option<String>,
}

fn map_filesystem_error(error: FilesystemError) -> ApiError {
    match error {
        FilesystemError::DirectoryDoesNotExist => {
            ApiError::NotFound("Directory does not exist".to_string())
        }
        FilesystemError::PathIsNotDirectory => {
            ApiError::BadRequest("Path is not a directory".to_string())
        }
        FilesystemError::Io(e) => {
            tracing::error!("Failed to read directory: {}", e);
            ApiError::Io(e)
        }
    }
}

fn canonicalize_directory(path: &Path) -> Result<PathBuf, ApiError> {
    if !path.exists() {
        return Err(ApiError::NotFound("Directory does not exist".to_string()));
    }
    if !path.is_dir() {
        return Err(ApiError::BadRequest("Path is not a directory".to_string()));
    }
    fs::canonicalize(path).map_err(ApiError::Io)
}

async fn allowed_workspace_roots(deployment: &DeploymentImpl) -> Result<Vec<PathBuf>, ApiError> {
    let configured_workspace_dir = deployment.config().read().await.workspace_dir.clone();
    let mut candidates: Vec<PathBuf> = vec![WorkspaceManager::get_workspace_base_dir()];

    if let Some(workspace_dir) = configured_workspace_dir {
        let workspace_path = PathBuf::from(workspace_dir);
        candidates.insert(0, canonicalize_directory(&workspace_path)?);
    } else {
        if let Ok(home_dir) = std::env::var("HOME") {
            candidates.insert(0, PathBuf::from(home_dir));
        }
        if let Ok(cwd) = std::env::current_dir() {
            candidates.push(cwd);
        }
    }

    let mut roots = Vec::new();
    let mut seen = HashSet::new();
    for candidate in candidates {
        let canonical = match fs::canonicalize(&candidate) {
            Ok(path) if path.is_dir() => path,
            _ => continue,
        };
        if seen.insert(canonical.clone()) {
            roots.push(canonical);
        }
    }

    if roots.is_empty() {
        return Err(ApiError::Forbidden(
            "No readable workspace roots are configured".to_string(),
        ));
    }

    Ok(roots)
}

fn resolve_request_path(path: Option<&str>, roots: &[PathBuf]) -> Result<PathBuf, ApiError> {
    let fallback_root = roots.first().ok_or_else(|| {
        ApiError::Forbidden("No allowed workspace roots are available".to_string())
    })?;
    let requested = match path {
        Some(path) if !path.trim().is_empty() => {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                path
            } else {
                fallback_root.join(path)
            }
        }
        _ => fallback_root.clone(),
    };

    let canonical = canonicalize_directory(&requested)?;
    if roots.iter().any(|root| canonical.starts_with(root)) {
        return Ok(canonical);
    }

    Err(ApiError::Forbidden(
        "Path is outside configured workspace roots".to_string(),
    ))
}

pub async fn list_directory(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListDirectoryQuery>,
) -> Result<ResponseJson<ApiResponse<DirectoryListResponse>>, ApiError> {
    let roots = allowed_workspace_roots(&deployment).await?;
    let path = resolve_request_path(query.path.as_deref(), &roots)?
        .to_string_lossy()
        .to_string();

    match deployment.filesystem().list_directory(Some(path)).await {
        Ok(response) => Ok(ResponseJson(ApiResponse::success(response))),
        Err(error) => Err(map_filesystem_error(error)),
    }
}

pub async fn list_git_repos(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListDirectoryQuery>,
) -> Result<ResponseJson<ApiResponse<Vec<DirectoryEntry>>>, ApiError> {
    let roots = allowed_workspace_roots(&deployment).await?;

    let res = if let Some(ref path) = query.path {
        let resolved_path = resolve_request_path(Some(path), &roots)?
            .to_string_lossy()
            .to_string();
        deployment
            .filesystem()
            .list_git_repos(Some(resolved_path), 800, 1200, Some(3))
            .await
    } else {
        deployment
            .filesystem()
            .list_git_repos_in_paths(roots, 800, 1200, Some(4))
            .await
    };
    match res {
        Ok(response) => Ok(ResponseJson(ApiResponse::success(response))),
        Err(error) => Err(map_filesystem_error(error)),
    }
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/filesystem/directory", get(list_directory))
        .route("/filesystem/git-repos", get(list_git_repos))
}
