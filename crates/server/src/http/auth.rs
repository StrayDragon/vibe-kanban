use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, Request, State},
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
};
use deployment::Deployment;
use url::form_urlencoded;
use utils::response::ApiResponse;

use crate::DeploymentImpl;

fn parse_authorization_bearer(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    let (prefix, rest) = trimmed.split_once(' ')?;
    if !prefix.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = rest.trim();
    if token.is_empty() {
        return None;
    }
    Some(token)
}

fn extract_query_token(req: &Request) -> Option<String> {
    let query = req.uri().query()?;
    for (key, value) in form_urlencoded::parse(query.as_bytes()) {
        if key == "token" {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return None;
            }
            return Some(trimmed.to_string());
        }
    }
    None
}

fn is_websocket_request(req: &Request) -> bool {
    req.headers()
        .get(header::UPGRADE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("websocket"))
}

fn is_sse_events_endpoint(req: &Request) -> bool {
    // This middleware is installed on the nested `/api` router, so paths are
    // relative to that prefix (e.g. `/events` instead of `/api/events`).
    req.uri().path().starts_with("/events")
}

fn peer_is_loopback(req: &Request) -> Option<bool> {
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|connect_info| connect_info.0.ip().is_loopback())
}

fn extract_request_token(req: &Request) -> Option<String> {
    // 1) Authorization: Bearer <token>
    if let Some(value) = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_authorization_bearer)
    {
        return Some(value.to_string());
    }

    // 2) X-API-Token: <token>
    if let Some(value) = req
        .headers()
        .get("x-api-token")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(value.to_string());
    }

    // 3) Query param token for EventSource / WebSocket
    if is_sse_events_endpoint(req) || is_websocket_request(req) {
        return extract_query_token(req);
    }

    None
}

pub async fn require_api_auth(
    State(deployment): State<DeploymentImpl>,
    req: Request,
    next: Next,
) -> Response {
    let access_control = {
        let config = deployment.config().read().await;
        config.access_control.clone()
    };

    if matches!(
        access_control.mode,
        services::services::config::AccessControlMode::Disabled
    ) {
        return next.run(req).await;
    }

    let Some(expected_token) = access_control.token.as_deref().filter(|t| !t.is_empty()) else {
        tracing::warn!(
            "accessControl.mode=TOKEN but accessControl.token is missing; treating as disabled"
        );
        return next.run(req).await;
    };

    let is_loopback = peer_is_loopback(&req).unwrap_or(false);
    if access_control.allow_localhost_bypass && is_loopback {
        return next.run(req).await;
    }

    let presented = extract_request_token(&req);
    if presented.as_deref() != Some(expected_token) {
        let peer = req
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|connect_info| connect_info.0.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let reason = if presented.is_none() {
            "missing_token"
        } else {
            "token_mismatch"
        };

        tracing::warn!(
            path = %req.uri().path(),
            method = %req.method(),
            peer = %peer,
            reason,
            "Unauthorized API request"
        );

        // Ensure all unauthorized requests return the standard ApiResponse
        // error envelope with a 401 status.
        let response = ApiResponse::<()>::error("Unauthorized");
        return (axum::http::StatusCode::UNAUTHORIZED, Json(response)).into_response();
    }

    next.run(req).await
}
