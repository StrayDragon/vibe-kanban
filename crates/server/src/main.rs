use std::future::{Future, IntoFuture};

use anyhow::{self, Error as AnyhowError};
use app_runtime::{Deployment, DeploymentError};
use chrono::Utc;
use db::DbErr;
use execution::container::ContainerService;
use server::{DeploymentImpl, http};
use strip_ansi_escapes::strip;
use thiserror::Error;
use tokio::sync::watch;
use tracing_subscriber::{EnvFilter, prelude::*};
use utils_assets::asset_dir;
use utils_core::{browser::open_browser, port_file::write_port_file};

const GRACEFUL_SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const CLEANUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
const IDEMPOTENCY_PRUNE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60 * 60);
const DEFAULT_IDEMPOTENCY_IN_PROGRESS_TTL_SECS: i64 = 60 * 60;
const DEFAULT_IDEMPOTENCY_COMPLETED_TTL_SECS: i64 = 60 * 60 * 24 * 7;
const IDEMPOTENCY_IN_PROGRESS_TTL_ENV: &str = "VK_IDEMPOTENCY_IN_PROGRESS_TTL_SECS";
const IDEMPOTENCY_COMPLETED_TTL_ENV: &str = "VK_IDEMPOTENCY_COMPLETED_TTL_SECS";
const OPEN_BROWSER_STARTUP_ENV: &str = "VK_OPEN_BROWSER_STARTUP";

#[derive(Debug, Error)]
pub enum VibeKanbanError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error(transparent)]
    Deployment(#[from] DeploymentError),
    #[error(transparent)]
    Other(#[from] AnyhowError),
}

fn spawn_background<F>(task: F) -> tokio::task::JoinHandle<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(task)
}

fn env_var_truthy(name: &str) -> bool {
    let raw = match std::env::var(name) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let value = raw.trim();
    if value.is_empty() {
        return false;
    }

    let normalized = value.to_ascii_lowercase();
    match normalized.as_str() {
        "1" | "true" | "t" | "yes" | "y" | "on" => true,
        "0" | "false" | "f" | "no" | "n" | "off" => false,
        _ => {
            tracing::warn!(
                env = name,
                value = value,
                "Unrecognized boolean env var value, treating as false"
            );
            false
        }
    }
}

fn print_cli_help() {
    println!(
        r#"VK Server

Usage:
  server                           # start backend server (no CLI args; configure via env + config.yaml)
  server --help

Legacy (DEPRECATED; will be removed):
  server legacy export-db-projects-yaml [--install|--out <path>|--print-paths] [--dry-run]
  server legacy export-asset-config-yaml [--install|--out <path>|--print-paths] [--dry-run]

Run `server legacy export-db-projects-yaml --help` or `server legacy export-asset-config-yaml --help` for details.
"#
    );
}

async fn maybe_run_cli_command() -> Result<bool, AnyhowError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return Ok(false);
    }

    let first = args[0].as_str();
    if matches!(first, "--help" | "-h" | "help") {
        print_cli_help();
        return Ok(true);
    }

    if first == "legacy" {
        let sub = args.get(1).map(String::as_str);
        match sub {
            None | Some("--help" | "-h" | "help") => {
                print_cli_help();
                return Ok(true);
            }
            Some("export-db-projects-yaml") => {
                let (parsed, action) =
                    server::legacy_migrations::parse_export_db_projects_yaml_args(
                        args.into_iter().skip(2),
                    )?;

                if action == server::legacy_migrations::ExportDbProjectsYamlParseResult::Help {
                    println!(
                        "{}",
                        server::legacy_migrations::export_db_projects_yaml_help()
                    );
                    return Ok(true);
                }

                server::legacy_migrations::run_export_db_projects_yaml(parsed).await?;
                return Ok(true);
            }
            Some("export-asset-config-yaml") => {
                let (parsed, action) =
                    server::legacy_migrations::parse_export_asset_config_yaml_args(
                        args.into_iter().skip(2),
                    )?;

                if action == server::legacy_migrations::ExportAssetConfigYamlParseResult::Help {
                    println!(
                        "{}",
                        server::legacy_migrations::export_asset_config_yaml_help()
                    );
                    return Ok(true);
                }

                server::legacy_migrations::run_export_asset_config_yaml(parsed).await?;
                return Ok(true);
            }
            Some(other) => {
                anyhow::bail!("Unknown legacy subcommand: {other}");
            }
        }
    }

    anyhow::bail!(
        "Unknown CLI arguments: {:?}. Run `server --help` for supported commands.",
        args
    );
}

