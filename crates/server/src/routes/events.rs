use app_runtime::Deployment;
use axum::{
    BoxError, Router,
    extract::State,
    response::{
        Sse,
        sse::{Event, KeepAlive},
    },
    routing::get,
};
use futures_util::{StreamExt, TryStreamExt};

use crate::DeploymentImpl;

pub async fn events(
    State(deployment): State<DeploymentImpl>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, BoxError>>>, axum::http::StatusCode>
{
    // Ask the container service for a combined "history + live" stream
    let stream = deployment.stream_events().await;
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
