use std::{fmt::Display, future::Future};

use axum::{
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use db::{
    DBService,
    models::{
        execution_process::ExecutionProcess, project::Project, session::Session, tag::Tag,
        task::Task, task_group::TaskGroup, workspace::Workspace,
    },
};
use deployment::Deployment;
use uuid::Uuid;

pub trait ModelLoaderDeps {
    fn db_service(&self) -> &DBService;
}

impl<D> ModelLoaderDeps for D
where
    D: Deployment,
{
    fn db_service(&self) -> &DBService {
        self.db()
    }
}

async fn fetch_model_or_status<M, E, Fut>(
    model_name: &'static str,
    model_id: Uuid,
    load_future: Fut,
) -> Result<M, StatusCode>
where
    E: Display,
    Fut: Future<Output = Result<Option<M>, E>>,
{
    match load_future.await {
        Ok(Some(model)) => Ok(model),
        Ok(None) => {
            tracing::warn!("{model_name} {model_id} not found");
            Err(StatusCode::NOT_FOUND)
        }
        Err(error) => {
            tracing::error!("Failed to fetch {model_name} {model_id}: {error}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn load_request_extension<M, E, Fut>(
    request: Request,
    next: Next,
    model_name: &'static str,
    model_id: Uuid,
    load_future: Fut,
) -> Result<Response, StatusCode>
where
    M: Clone + Send + Sync + 'static,
    E: Display,
    Fut: Future<Output = Result<Option<M>, E>>,
{
    let model = fetch_model_or_status(model_name, model_id, load_future).await?;
    let mut request = request;
    request.extensions_mut().insert(model);
    Ok(next.run(request).await)
}

pub async fn load_project_middleware<S>(
    State(deployment): State<S>,
    Path(project_id): Path<Uuid>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode>
where
    S: ModelLoaderDeps,
{
    load_request_extension(
        request,
        next,
        "Project",
        project_id,
        Project::find_by_id(&deployment.db_service().pool, project_id),
    )
    .await
}

pub async fn load_task_middleware<S>(
    State(deployment): State<S>,
    Path(task_id): Path<Uuid>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode>
where
    S: ModelLoaderDeps,
{
    load_request_extension(
        request,
        next,
        "Task",
        task_id,
        Task::find_by_id(&deployment.db_service().pool, task_id),
    )
    .await
}

pub async fn load_task_group_middleware<S>(
    State(deployment): State<S>,
    Path(task_group_id): Path<Uuid>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode>
where
    S: ModelLoaderDeps,
{
    load_request_extension(
        request,
        next,
        "TaskGroup",
        task_group_id,
        TaskGroup::find_by_id(&deployment.db_service().pool, task_group_id),
    )
    .await
}

pub async fn load_workspace_middleware<S>(
    State(deployment): State<S>,
    Path(workspace_id): Path<Uuid>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode>
where
    S: ModelLoaderDeps,
{
    load_request_extension(
        request,
        next,
        "Workspace",
        workspace_id,
        Workspace::find_by_id(&deployment.db_service().pool, workspace_id),
    )
    .await
}

pub async fn load_execution_process_middleware<S>(
    State(deployment): State<S>,
    Path(process_id): Path<Uuid>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode>
where
    S: ModelLoaderDeps,
{
    load_request_extension(
        request,
        next,
        "ExecutionProcess",
        process_id,
        ExecutionProcess::find_by_id(&deployment.db_service().pool, process_id),
    )
    .await
}

pub async fn load_tag_middleware<S>(
    State(deployment): State<S>,
    Path(tag_id): Path<Uuid>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode>
where
    S: ModelLoaderDeps,
{
    load_request_extension(
        request,
        next,
        "Tag",
        tag_id,
        Tag::find_by_id(&deployment.db_service().pool, tag_id),
    )
    .await
}

pub async fn load_session_middleware<S>(
    State(deployment): State<S>,
    Path(session_id): Path<Uuid>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode>
where
    S: ModelLoaderDeps,
{
    load_request_extension(
        request,
        next,
        "Session",
        session_id,
        Session::find_by_id(&deployment.db_service().pool, session_id),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::fetch_model_or_status;

    #[tokio::test]
    async fn fetch_model_or_status_returns_not_found_on_missing_model() {
        let result = fetch_model_or_status::<String, &'static str, _>(
            "Project",
            uuid::Uuid::new_v4(),
            async { Ok(None) },
        )
        .await;

        assert_eq!(result.unwrap_err(), axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn fetch_model_or_status_returns_internal_error_on_fetch_failure() {
        let result = fetch_model_or_status::<String, &'static str, _>(
            "Project",
            uuid::Uuid::new_v4(),
            async { Err("db unavailable") },
        )
        .await;

        assert_eq!(
            result.unwrap_err(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
