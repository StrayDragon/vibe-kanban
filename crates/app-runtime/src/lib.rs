use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

mod notification;
use anyhow::Error as AnyhowError;
use async_trait::async_trait;
use axum::response::sse::Event;
use config::{
    Config, ConfigError, cache_budget::cache_budgets, load_config_from_file, save_config_to_file,
};
use db::{
    DBService, DbErr,
    models::{
        project::{CreateProject, Project},
        project_repo::CreateProjectRepo,
        workspace::WorkspaceError,
    },
};
use events::{EventError, EventService};
use execution::{
    container::{
        ContainerError, ContainerService, LocalContainerService, log_backfill_completion_cache_len,
    },
    image::{ImageError, ImageService},
    pr_monitor::PrMonitorService,
    queued_message::QueuedMessageService,
};
use executors::{executors::ExecutorError, profile::ExecutorConfigs};
use futures::StreamExt;
use logs_axum::SequencedLogMsgAxumExt;
use logs_store::MsgStore;
pub use notification::NotificationService;
use repos::{
    file_ranker::file_stats_cache_len,
    file_search_cache::FileSearchCache,
    filesystem::{FilesystemError, FilesystemService},
    filesystem_watcher::FilesystemWatcherError,
    git::{GitService, GitServiceError},
    project::ProjectService,
    repo::RepoService,
    worktree_manager::WorktreeError,
};
use tasks::approvals::Approvals;
use thiserror::Error;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use utils_assets::config_path;
use utils_core::notifications::SharedNotifier;
use uuid::Uuid;

const DISABLE_BACKGROUND_TASKS_ENV: &str = "VIBE_DISABLE_BACKGROUND_TASKS";

fn background_tasks_disabled() -> bool {
    match std::env::var(DISABLE_BACKGROUND_TASKS_ENV) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        ),
        Err(_) => false,
    }
}

