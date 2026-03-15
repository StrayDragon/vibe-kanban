use app_runtime::Deployment;
use axum::{
    BoxError, Router,
    extract::{Query, State},
    response::{
        Sse,
        sse::{Event, KeepAlive},
    },
    routing::get,
};
use futures_util::{StreamExt, TryStreamExt};
use serde::Deserialize;

use crate::DeploymentImpl;

#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    pub after_seq: Option<u64>,
}

fn parse_last_event_id(headers: &axum::http::HeaderMap) -> Option<u64> {
    let raw = headers.get("last-event-id")?.to_str().ok()?;
    raw.trim().parse::<u64>().ok()
}

pub async fn events(
    State(deployment): State<DeploymentImpl>,
    headers: axum::http::HeaderMap,
    Query(query): Query<EventsQuery>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, BoxError>>>, axum::http::StatusCode>
{
    // Ask the container service for a combined "history + live" stream
    let resume_after_seq = query.after_seq.or_else(|| parse_last_event_id(&headers));
    let stream = deployment.stream_events(resume_after_seq).await;
    let shutdown = deployment.shutdown_token();
    let stream = stream
        .map_err(|e| -> BoxError { e.into() })
        .take_until(async move {
            shutdown.cancelled().await;
        });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

pub fn router(_: &DeploymentImpl) -> Router<DeploymentImpl> {
    let events_router = Router::new().route("/", get(events));

    Router::new().nest("/events", events_router)
}
