use std::path::Path;

use app_runtime::Deployment;
use axum::{
    Extension, Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Query, Request, State},
    http::StatusCode,
    middleware::{Next, from_fn_with_state},
    response::{Json as ResponseJson, Response},
    routing::{get, post},
};
use db::models::{task::Task, workspace::Workspace};
use execution::{container::ContainerService, image::ImageError};
use serde::Deserialize;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use utils_core::response::ApiResponse;
use uuid::Uuid;

use crate::{
    DeploymentImpl,
    error::ApiError,
    middleware::load_workspace_middleware,
    routes::images::{ImageMetadata, ImageResponse, build_image_file_response, process_image_upload},
};

#[derive(Debug, Deserialize)]
pub struct ImageMetadataQuery {
    /// Path relative to worktree root, e.g., ".vibe-images/screenshot.png"
    pub path: String,
}

/// Upload an image and immediately copy it to the workspace's worktree.
/// This allows images to be available in the container before follow-up is sent.
pub async fn upload_image(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    multipart: Multipart,
) -> Result<ResponseJson<ApiResponse<ImageResponse>>, ApiError> {
    // Get the task for this attempt
    let task = Task::find_by_id(&deployment.db().pool, workspace.task_id)
        .await?
        .ok_or_else(|| ApiError::Image(ImageError::NotFound))?;

    // Process upload (store in cache, associate with task)
    let image_response = process_image_upload(&deployment, multipart, Some(task.id)).await?;

    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_path = std::path::PathBuf::from(container_ref);
    deployment
        .image()
        .copy_images_by_ids_to_worktree(&workspace_path, &[image_response.id])
        .await?;

    Ok(ResponseJson(ApiResponse::success(image_response)))
}

/// Get metadata about an image in the workspace's worktree.
pub async fn get_image_metadata(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ImageMetadataQuery>,
) -> Result<ResponseJson<ApiResponse<ImageMetadata>>, ApiError> {
    // Validate path starts with .vibe-images/
    let vibe_images_prefix = format!("{}/", utils_core::path::VIBE_IMAGES_DIR);
    if !query.path.starts_with(&vibe_images_prefix) {
        return Ok(ResponseJson(ApiResponse::success(ImageMetadata {
            exists: false,
            file_name: None,
            path: Some(query.path),
            size_bytes: None,
            format: None,
            proxy_url: None,
        })));
    }

    // Reject paths with .. to prevent traversal
    if query.path.contains("..") {
        return Ok(ResponseJson(ApiResponse::success(ImageMetadata {
            exists: false,
            file_name: None,
            path: Some(query.path),
            size_bytes: None,
            format: None,
            proxy_url: None,
        })));
    }

    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_path = std::path::PathBuf::from(container_ref);
    let vibe_images_dir = workspace_path.join(utils_core::path::VIBE_IMAGES_DIR);
    let image_rel = query.path.strip_prefix(&vibe_images_prefix).unwrap_or("");
    if image_rel.trim().is_empty() {
        return Ok(ResponseJson(ApiResponse::success(ImageMetadata {
            exists: false,
            file_name: None,
            path: Some(query.path),
            size_bytes: None,
            format: None,
            proxy_url: None,
        })));
    }

    let full_path = vibe_images_dir.join(image_rel);

    // Security: canonicalize and verify path stays within .vibe-images (blocks symlink escapes).
    let canonical_path = match tokio::fs::canonicalize(&full_path).await {
        Ok(path) => path,
        Err(_) => {
            return Ok(ResponseJson(ApiResponse::success(ImageMetadata {
                exists: false,
                file_name: None,
                path: Some(query.path),
                size_bytes: None,
                format: None,
                proxy_url: None,
            })));
        }
    };

    let canonical_vibe_images = match tokio::fs::canonicalize(&vibe_images_dir).await {
        Ok(path) => path,
        Err(_) => {
            return Ok(ResponseJson(ApiResponse::success(ImageMetadata {
                exists: false,
                file_name: None,
                path: Some(query.path),
                size_bytes: None,
                format: None,
                proxy_url: None,
            })));
        }
    };

    if !canonical_path.starts_with(&canonical_vibe_images) {
        return Ok(ResponseJson(ApiResponse::success(ImageMetadata {
            exists: false,
            file_name: None,
            path: Some(query.path),
            size_bytes: None,
            format: None,
            proxy_url: None,
        })));
    }

    // Check if file exists
    let metadata = match tokio::fs::metadata(&canonical_path).await {
        Ok(m) if m.is_file() => m,
        _ => {
            return Ok(ResponseJson(ApiResponse::success(ImageMetadata {
                exists: false,
                file_name: None,
                path: Some(query.path),
                size_bytes: None,
                format: None,
                proxy_url: None,
            })));
        }
    };

    // Extract filename
    let file_name = Path::new(&query.path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string());

    // Detect format from extension
    let format = Path::new(&query.path)
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase());

    // Build proxy URL - the path after .vibe-images/
    let image_path = image_rel;
    let proxy_url = format!(
        "/api/task-attempts/{}/images/file/{}",
        workspace.id, image_path
    );

    Ok(ResponseJson(ApiResponse::success(ImageMetadata {
        exists: true,
        file_name,
        path: Some(query.path),
        size_bytes: Some(metadata.len() as i64),
        format,
        proxy_url: Some(proxy_url),
    })))
}