#[derive(Debug, Error)]
pub enum DeploymentError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error(transparent)]
    GitServiceError(#[from] GitServiceError),
    #[error(transparent)]
    FilesystemWatcherError(#[from] FilesystemWatcherError),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    Container(#[from] ContainerError),
    #[error(transparent)]
    Executor(#[from] ExecutorError),
    #[error(transparent)]
    Image(#[from] ImageError),
    #[error(transparent)]
    Filesystem(#[from] FilesystemError),
    #[error(transparent)]
    Worktree(#[from] WorktreeError),
    #[error(transparent)]
    Event(#[from] EventError),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Other(#[from] AnyhowError),
}

#[async_trait]
pub trait Deployment: Clone + Send + Sync + 'static {
    async fn new() -> Result<Self, DeploymentError>;

    fn config(&self) -> &Arc<RwLock<Config>>;

    fn db(&self) -> &DBService;

    fn container(&self) -> &impl ContainerService;

    fn git(&self) -> &GitService;

    fn project(&self) -> &ProjectService;

    fn repo(&self) -> &RepoService;

    fn image(&self) -> &ImageService;

    fn filesystem(&self) -> &FilesystemService;

    fn events(&self) -> &EventService;

    fn file_search_cache(&self) -> &Arc<FileSearchCache>;

    fn approvals(&self) -> &Approvals;

    fn queued_message_service(&self) -> &QueuedMessageService;

    fn shutdown_token(&self) -> CancellationToken {
        CancellationToken::new()
    }

    async fn spawn_pr_monitor_service(&self) -> tokio::task::JoinHandle<()> {
        let db = self.db().clone();
        PrMonitorService::spawn(db, self.shutdown_token()).await
    }

    async fn trigger_auto_project_setup(&self) {
        let soft_timeout_ms = 2_000;
        let hard_timeout_ms = 2_300;
        let project_count = Project::count(&self.db().pool).await.unwrap_or(0);

        if project_count != 0 {
            return;
        }

        let Ok(repos) = self
            .filesystem()
            .list_common_git_repos(soft_timeout_ms, hard_timeout_ms, Some(4))
            .await
        else {
            return;
        };

        for repo in repos.into_iter().take(3) {
            let project_name = repo.name.clone();
            let repo_path = repo.path.to_string_lossy().to_string();

            let create_data = CreateProject {
                name: project_name,
                repositories: vec![CreateProjectRepo {
                    display_name: repo.name,
                    git_repo_path: repo_path.clone(),
                }],
            };

            match self
                .project()
                .create_project(&self.db().pool, self.repo(), create_data.clone())
                .await
            {
                Ok(project) => {
                    tracing::info!("Auto-created project '{}' from {}", project.name, repo_path);
                }
                Err(error) => {
                    tracing::warn!(
                        "Failed to auto-create project from {}: {}",
                        repo.path.display(),
                        error
                    );
                }
            }
        }
    }

    async fn stream_events(
        &self,
    ) -> futures::stream::BoxStream<'static, Result<Event, std::io::Error>> {
        let (history, receiver, _meta) = self
            .events()
            .msg_store()
            .subscribe_sequenced_from(None);

        let initial_last_seq = history.last().map(|m| m.seq).unwrap_or(0);
        let last_seq = Arc::new(AtomicU64::new(initial_last_seq));

        let hist = futures::stream::iter(history.into_iter().map(Ok::<_, std::io::Error>));
        let live = futures::stream::unfold((receiver, last_seq), |(mut receiver, last_seq)| async move {
            loop {
                match receiver.recv().await {
                    Ok(msg) => {
                        if msg.seq <= last_seq.load(Ordering::Relaxed) {
                            continue;
                        }
                        last_seq.store(msg.seq, Ordering::Relaxed);
                        return Some((Ok::<_, std::io::Error>(msg), (receiver, last_seq)));
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(
                            skipped = skipped,
                            "events SSE receiver lagged; dropping missed messages"
                        );
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return None;
                    }
                }
            }
        });

        let sequenced_stream = hist.chain(live);

        sequenced_stream
            .flat_map(|result| match result {
                Ok(msg) => {
                    let mut events: Vec<Result<Event, std::io::Error>> = Vec::with_capacity(2);
                    events.push(Ok(msg.to_sse_event()));
                    if let Some(invalidate) = msg.to_invalidate_sse_event() {
                        events.push(Ok(invalidate));
                    }
                    futures::stream::iter(events)
                }
                Err(err) => futures::stream::iter(vec![Err(err)]),
            })
            .boxed()
    }
}

#[derive(Clone)]
pub struct AppRuntime {
    config: Arc<RwLock<Config>>,
    db: DBService,
    container: LocalContainerService,
    git: GitService,
    project: ProjectService,
    repo: RepoService,
    image: ImageService,
    filesystem: FilesystemService,
    events: EventService,
    file_search_cache: Arc<FileSearchCache>,
    approvals: Approvals,
    queued_message_service: QueuedMessageService,
    shutdown_token: CancellationToken,
}

struct CoreServices {
    git: GitService,
    project: ProjectService,
    repo: RepoService,
    filesystem: FilesystemService,
    file_search_cache: Arc<FileSearchCache>,
    msg_stores: Arc<RwLock<HashMap<Uuid, Arc<MsgStore>>>>,
    approvals: Approvals,
    queued_message_service: QueuedMessageService,
}

struct RuntimeServices {
    db: DBService,
    image: ImageService,
    events: EventService,
    container: LocalContainerService,
    shutdown_token: CancellationToken,
}

#[async_trait]
impl Deployment for AppRuntime {
    async fn new() -> Result<Self, DeploymentError> {
        let config = Self::load_runtime_config().await?;
        let core = Self::build_core_services();
        let runtime = Self::build_runtime_services(config.clone(), &core).await?;

        let CoreServices {
            git,
            project,
            repo,
            filesystem,
            file_search_cache,
            msg_stores: _msg_stores,
            approvals,
            queued_message_service,
        } = core;

        let RuntimeServices {
            db,
            image,
            events,
            container,
            shutdown_token,
        } = runtime;

        let deployment = Self {
            config,
            db,
            container,
            git,
            project,
            repo,
            image,
            filesystem,
            events,
            file_search_cache,
            approvals,
            queued_message_service,
            shutdown_token,
        };

        Ok(deployment)
    }

    fn config(&self) -> &Arc<RwLock<Config>> {
        &self.config
    }

    fn db(&self) -> &DBService {
        &self.db
    }

    fn container(&self) -> &impl ContainerService {
        &self.container
    }

    fn git(&self) -> &GitService {
        &self.git
    }

    fn project(&self) -> &ProjectService {
        &self.project
    }

    fn repo(&self) -> &RepoService {
        &self.repo
    }

    fn image(&self) -> &ImageService {
        &self.image
    }

    fn filesystem(&self) -> &FilesystemService {
        &self.filesystem
    }

    fn events(&self) -> &EventService {
        &self.events
    }

    fn file_search_cache(&self) -> &Arc<FileSearchCache> {
        &self.file_search_cache
    }

    fn approvals(&self) -> &Approvals {
        &self.approvals
    }

    fn queued_message_service(&self) -> &QueuedMessageService {
        &self.queued_message_service
    }

    fn shutdown_token(&self) -> CancellationToken {
        self.shutdown_token.clone()
    }
}

impl AppRuntime {
    async fn load_runtime_config() -> Result<Arc<RwLock<Config>>, DeploymentError> {
        let mut raw_config = load_config_from_file(&config_path()).await;

        let profiles = ExecutorConfigs::get_cached();
        executors_core::agent_command::agent_command_resolver().warm_cache();
        if !raw_config.onboarding_acknowledged
            && let Ok(recommended_executor) = profiles.get_recommended_executor_profile().await
        {
            raw_config.executor_profile = recommended_executor;
        }

        Self::update_app_version_state(&mut raw_config, utils_core::version::APP_VERSION);
        save_config_to_file(&raw_config, &config_path()).await?;

        Ok(Arc::new(RwLock::new(raw_config)))
    }

    fn update_app_version_state(config: &mut Config, current_version: &str) {
        // This fork does not ship an external release notes flow. Ensure the
        // legacy flag is cleared so the frontend never attempts to load hosted
        // content.
        config.show_release_notes = false;

        let stored_version = config.last_app_version.as_deref();
        if stored_version != Some(current_version) {
            config.last_app_version = Some(current_version.to_string());
        }
    }

    fn build_core_services() -> CoreServices {
        let msg_stores = Arc::new(RwLock::new(HashMap::new()));
        let approvals = Approvals::new(msg_stores.clone());

        CoreServices {
            git: GitService::new(),
            project: ProjectService::new(),
            repo: RepoService::new(),
            filesystem: FilesystemService::new(),
            file_search_cache: Arc::new(FileSearchCache::new()),
            msg_stores,
            approvals,
            queued_message_service: QueuedMessageService::new(),
        }
    }

    async fn build_runtime_services(
        config: Arc<RwLock<Config>>,
        core: &CoreServices,
    ) -> Result<RuntimeServices, DeploymentError> {
        let shutdown_token = CancellationToken::new();
        let notification_service: SharedNotifier =
            Arc::new(NotificationService::new(config.clone()));
        let db = DBService::new().await?;
        let image = ImageService::new(db.clone().pool)?;
        if !background_tasks_disabled() {
            Self::spawn_orphaned_image_cleanup(image.clone());
        }

        let events = EventService::new(
            db.clone(),
            Arc::new(MsgStore::new()),
            Arc::new(RwLock::new(0)),
            shutdown_token.clone(),
        );

        let container = LocalContainerService::new(
            db.clone(),
            core.msg_stores.clone(),
            config,
            core.git.clone(),
            image.clone(),
            core.approvals.clone(),
            core.queued_message_service.clone(),
            notification_service,
            shutdown_token.clone(),
        )
        .await;

        Ok(RuntimeServices {
            db,
            image,
            events,
            container,
            shutdown_token,
        })
    }

    fn spawn_orphaned_image_cleanup(image_service: ImageService) {
        tokio::spawn(async move {
            tracing::info!("Starting orphaned image cleanup...");
            if let Err(e) = image_service.delete_orphaned_images().await {
                tracing::error!("Failed to clean up orphaned images: {}", e);
            }
        });
    }

    pub fn log_cache_budgets(&self) {
        let budgets = cache_budgets();
        let file_search_entries = self.file_search_cache.cache_entry_count();
        let file_search_watchers = self.file_search_cache.watcher_count();
        let file_stats_entries = file_stats_cache_len();
        let approvals_waiters = self.approvals.pending_len();
        let queued_messages = self.queued_message_service.queue_len();
        let log_backfill_entries = log_backfill_completion_cache_len();

        tracing::info!(
            cache = "file_search_cache",
            max_entries = budgets.file_search_cache_max_repos,
            ttl_secs = budgets.file_search_cache_ttl.as_secs(),
            current_entries = file_search_entries,
            "Cache budget"
        );
        tracing::info!(
            cache = "file_search_watchers",
            max_entries = budgets.file_search_watchers_max,
            ttl_secs = budgets.file_search_watcher_ttl.as_secs(),
            current_entries = file_search_watchers,
            "Cache budget"
        );
        tracing::info!(
            cache = "file_stats_cache",
            max_entries = budgets.file_stats_cache_max_repos,
            ttl_secs = budgets.file_stats_cache_ttl.as_secs(),
            current_entries = file_stats_entries,
            "Cache budget"
        );
        tracing::info!(
            cache = "approvals_waiters",
            current_entries = approvals_waiters,
            "Cache budget"
        );
        tracing::info!(
            cache = "queued_messages",
            ttl_secs = budgets.queued_messages_ttl.as_secs(),
            current_entries = queued_messages,
            "Cache budget"
        );
        tracing::info!(
            cache = "log_backfill_completion",
            max_entries = budgets.log_backfill_completion_max_entries,
            ttl_secs = budgets.log_backfill_completion_ttl.as_secs(),
            current_entries = log_backfill_entries,
            "Cache budget"
        );
        tracing::info!(
            cache = "cache_warnings",
            warn_at_ratio = budgets.cache_warn_at_ratio,
            sample_secs = budgets.cache_warn_sample.as_secs(),
            "Cache warning thresholds"
        );
    }

    pub fn shutdown_token(&self) -> CancellationToken {
        self.shutdown_token.clone()
    }

    pub fn begin_shutdown(&self) {
        self.shutdown_token.cancel();
    }

    pub async fn spawn_pr_monitor_service(&self) -> tokio::task::JoinHandle<()> {
        PrMonitorService::spawn(self.db.clone(), self.shutdown_token()).await
    }
}

#[cfg(test)]
mod tests {
    use config::Config;

    use super::AppRuntime;

    #[test]
    fn update_app_version_state_clears_release_notes_flag() {
        let mut config = Config {
            last_app_version: Some("0.0.100".to_string()),
            show_release_notes: true,
            ..Config::default()
        };

        AppRuntime::update_app_version_state(&mut config, "0.0.101");

        assert_eq!(config.last_app_version.as_deref(), Some("0.0.101"));
        assert!(!config.show_release_notes);
    }

    #[test]
    fn update_app_version_state_does_not_flip_on_same_version() {
        let mut config = Config {
            last_app_version: Some("0.0.101".to_string()),
            show_release_notes: true,
            ..Config::default()
        };

        AppRuntime::update_app_version_state(&mut config, "0.0.101");

        assert_eq!(config.last_app_version.as_deref(), Some("0.0.101"));
        assert!(!config.show_release_notes);
    }
}