#[tokio::main]
async fn main() -> Result<(), VibeKanbanError> {
    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let filter_string = format!(
        "warn,server={level},services={level},db={level},executors={level},deployment={level},local_deployment={level},utils={level}",
        level = log_level
    );
    let env_filter = EnvFilter::try_new(filter_string).expect("Failed to create tracing filter");
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(env_filter))
        .init();

    // Create asset directory if it doesn't exist
    if !asset_dir().exists() {
        std::fs::create_dir_all(asset_dir())?;
    }

    if maybe_run_cli_command().await? {
        return Ok(());
    }

    let deployment = DeploymentImpl::new().await?;
    deployment.log_cache_budgets();
    deployment
        .container()
        .cleanup_orphan_executions()
        .await
        .map_err(DeploymentError::from)?;
    deployment
        .container()
        .backfill_before_head_commits()
        .await
        .map_err(DeploymentError::from)?;
    deployment
        .container()
        .backfill_repo_names()
        .await
        .map_err(DeploymentError::from)?;
    let deployment_for_logs = deployment.clone();
    spawn_background(async move {
        if let Err(err) = deployment_for_logs
            .container()
            .backfill_log_entries_startup()
            .await
        {
            tracing::warn!("Failed to backfill legacy log entries: {}", err);
        }
        if let Err(err) = deployment_for_logs
            .container()
            .cleanup_legacy_jsonl_logs()
            .await
        {
            tracing::warn!("Failed to cleanup legacy JSONL logs: {}", err);
        }
    });
    deployment.spawn_pr_monitor_service().await;
    let _auto_orchestrator_handle = server::auto_orchestrator::spawn(deployment.clone());
    // Pre-warm file search cache for most active projects
    let deployment_for_cache = deployment.clone();
    tokio::spawn(async move {
        let mut repo_paths = {
            let config = deployment_for_cache.config().read().await;
            config
                .projects
                .iter()
                .flat_map(|project| project.repos.iter())
                .map(|repo| std::path::PathBuf::from(repo.path.clone()))
                .collect::<Vec<_>>()
        };

        repo_paths.sort();
        repo_paths.dedup();

        if repo_paths.is_empty() {
            return;
        }

        if let Err(e) = deployment_for_cache
            .file_search_cache()
            .warm_repos(repo_paths)
            .await
        {
            tracing::warn!("Failed to warm file search cache: {}", e);
        }
    });

    let idempotency_pool = deployment.db().pool.clone();
    let idempotency_shutdown = deployment.shutdown_token();
    spawn_background(async move {
        let in_progress_ttl_secs = read_ttl_secs(
            IDEMPOTENCY_IN_PROGRESS_TTL_ENV,
            DEFAULT_IDEMPOTENCY_IN_PROGRESS_TTL_SECS,
        );
        let completed_ttl_secs = read_ttl_secs(
            IDEMPOTENCY_COMPLETED_TTL_ENV,
            DEFAULT_IDEMPOTENCY_COMPLETED_TTL_SECS,
        );
        tracing::info!(
            in_progress_ttl_secs = in_progress_ttl_secs.unwrap_or(0),
            completed_ttl_secs = completed_ttl_secs.unwrap_or(0),
            "Starting idempotency key retention job"
        );

        loop {
            let prune_result = tokio::select! {
                _ = idempotency_shutdown.cancelled() => {
                    tracing::info!("Stopping idempotency key retention job");
                    break;
                }
                result = prune_idempotency_keys_once(
                    &idempotency_pool,
                    in_progress_ttl_secs,
                    completed_ttl_secs,
                ) => result,
            };

            if let Err(err) = prune_result {
                tracing::warn!(error = %err, "Failed to prune idempotency keys");
            }

            tokio::select! {
                _ = idempotency_shutdown.cancelled() => {
                    tracing::info!("Stopping idempotency key retention job");
                    break;
                }
                _ = tokio::time::sleep(IDEMPOTENCY_PRUNE_INTERVAL) => {}
            }
        }
    });

    let app_router = http::router(deployment.clone());

    let port = std::env::var("BACKEND_PORT")
        .or_else(|_| std::env::var("PORT"))
        .ok()
        .and_then(|s| {
            // remove any ANSI codes, then turn into String
            let cleaned =
                String::from_utf8(strip(s.as_bytes())).expect("UTF-8 after stripping ANSI");
            cleaned.trim().parse::<u16>().ok()
        })
        .unwrap_or_else(|| {
            tracing::info!("No PORT environment variable set, using port 0 for auto-assignment");
            0
        }); // Use 0 to find free port if no specific port provided

    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}")).await?;
    let actual_port = listener.local_addr()?.port(); // get → 53427 (example)

    // Write port file for discovery if prod, warn on fail
    if let Err(e) = write_port_file(actual_port).await {
        tracing::warn!("Failed to write port file: {}", e);
    }

    tracing::info!("Server listening on http://{host}:{actual_port}");
    // `HOST` defaults to 0.0.0.0 in `just run`, which isn't directly openable as a URL.
    // Always print a local URL that's clickable in most terminals.
    tracing::info!("Open http://127.0.0.1:{actual_port}");
    if env_var_truthy(OPEN_BROWSER_STARTUP_ENV) {
        tracing::info!("{OPEN_BROWSER_STARTUP_ENV}=true, opening browser...");
        tokio::spawn(async move {
            if let Err(e) = open_browser(&format!("http://127.0.0.1:{actual_port}")).await {
                tracing::warn!(
                    "Failed to open browser automatically: {}. Please open http://127.0.0.1:{} manually.",
                    e,
                    actual_port
                );
            }
        });
    }

    let (shutdown_rx, force_exit_rx) = spawn_shutdown_watchers();
    let deployment_for_shutdown = deployment.clone();
    let shutdown_bridge_rx = shutdown_rx.clone();
    tokio::spawn(async move {
        wait_for_watch_true(shutdown_bridge_rx).await;
        deployment_for_shutdown.begin_shutdown();
    });

    let server = axum::serve(
        listener,
        app_router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(wait_for_watch_true(shutdown_rx.clone()))
    .into_future();
    tokio::pin!(server);

    let serve_result = tokio::select! {
        res = &mut server => res,
        _ = wait_for_watch_true(force_exit_rx.clone()) => {
            tracing::warn!("Force shutdown requested (second signal), exiting immediately");
            std::process::exit(130);
        }
        _ = shutdown_deadline(shutdown_rx.clone(), GRACEFUL_SHUTDOWN_TIMEOUT) => {
            tracing::warn!(
                "Graceful shutdown timed out after {:?}, exiting immediately",
                GRACEFUL_SHUTDOWN_TIMEOUT
            );
            std::process::exit(130);
        }
    };

    serve_result?;

    tokio::select! {
        _ = perform_cleanup_actions(&deployment) => {}
        _ = wait_for_watch_true(force_exit_rx.clone()) => {
            tracing::warn!("Force shutdown requested during cleanup, exiting immediately");
            std::process::exit(130);
        }
        _ = tokio::time::sleep(CLEANUP_TIMEOUT) => {
            tracing::warn!("Cleanup timed out after {:?}, exiting immediately", CLEANUP_TIMEOUT);
            std::process::exit(130);
        }
    }

    if *shutdown_rx.borrow() {
        std::process::exit(0);
    }

    Ok(())
}