/// Serve an image file from the workspace's .vibe-images folder.
pub async fn serve_image(
    axum::extract::Path((_id, path)): axum::extract::Path<(Uuid, String)>,
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<Response, ApiError> {
    // Reject paths with .. to prevent traversal
    if path.contains("..") {
        return Err(ApiError::Image(ImageError::NotFound));
    }
    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_path = std::path::PathBuf::from(container_ref);

    let vibe_images_dir = workspace_path.join(utils_core::path::VIBE_IMAGES_DIR);
    let full_path = vibe_images_dir.join(&path);

    // Security: Canonicalize and verify path is within .vibe-images
    let canonical_path = tokio::fs::canonicalize(&full_path)
        .await
        .map_err(|_| ApiError::Image(ImageError::NotFound))?;

    let canonical_vibe_images = tokio::fs::canonicalize(&vibe_images_dir)
        .await
        .map_err(|_| ApiError::Image(ImageError::NotFound))?;

    if !canonical_path.starts_with(&canonical_vibe_images) {
        return Err(ApiError::Image(ImageError::NotFound));
    }

    // Open and stream the file
    let file = File::open(&canonical_path)
        .await
        .map_err(|_| ApiError::Image(ImageError::NotFound))?;

    let metadata = file
        .metadata()
        .await
        .map_err(|_| ApiError::Image(ImageError::NotFound))?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    // Determine content type from extension
    let content_type = Path::new(&path)
        .extension()
        .and_then(|ext| match ext.to_string_lossy().to_lowercase().as_str() {
            "png" => Some("image/png"),
            "jpg" | "jpeg" => Some("image/jpeg"),
            "gif" => Some("image/gif"),
            "webp" => Some("image/webp"),
            "svg" => Some("image/svg+xml"),
            "ico" => Some("image/x-icon"),
            "bmp" => Some("image/bmp"),
            "tiff" | "tif" => Some("image/tiff"),
            _ => None,
        })
        .unwrap_or("application/octet-stream");

    build_image_file_response(body, content_type, metadata.len())
}

/// Middleware to load Workspace for routes with wildcard path params.
async fn load_workspace_with_wildcard(
    State(deployment): State<DeploymentImpl>,
    axum::extract::Path((id, _path)): axum::extract::Path<(Uuid, String)>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let attempt = match Workspace::find_by_id(&deployment.db().pool, id).await {
        Ok(Some(a)) => a,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };
    request.extensions_mut().insert(attempt);
    Ok(next.run(request).await)
}

pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    let metadata_router = Router::new()
        .route("/metadata", get(get_image_metadata))
        .route(
            "/upload",
            post(upload_image).layer(DefaultBodyLimit::max(20 * 1024 * 1024)), // 20MB limit
        )
        .layer(from_fn_with_state(
            deployment.clone(),
            load_workspace_middleware::<DeploymentImpl>,
        ));

    let file_router = Router::new()
        .route("/file/{*path}", get(serve_image))
        .layer(from_fn_with_state(
            deployment.clone(),
            load_workspace_with_wildcard,
        ));

    metadata_router.merge(file_router)
}
