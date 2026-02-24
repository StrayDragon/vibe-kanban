use axum::{Router, middleware::from_fn_with_state, routing::get};

use crate::{DeploymentImpl, routes};

mod auth;
mod frontend;

pub fn router(deployment: DeploymentImpl) -> Router {
    let api_routes = Router::new()
        .merge(routes::config::router())
        .merge(routes::containers::router(&deployment))
        .merge(routes::projects::router(&deployment))
        .merge(routes::tasks::router(&deployment))
        .merge(routes::task_groups::router(&deployment))
        .merge(routes::task_attempts::router(&deployment))
        .merge(routes::execution_processes::router(&deployment))
        .merge(routes::tags::router(&deployment))
        .merge(routes::filesystem::router())
        .merge(routes::repo::router())
        .merge(routes::events::router(&deployment))
        .merge(routes::approvals::router())
        .merge(routes::scratch::router(&deployment))
        .merge(routes::sessions::router(&deployment))
        .merge(routes::translation::router())
        .nest("/images", routes::images::routes())
        .layer(from_fn_with_state(
            deployment.clone(),
            auth::require_api_auth,
        ));

    Router::new()
        .route("/health", get(routes::health::health_check))
        .route("/", get(frontend::serve_frontend_root))
        .route("/{*path}", get(frontend::serve_frontend))
        .nest("/api", api_routes)
        .with_state(deployment)
}