pub async fn perform_cleanup_actions(deployment: &DeploymentImpl) {
    if let Err(e) = deployment.container().kill_all_running_processes().await {
        tracing::warn!("Failed to cleanly kill running execution processes: {e}");
    }
}

fn spawn_shutdown_watchers() -> (watch::Receiver<bool>, watch::Receiver<bool>) {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (force_exit_tx, force_exit_rx) = watch::channel(false);

    tokio::spawn(async move {
        let mut shutdown_sent = false;

        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};

            let mut sigint = match signal(SignalKind::interrupt()) {
                Ok(sig) => sig,
                Err(e) => {
                    tracing::error!("Failed to install SIGINT handler: {e}");
                    return;
                }
            };

            let mut sigterm = match signal(SignalKind::terminate()) {
                Ok(sig) => Some(sig),
                Err(e) => {
                    tracing::error!("Failed to install SIGTERM handler: {e}");
                    None
                }
            };

            loop {
                tokio::select! {
                    _ = sigint.recv() => {},
                    _ = async {
                        if let Some(sigterm) = sigterm.as_mut() {
                            sigterm.recv().await;
                        } else {
                            std::future::pending::<()>().await;
                        }
                    } => {},
                }

                if !shutdown_sent {
                    shutdown_sent = true;
                    tracing::info!(
                        "Shutdown signal received, starting graceful shutdown (press Ctrl+C again to force)"
                    );
                    let _ = shutdown_tx.send(true);
                } else {
                    tracing::warn!("Second shutdown signal received, forcing exit");
                    let _ = force_exit_tx.send(true);
                    break;
                }
            }
        }

        #[cfg(not(unix))]
        {
            if let Err(e) = tokio::signal::ctrl_c().await {
                tracing::error!("Failed to install Ctrl+C handler: {e}");
                return;
            }

            tracing::info!(
                "Shutdown signal received, starting graceful shutdown (press Ctrl+C again to force)"
            );
            let _ = shutdown_tx.send(true);

            if let Err(e) = tokio::signal::ctrl_c().await {
                tracing::error!("Failed to install Ctrl+C handler: {e}");
                return;
            }

            tracing::warn!("Second shutdown signal received, forcing exit");
            let _ = force_exit_tx.send(true);
        }
    });

    (shutdown_rx, force_exit_rx)
}

