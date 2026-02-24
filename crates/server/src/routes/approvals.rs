use axum::{
    Json, Router,
    extract::{Path, State},
    routing::post,
};
use deployment::Deployment;
use utils::approvals::{ApprovalResponse, ApprovalStatus};

use crate::{DeploymentImpl, error::ApiError};

pub async fn respond_to_approval(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
    Json(request): Json<ApprovalResponse>,
) -> Result<Json<ApprovalStatus>, ApiError> {
    let service = deployment.approvals();

    match service.respond(&deployment.db().pool, &id, request).await {
        Ok((status, _context)) => Ok(Json(status)),
        Err(e) => {
            tracing::error!("Failed to respond to approval: {:?}", e);
            Err(ApiError::Internal(
                "Failed to respond to approval".to_string(),
            ))
        }
    }
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new().route("/approvals/{id}/respond", post(respond_to_approval))
}
