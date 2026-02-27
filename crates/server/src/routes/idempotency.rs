use axum::{http::HeaderMap, response::Json as ResponseJson};
use chrono::Duration as ChronoDuration;
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use utils::response::ApiResponse;

use crate::error::ApiError;

pub const IDEMPOTENCY_KEY_HEADER: &str = "Idempotency-Key";
const DEFAULT_IDEMPOTENCY_IN_PROGRESS_TTL_SECS: i64 = 60 * 60;
const IDEMPOTENCY_IN_PROGRESS_TTL_ENV: &str = "VK_IDEMPOTENCY_IN_PROGRESS_TTL_SECS";

fn idempotency_in_progress_ttl() -> Option<ChronoDuration> {
    let raw = match std::env::var(IDEMPOTENCY_IN_PROGRESS_TTL_ENV) {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => {
            return Some(ChronoDuration::seconds(
                DEFAULT_IDEMPOTENCY_IN_PROGRESS_TTL_SECS,
            ));
        }
        Err(err) => {
            tracing::warn!(
                error = %err,
                "Failed to read {IDEMPOTENCY_IN_PROGRESS_TTL_ENV}; using default"
            );
            return Some(ChronoDuration::seconds(
                DEFAULT_IDEMPOTENCY_IN_PROGRESS_TTL_SECS,
            ));
        }
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        tracing::warn!("{IDEMPOTENCY_IN_PROGRESS_TTL_ENV} is set but empty; using default");
        return Some(ChronoDuration::seconds(
            DEFAULT_IDEMPOTENCY_IN_PROGRESS_TTL_SECS,
        ));
    }

    match trimmed.parse::<i64>() {
        Ok(value) if value <= 0 => None,
        Ok(value) => Some(ChronoDuration::seconds(value)),
        Err(err) => {
            tracing::warn!(
                value = trimmed,
                error = %err,
                "Invalid {IDEMPOTENCY_IN_PROGRESS_TTL_ENV}; using default"
            );
            Some(ChronoDuration::seconds(
                DEFAULT_IDEMPOTENCY_IN_PROGRESS_TTL_SECS,
            ))
        }
    }
}

pub fn idempotency_key(headers: &HeaderMap) -> Option<String> {
    headers
        .get(IDEMPOTENCY_KEY_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn request_hash<T: Serialize>(payload: &T) -> Result<String, ApiError> {
    let bytes = serde_json::to_vec(payload).map_err(|e| {
        ApiError::Internal(format!(
            "Failed to serialize request payload for hashing: {e}"
        ))
    })?;
    let digest = Sha256::digest(&bytes);
    Ok(format!("{digest:x}"))
}

pub async fn idempotent_success<T, F, Fut>(
    db: &db::DbPool,
    scope: &'static str,
    key: Option<String>,
    request_hash: String,
    execute: F,
) -> Result<ResponseJson<ApiResponse<T>>, ApiError>
where
    T: Serialize + DeserializeOwned,
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, ApiError>>,
{
    let Some(key) = key else {
        let data = execute().await?;
        return Ok(ResponseJson(ApiResponse::success(data)));
    };

    match db::models::idempotency::begin(
        db,
        scope,
        &key,
        &request_hash,
        idempotency_in_progress_ttl(),
    )
    .await?
    {
        db::models::idempotency::IdempotencyBeginOutcome::New { record_uuid } => {
            let result = execute().await;
            match result {
                Ok(data) => {
                    let response = ApiResponse::success(data);
                    let response_json = serde_json::to_string(&response).map_err(|e| {
                        ApiError::Internal(format!(
                            "Failed to serialize idempotent response payload: {e}"
                        ))
                    })?;
                    db::models::idempotency::complete(db, record_uuid, 200, response_json).await?;
                    Ok(ResponseJson(response))
                }
                Err(err) => {
                    // Best-effort cleanup so retries can proceed.
                    if let Err(cleanup_err) = db::models::idempotency::delete(db, record_uuid).await
                    {
                        tracing::warn!(
                            record_uuid = %record_uuid,
                            error = %cleanup_err,
                            "Failed to delete idempotency record after error"
                        );
                    }
                    Err(err)
                }
            }
        }
        db::models::idempotency::IdempotencyBeginOutcome::Existing { record } => {
            if record.request_hash != request_hash {
                return Err(ApiError::Conflict(
                    "Idempotency key already used with different request parameters".to_string(),
                ));
            }

            match record.state.as_str() {
                db::models::idempotency::IDEMPOTENCY_STATE_COMPLETED => {
                    let Some(response_json) = record.response_json else {
                        return Err(ApiError::Internal(
                            "Idempotency record is completed but has no stored response"
                                .to_string(),
                        ));
                    };
                    let response: ApiResponse<T> =
                        serde_json::from_str(&response_json).map_err(|e| {
                            ApiError::Internal(format!("Failed to parse stored response: {e}"))
                        })?;
                    Ok(ResponseJson(response))
                }
                db::models::idempotency::IDEMPOTENCY_STATE_IN_PROGRESS => Err(ApiError::Conflict(
                    "Request with this idempotency key is in progress. Retry shortly.".to_string(),
                )),
                other => Err(ApiError::Internal(format!(
                    "Unknown idempotency record state: {other}"
                ))),
            }
        }
    }
}