fn read_ttl_secs(name: &str, default: i64) -> Option<i64> {
    let raw = match std::env::var(name) {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return Some(default),
        Err(err) => {
            tracing::warn!(error = %err, "Failed to read {name}; using default");
            return Some(default);
        }
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        tracing::warn!("{name} is set but empty; using default");
        return Some(default);
    }

    match trimmed.parse::<i64>() {
        Ok(value) if value <= 0 => None,
        Ok(value) => Some(value),
        Err(err) => {
            tracing::warn!(value = trimmed, error = %err, "Invalid {name}; using default");
            Some(default)
        }
    }
}

async fn prune_idempotency_keys_once(
    db: &db::DbPool,
    in_progress_ttl_secs: Option<i64>,
    completed_ttl_secs: Option<i64>,
) -> Result<(), db::DbErr> {
    let now = Utc::now();

    let mut removed_in_progress = 0u64;
    if let Some(ttl_secs) = in_progress_ttl_secs {
        let cutoff = now - chrono::Duration::seconds(ttl_secs);
        removed_in_progress = db::models::idempotency::prune_in_progress_before(db, cutoff).await?;
    }

    let mut removed_completed = 0u64;
    if let Some(ttl_secs) = completed_ttl_secs {
        let cutoff = now - chrono::Duration::seconds(ttl_secs);
        removed_completed = db::models::idempotency::prune_completed_before(db, cutoff).await?;
    }

    if removed_in_progress > 0 || removed_completed > 0 {
        tracing::info!(
            removed_in_progress,
            removed_completed,
            "Pruned idempotency keys"
        );
    }

    Ok(())
}

async fn wait_for_watch_true(mut rx: watch::Receiver<bool>) {
    loop {
        if *rx.borrow() {
            return;
        }

        if rx.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

async fn shutdown_deadline(rx: watch::Receiver<bool>, timeout: std::time::Duration) {
    wait_for_watch_true(rx).await;
    tokio::time::sleep(timeout).await;
}

#[cfg(test)]
mod tests {
    use tokio::sync::oneshot;

    use super::spawn_background;

    #[tokio::test]
    async fn spawn_background_returns_immediately() {
        let (tx, rx) = oneshot::channel::<()>();

        let handle = spawn_background(async move {
            let _ = rx.await;
        });
        assert!(!handle.is_finished());

        let _ = tx.send(());
        let _ = handle.await;
    }
}
