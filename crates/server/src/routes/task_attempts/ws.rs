use axum::{
    Extension,
    extract::{
        Query, State,
        ws::{WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use db::models::workspace::Workspace;
use deployment::Deployment;
use services::services::container::ContainerService;

use super::DiffStreamQuery;
use crate::DeploymentImpl;

#[axum::debug_handler]
pub async fn stream_task_attempt_diff_ws(
    ws: WebSocketUpgrade,
    Query(params): Query<DiffStreamQuery>,
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> impl IntoResponse {
    let options = services::services::container::DiffStreamOptions {
        stats_only: params.stats_only,
        force: params.force,
    };
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_task_attempt_diff_ws(socket, deployment, workspace, options).await {
            tracing::warn!("diff WS closed: {}", e);
        }
    })
}

async fn handle_task_attempt_diff_ws(
    socket: WebSocket,
    deployment: DeploymentImpl,
    workspace: Workspace,
    options: services::services::container::DiffStreamOptions,
) -> anyhow::Result<()> {
    use futures_util::{SinkExt, StreamExt, TryStreamExt};
    use utils::log_msg::LogMsg;

    let stream = deployment
        .container()
        .stream_diff(&workspace, options)
        .await?;

    let mut stream = stream.map_ok(|msg: LogMsg| msg.to_ws_message_unchecked());

    let (mut sender, mut receiver) = socket.split();

    loop {
        tokio::select! {
            item = stream.next() => {
                match item {
                    Some(Ok(msg)) => {
                        if sender.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        tracing::error!("stream error: {}", e);
                        break;
                    }
                    None => break,
                }
            }
            msg = receiver.next() => {
                if msg.is_none() {
                    break;
                }
            }
        }
    }

    let _ = sender.close().await;
    Ok(())
}
