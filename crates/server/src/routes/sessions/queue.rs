use axum::{
    Extension, Json, Router, extract::State, http::HeaderMap, middleware::from_fn_with_state,
    response::Json as ResponseJson, routing::get,
};
use db::models::{scratch::DraftFollowUpData, session::Session};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use services::services::queued_message::QueueStatus;
use ts_rs::TS;
use utils::response::ApiResponse;

use crate::{DeploymentImpl, error::ApiError, middleware::load_session_middleware};

/// Request body for queueing a follow-up message
#[derive(Debug, Deserialize, Serialize, TS)]
pub struct QueueMessageRequest {
    pub message: String,
    pub variant: Option<String>,
}

/// Queue a follow-up message to be executed when the current execution finishes
pub async fn queue_message(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
    headers: HeaderMap,
    Json(payload): Json<QueueMessageRequest>,
) -> Result<ResponseJson<ApiResponse<QueueStatus>>, ApiError> {
    let key = crate::routes::idempotency::idempotency_key(&headers);
    let hash = crate::routes::idempotency::request_hash(&payload)?;

    let data = DraftFollowUpData {
        message: payload.message,
        variant: payload.variant,
    };

    let queued = match key {
        Some(key) => deployment
            .queued_message_service()
            .queue_message_idempotent(session.id, key, hash, data)
            .map_err(|_| ApiError::Conflict("Idempotency key conflict".to_string()))?,
        None => deployment
            .queued_message_service()
            .queue_message(session.id, data),
    };

    Ok(ResponseJson(ApiResponse::success(QueueStatus::Queued {
        message: queued,
    })))
}

/// Cancel a queued follow-up message
pub async fn cancel_queued_message(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<QueueStatus>>, ApiError> {
    deployment
        .queued_message_service()
        .cancel_queued(session.id);

    Ok(ResponseJson(ApiResponse::success(QueueStatus::Empty)))
}

/// Get the current queue status for a session's workspace
pub async fn get_queue_status(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<QueueStatus>>, ApiError> {
    let status = deployment.queued_message_service().get_status(session.id);

    Ok(ResponseJson(ApiResponse::success(status)))
}

pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    Router::new()
        .route(
            "/",
            get(get_queue_status)
                .post(queue_message)
                .delete(cancel_queued_message),
        )
        .layer(from_fn_with_state(
            deployment.clone(),
            load_session_middleware::<DeploymentImpl>,
        ))
}
