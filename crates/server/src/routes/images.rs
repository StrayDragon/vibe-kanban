use std::path::Path as StdPath;

use app_runtime::Deployment;
use axum::{
    Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{StatusCode, header},
    response::{Json as ResponseJson, Response},
    routing::{delete, get, post},
};
use chrono::{DateTime, Utc};
use db::{
    DbErr,
    models::{
        image::{Image, TaskImage},
        task::Task,
    },
};
use execution::image::ImageError;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use ts_rs::TS;
use utils_core::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

pub(crate) const IMAGE_FILE_CACHE_CONTROL: &str = "private, max-age=31536000, immutable";

pub(crate) fn build_image_file_response(
    body: Body,
    content_type: &str,
    content_length: u64,
) -> Result<Response, ApiError> {
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, content_length)
        .header(header::CACHE_CONTROL, IMAGE_FILE_CACHE_CONTROL)
        .header("X-Content-Type-Options", "nosniff")
        .body(body)
        .map_err(|e| ApiError::Image(ImageError::ResponseBuildError(e.to_string())))?;

    Ok(response)
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ImageResponse {
    pub id: Uuid,
    pub file_path: String, // relative path to display in markdown
    pub original_name: String,
    pub mime_type: Option<String>,
    pub size_bytes: i64,
    pub hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ImageResponse {
    pub fn from_image(image: Image) -> Self {
        // special relative path for images
        let markdown_path = format!("{}/{}", utils_core::path::VIBE_IMAGES_DIR, image.file_path);
        Self {
            id: image.id,
            file_path: markdown_path,
            original_name: image.original_name,
            mime_type: image.mime_type,
            size_bytes: image.size_bytes,
            hash: image.hash,
            created_at: image.created_at,
            updated_at: image.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ImageMetadataQuery {
    /// Path relative to worktree root, e.g., ".vibe-images/screenshot.png"
    pub path: String,
}

/// Metadata response for image files, used for rendering in WYSIWYG editor
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ImageMetadata {
    pub exists: bool,
    pub file_name: Option<String>,
    pub path: Option<String>,
    pub size_bytes: Option<i64>,
    pub format: Option<String>,
    pub proxy_url: Option<String>,
}

pub async fn upload_image(
    State(deployment): State<DeploymentImpl>,
    multipart: Multipart,
) -> Result<ResponseJson<ApiResponse<ImageResponse>>, ApiError> {
    let image_response = process_image_upload(&deployment, multipart, None).await?;
    Ok(ResponseJson(ApiResponse::success(image_response)))
}

pub(crate) async fn process_image_upload(
    deployment: &DeploymentImpl,
    mut multipart: Multipart,
    link_task_id: Option<Uuid>,
) -> Result<ImageResponse, ApiError> {
    let image_service = deployment.image();

    while let Some(field) = multipart.next_field().await? {
        if field.name() == Some("image") {
            let filename = field
                .file_name()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "image.png".to_string());

            let data = field.bytes().await?;
            let image = image_service.store_image(&data, &filename).await?;

            if let Some(task_id) = link_task_id {
                TaskImage::associate_many_dedup(
                    &deployment.db().pool,
                    task_id,
                    std::slice::from_ref(&image.id),
                )
                .await?;
            }

            return Ok(ImageResponse::from_image(image));
        }
    }

    Err(ApiError::Image(ImageError::NotFound))
}

pub async fn upload_task_image(
    Path(task_id): Path<Uuid>,
    State(deployment): State<DeploymentImpl>,
    multipart: Multipart,
) -> Result<ResponseJson<ApiResponse<ImageResponse>>, ApiError> {
    Task::find_by_id(&deployment.db().pool, task_id)
        .await?
        .ok_or(ApiError::Database(DbErr::RecordNotFound(
            "Task not found".to_string(),
        )))?;

    let image_response = process_image_upload(&deployment, multipart, Some(task_id)).await?;
    Ok(ResponseJson(ApiResponse::success(image_response)))
}

/// Serve an image file by ID
pub async fn serve_image(
    Path(image_id): Path<Uuid>,
    State(deployment): State<DeploymentImpl>,
) -> Result<Response, ApiError> {
    let image_service = deployment.image();
    let image = image_service
        .get_image(image_id)
        .await?
        .ok_or_else(|| ApiError::Image(ImageError::NotFound))?;
    let file_path = image_service.get_absolute_path(&image);

    let file = File::open(&file_path).await?;
    let metadata = file.metadata().await?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let content_type = image
        .mime_type
        .as_deref()
        .unwrap_or("application/octet-stream");

    build_image_file_response(body, content_type, metadata.len())
}

pub async fn delete_image(
    Path(image_id): Path<Uuid>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let image_service = deployment.image();
    image_service.delete_image(image_id).await?;
    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn get_task_images(
    Path(task_id): Path<Uuid>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<ImageResponse>>>, ApiError> {
    let images = Image::find_by_task_id(&deployment.db().pool, task_id).await?;
    let image_responses = images.into_iter().map(ImageResponse::from_image).collect();
    Ok(ResponseJson(ApiResponse::success(image_responses)))
}

/// Get metadata for an image associated with a task.
/// The path should be in the format `.vibe-images/{uuid}.{ext}`.
pub async fn get_task_image_metadata(
    Path(task_id): Path<Uuid>,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ImageMetadataQuery>,
) -> Result<ResponseJson<ApiResponse<ImageMetadata>>, ApiError> {
    let not_found_response = || ImageMetadata {
        exists: false,
        file_name: None,
        path: Some(query.path.clone()),
        size_bytes: None,
        format: None,
        proxy_url: None,
    };

    // Validate path starts with .vibe-images/
    let vibe_images_prefix = format!("{}/", utils_core::path::VIBE_IMAGES_DIR);
    if !query.path.starts_with(&vibe_images_prefix) {
        return Ok(ResponseJson(ApiResponse::success(not_found_response())));
    }

    // Reject paths with .. to prevent traversal
    if query.path.contains("..") {
        return Ok(ResponseJson(ApiResponse::success(not_found_response())));
    }

    // Extract the filename from the path (e.g., "uuid.png" from ".vibe-images/uuid.png")
    let file_name = match query.path.strip_prefix(&vibe_images_prefix) {
        Some(name) if !name.is_empty() => name,
        _ => return Ok(ResponseJson(ApiResponse::success(not_found_response()))),
    };

    // Look up the image by file_path (which is just the filename in the images table)
    let image = match Image::find_by_file_path(&deployment.db().pool, file_name).await? {
        Some(img) => img,
        None => return Ok(ResponseJson(ApiResponse::success(not_found_response()))),
    };

    // Verify the image is associated with this task
    let is_associated = TaskImage::is_associated(&deployment.db().pool, task_id, image.id).await?;
    if !is_associated {
        return Ok(ResponseJson(ApiResponse::success(not_found_response())));
    }

    // Get format from extension
    let format = StdPath::new(file_name)
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase());

    // Build the proxy URL
    let proxy_url = format!("/api/images/{}/file", image.id);

    Ok(ResponseJson(ApiResponse::success(ImageMetadata {
        exists: true,
        file_name: Some(image.original_name),
        path: Some(query.path),
        size_bytes: Some(image.size_bytes),
        format,
        proxy_url: Some(proxy_url),
    })))
}

pub fn routes() -> Router<DeploymentImpl> {
    Router::new()
        .route(
            "/upload",
            post(upload_image).layer(DefaultBodyLimit::max(20 * 1024 * 1024)), // 20MB limit
        )
        .route("/{id}/file", get(serve_image))
        .route("/{id}", delete(delete_image))
        .route("/task/{task_id}", get(get_task_images))
        .route("/task/{task_id}/metadata", get(get_task_image_metadata))
        .route(
            "/task/{task_id}/upload",
            post(upload_task_image).layer(DefaultBodyLimit::max(20 * 1024 * 1024)),
        )
}

#[cfg(test)]
mod tests {
    use app_runtime::Deployment;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode, header},
    };
    use test_support::TestEnv;
    use tower::ServiceExt;
    use uuid::Uuid;

    use crate::{DeploymentImpl, http};

    async fn setup_deployment() -> (TestEnv, DeploymentImpl) {
        let env_guard = TestEnv::new("vk-test-");
        let deployment = DeploymentImpl::new().await.unwrap();

        (env_guard, deployment)
    }

    fn multipart_body(filename: &str, content_type: &str, data: &[u8]) -> (String, Vec<u8>) {
        let boundary = format!("vk-boundary-{}", Uuid::new_v4());
        let mut body = Vec::new();

        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"image\"; filename=\"{filename}\"\r\n")
                .as_bytes(),
        );
        body.extend_from_slice(format!("Content-Type: {content_type}\r\n\r\n").as_bytes());
        body.extend_from_slice(data);
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

        (boundary, body)
    }

    #[tokio::test]
    async fn svg_upload_is_rejected() {
        let (_env_guard, deployment) = setup_deployment().await;
        let app = http::router(deployment);

        let (boundary, body) = multipart_body(
            "evil.svg",
            "image/svg+xml",
            br#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#,
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/images/upload")
                    .header(
                        header::CONTENT_TYPE,
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn uploaded_images_are_not_publicly_cacheable_and_nosniff() {
        let (_env_guard, deployment) = setup_deployment().await;
        let app = http::router(deployment);

        let (boundary, body) = multipart_body("ok.png", "image/png", b"not-a-real-png");
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/images/upload")
                    .header(
                        header::CONTENT_TYPE,
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let image_id = json
            .pointer("/data/id")
            .and_then(|v| v.as_str())
            .expect("upload response should include image id");

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/images/{image_id}/file"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let cache_control = response
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        assert!(!cache_control.contains("public"));

        assert_eq!(
            response
                .headers()
                .get("X-Content-Type-Options")
                .and_then(|v| v.to_str().ok()),
            Some("nosniff")
        );
    }
}
