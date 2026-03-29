use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::SystemTime,
};

mod notification;
use anyhow::Error as AnyhowError;
use async_trait::async_trait;
use axum::response::sse::Event;
use config::{Config, ConfigError, cache_budget::cache_budgets};
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
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use utils_core::notifications::SharedNotifier;
use uuid::Uuid;

const CONFIG_WATCH_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(250);

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

    fn config_status(&self) -> &Arc<RwLock<RuntimeConfigStatus>>;

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
        resume_after_seq: Option<u64>,
    ) -> futures::stream::BoxStream<'static, Result<Event, std::io::Error>> {
        fn can_resume_from(after_seq: u64, meta: logs_store::SequencedHistoryMetadata) -> bool {
            match meta.min_seq {
                Some(min) => after_seq >= min.saturating_sub(1),
                None => after_seq == 0,
            }
        }

        fn invalidate_all_event(id: u64, payload: serde_json::Value) -> Event {
            let data = serde_json::to_string(&payload)
                .unwrap_or_else(|_| r#"{"reason":"unknown"}"#.into());
            Event::default()
                .event("invalidate_all")
                .id(id.to_string())
                .data(data)
        }

        let msg_store = Arc::clone(self.events().msg_store());

        // By default, do NOT replay global history. Instead, start at the current watermark so the
        // event stream only carries forward-going invalidations.
        let subscribe_after_seq = resume_after_seq.or_else(|| msg_store.max_seq());

        let (history, receiver, meta) = msg_store.subscribe_sequenced_from(subscribe_after_seq);
        let watermark = meta.max_seq.unwrap_or(0);

        let last_history_seq = history.last().map(|m| m.seq);
        let mut initial_events: Vec<Event> = Vec::new();
        let mut initial_last_seq = subscribe_after_seq.unwrap_or(0);

        if let Some(requested_after_seq) = resume_after_seq {
            let can_resume =
                requested_after_seq <= watermark && can_resume_from(requested_after_seq, meta);
            if !can_resume {
                initial_events.push(invalidate_all_event(
                    watermark,
                    serde_json::json!({
                        "reason": "resume_unavailable",
                        "requested_after_seq": requested_after_seq,
                        "min_seq": meta.min_seq,
                        "watermark": watermark,
                        "evicted": meta.evicted,
                    }),
                ));
                initial_last_seq = watermark;
            } else {
                for msg in history {
                    // Prefer the targeted backend hints when possible. When hints are available,
                    // avoid also sending the (potentially large) json patch for the same seq.
                    if let Some(invalidate) = msg.to_invalidate_sse_event() {
                        initial_events.push(invalidate);
                    } else {
                        initial_events.push(msg.to_sse_event());
                    }
                }
                initial_last_seq = last_history_seq.unwrap_or(requested_after_seq);
            }
        } else {
            for msg in history {
                if let Some(invalidate) = msg.to_invalidate_sse_event() {
                    initial_events.push(invalidate);
                } else {
                    initial_events.push(msg.to_sse_event());
                }
            }
            initial_last_seq = last_history_seq.unwrap_or(initial_last_seq);
        }

        struct LiveState {
            receiver: tokio::sync::broadcast::Receiver<logs_store::SequencedLogMsg>,
            msg_store: Arc<MsgStore>,
            last_seq: u64,
            pending: VecDeque<Event>,
        }

        let hist = futures::stream::iter(initial_events.into_iter().map(Ok::<_, std::io::Error>));
        let live = futures::stream::unfold(
            LiveState {
                receiver,
                msg_store,
                last_seq: initial_last_seq,
                pending: VecDeque::new(),
            },
            |mut state| async move {
                if let Some(event) = state.pending.pop_front() {
                    return Some((Ok::<_, std::io::Error>(event), state));
                }

                loop {
                    match state.receiver.recv().await {
                        Ok(msg) => {
                            if msg.seq <= state.last_seq {
                                continue;
                            }
                            state.last_seq = msg.seq;
                            if let Some(invalidate) = msg.to_invalidate_sse_event() {
                                state.pending.push_back(invalidate);
                            } else {
                                state.pending.push_back(msg.to_sse_event());
                            }
                            if let Some(event) = state.pending.pop_front() {
                                return Some((Ok::<_, std::io::Error>(event), state));
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            let watermark = state.msg_store.max_seq().unwrap_or(state.last_seq);
                            state.last_seq = watermark;
                            let event = invalidate_all_event(
                                watermark,
                                serde_json::json!({
                                    "reason": "lagged",
                                    "skipped": skipped,
                                    "watermark": watermark,
                                }),
                            );
                            return Some((Ok::<_, std::io::Error>(event), state));
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            return None;
                        }
                    }
                }
            },
        );

        hist.chain(live).boxed()
    }
}

#[derive(Clone)]
pub struct AppRuntime {
    config: Arc<RwLock<Config>>,
    /// A view of the user config intended for API/UI display (does not resolve `{{secret.*}}` /
    /// `{{env.*}}` templates). Kept in-sync with the last successfully loaded runtime config.
    public_config: Arc<RwLock<Config>>,
    config_status: Arc<RwLock<RuntimeConfigStatus>>,
    config_reload_lock: Arc<Mutex<()>>,
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

#[derive(Clone, Debug)]
pub struct RuntimeConfigStatus {
    pub config_dir: std::path::PathBuf,
    pub config_path: std::path::PathBuf,
    pub secret_env_path: std::path::PathBuf,
    pub loaded_at: SystemTime,
    pub last_error: Option<String>,
    pub dirty: bool,
}

#[async_trait]
impl Deployment for AppRuntime {
    async fn new() -> Result<Self, DeploymentError> {
        let (config, public_config, config_status) = Self::load_runtime_config().await?;
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
            public_config,
            config_status,
            config_reload_lock: Arc::new(Mutex::new(())),
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

        deployment.maybe_spawn_config_auto_reload_watcher();

        Ok(deployment)
    }

    fn config(&self) -> &Arc<RwLock<Config>> {
        &self.config
    }

    fn config_status(&self) -> &Arc<RwLock<RuntimeConfigStatus>> {
        &self.config_status
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
    pub fn public_config(&self) -> &Arc<RwLock<Config>> {
        &self.public_config
    }

    fn maybe_spawn_config_auto_reload_watcher(&self) {
        if background_tasks_disabled() {
            return;
        }

        let config_dir = utils_core::vk_config_dir();
        let config_path = utils_core::vk_config_yaml_path();
        let secret_env_path = utils_core::vk_secret_env_path();
        let projects_path = utils_core::vk_projects_yaml_path();
        let projects_dir = utils_core::vk_projects_dir();
        let shutdown = self.shutdown_token();

        let deployment = self.clone();
        tokio::spawn(async move {
            if let Err(err) = deployment
                .run_config_auto_reload_watcher(
                    config_dir,
                    config_path,
                    secret_env_path,
                    projects_path,
                    projects_dir,
                    shutdown,
                )
                .await
            {
                tracing::warn!(error = %err, "Config auto-reload watcher stopped");
            }
        });
    }

    async fn run_config_auto_reload_watcher(
        &self,
        config_dir: std::path::PathBuf,
        config_path: std::path::PathBuf,
        secret_env_path: std::path::PathBuf,
        projects_path: std::path::PathBuf,
        projects_dir: std::path::PathBuf,
        shutdown: CancellationToken,
    ) -> Result<(), execution::fs_watch::FsWatchError> {
        fn is_relevant_path(
            path: &std::path::Path,
            config_dir: &std::path::Path,
            config_path: &std::path::Path,
            secret_env_path: &std::path::Path,
            projects_path: &std::path::Path,
            projects_dir: &std::path::Path,
        ) -> bool {
            if path == config_path || path == secret_env_path || path == projects_path {
                return true;
            }

            let file_name = path.file_name().and_then(|name| name.to_str());
            let ext = path.extension().and_then(|ext| ext.to_str());

            if matches!(
                file_name,
                Some("config.yaml" | "secret.env" | "projects.yaml")
            ) {
                return true;
            }

            if file_name == Some("projects.d") {
                return true;
            }

            let parent_name = path
                .parent()
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str());
            if parent_name == Some("projects.d") {
                return matches!(ext, Some("yaml" | "yml"));
            }

            // Fall back to strict path checks when the file watcher reports unexpected paths
            // (e.g., when config_dir/config_path representations differ due to symlinks).
            if path.parent().is_some_and(|parent| parent == config_dir) {
                return matches!(
                    file_name,
                    Some("config.yaml" | "secret.env" | "projects.yaml")
                );
            }
            if path.parent().is_some_and(|parent| parent == projects_dir) {
                return matches!(ext, Some("yaml" | "yml"));
            }

            false
        }

        let (_watcher, mut rx) = execution::fs_watch::recommended_recursive_watcher(&config_dir)?;

        tracing::info!(
            config_dir = %config_dir.display(),
            "Config auto-reload watcher started"
        );

        let far_future = std::time::Duration::from_secs(60 * 60 * 24 * 365 * 100);
        let mut next_reload_at: Option<tokio::time::Instant> = None;
        let sleep = tokio::time::sleep(far_future);
        tokio::pin!(sleep);

        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    break;
                }
                item = rx.recv() => {
                    let Some(item) = item else {
                        break;
                    };

                    match item {
                        Ok(event) => {
                            if event.is_access {
                                continue;
                            }

                            if !event.paths.iter().any(|path| {
                                is_relevant_path(
                                    path,
                                    &config_dir,
                                    &config_path,
                                    &secret_env_path,
                                    &projects_path,
                                    &projects_dir,
                                )
                            }) {
                                continue;
                            }

                            next_reload_at = Some(tokio::time::Instant::now() + CONFIG_WATCH_DEBOUNCE);
                            sleep.as_mut().reset(next_reload_at.expect("just set"));
                        }
                        Err(err) => {
                            tracing::warn!(error = %err, "Config auto-reload watcher event error");
                        }
                    }
                }
                _ = &mut sleep, if next_reload_at.is_some() => {
                    next_reload_at = None;
                    sleep.as_mut().reset(tokio::time::Instant::now() + far_future);

                    tracing::info!("Config change detected; marking dirty");
                    let mut status = self.config_status.write().await;
                    status.dirty = true;
                }
            }
        }

        tracing::info!("Config auto-reload watcher stopped");
        Ok(())
    }

    async fn load_runtime_config() -> Result<
        (
            Arc<RwLock<Config>>,
            Arc<RwLock<Config>>,
            Arc<RwLock<RuntimeConfigStatus>>,
        ),
        DeploymentError,
    > {
        let config_path = utils_core::vk_config_yaml_path();
        let config_dir = config_path
            .parent()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(utils_core::vk_config_dir);
        let secret_env_path = config_dir.join("secret.env");

        let loaded_at = SystemTime::now();

        let (mut raw_config, mut public_config, last_error) =
            match config::try_load_config_pair_from_file(&config_path) {
                Ok(config::ConfigPair { runtime, public }) => (runtime, public, None),
                Err(err) => {
                    tracing::warn!("Failed to load config from disk, using defaults: {}", err);
                    (Config::default(), Config::default(), Some(err.to_string()))
                }
            };

        let profiles = ExecutorConfigs::from_defaults_merged_with_overrides(
            raw_config.executor_profiles.as_ref(),
        )
        .unwrap_or_else(|err| {
            tracing::warn!("Failed to apply executor profile overrides: {}", err);
            ExecutorConfigs::from_defaults()
        });
        ExecutorConfigs::set_cached(profiles.clone());
        executors_core::agent_command::agent_command_resolver().warm_cache();
        if !raw_config.onboarding_acknowledged
            && let Ok(recommended_executor) = profiles.get_recommended_executor_profile().await
        {
            raw_config.executor_profile = recommended_executor.clone();
            public_config.executor_profile = recommended_executor;
        }

        Self::update_app_version_state(&mut raw_config, utils_core::version::APP_VERSION);
        Self::update_app_version_state(&mut public_config, utils_core::version::APP_VERSION);

        let status = RuntimeConfigStatus {
            config_dir,
            config_path,
            secret_env_path,
            loaded_at,
            last_error,
            dirty: false,
        };

        Ok((
            Arc::new(RwLock::new(raw_config)),
            Arc::new(RwLock::new(public_config)),
            Arc::new(RwLock::new(status)),
        ))
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

    pub async fn reload_user_config(&self) -> Result<(), ConfigError> {
        let _guard = self.config_reload_lock.lock().await;
        let config_path = utils_core::vk_config_yaml_path();

        match config::try_load_config_pair_from_file(&config_path) {
            Ok(config::ConfigPair {
                runtime: mut new_config,
                public: mut new_public_config,
            }) => {
                let profiles = ExecutorConfigs::from_defaults_merged_with_overrides(
                    new_config.executor_profiles.as_ref(),
                )
                .map_err(|err| ConfigError::ValidationError(err.to_string()))?;
                executors_core::agent_command::agent_command_resolver().warm_cache();
                if !new_config.onboarding_acknowledged
                    && let Ok(recommended_executor) =
                        profiles.get_recommended_executor_profile().await
                {
                    new_config.executor_profile = recommended_executor.clone();
                    new_public_config.executor_profile = recommended_executor;
                }
                Self::update_app_version_state(&mut new_config, utils_core::version::APP_VERSION);
                Self::update_app_version_state(
                    &mut new_public_config,
                    utils_core::version::APP_VERSION,
                );

                let loaded_at = SystemTime::now();

                // Commit all config-derived state under a single "snapshot" boundary to avoid
                // readers observing mixed generations across config/public_config/status.
                let mut config = self.config.write().await;
                let mut public_config = self.public_config.write().await;
                let mut status = self.config_status.write().await;

                *config = new_config;
                *public_config = new_public_config;
                ExecutorConfigs::set_cached(profiles);
                status.loaded_at = loaded_at;
                status.last_error = None;
                status.dirty = false;
                Ok(())
            }
            Err(err) => {
                let mut status = self.config_status.write().await;
                status.last_error = Some(err.to_string());
                Err(err)
            }
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
            cache = "file_search_head_check",
            ttl_secs = budgets.file_search_head_check_ttl.as_secs(),
            "Cache budget"
        );
        tracing::info!(
            cache = "file_search_truncated_rebuild",
            min_interval_secs = budgets.file_search_truncated_rebuild_min_interval.as_secs(),
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
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::response::{IntoResponse, Sse};
    use config::Config;
    use futures::StreamExt;
    use json_patch::Patch;
    use serde_json::Value;
    use test_support::{TempRoot, TestDb, TestEnvGuard};
    use tokio_util::sync::CancellationToken;

    use super::{AppRuntime, Deployment};

    fn parse_sse_chunk(chunk: &str) -> (Option<&str>, Option<&str>, Option<String>) {
        let mut event = None;
        let mut id = None;
        let mut data_lines: Vec<&str> = Vec::new();

        for line in chunk.lines() {
            if let Some(value) = line.strip_prefix("event: ") {
                event = Some(value);
                continue;
            }
            if let Some(value) = line.strip_prefix("id: ") {
                id = Some(value);
                continue;
            }
            if let Some(value) = line.strip_prefix("data: ") {
                data_lines.push(value);
                continue;
            }
        }

        let data = if data_lines.is_empty() {
            None
        } else {
            Some(data_lines.join("\n"))
        };

        (event, id, data)
    }

    async fn next_sse_event_text(
        stream: futures::stream::BoxStream<
            'static,
            Result<axum::response::sse::Event, std::io::Error>,
        >,
    ) -> (String, axum::body::BodyDataStream) {
        let response = Sse::new(stream).into_response();
        let mut body_stream = response.into_body().into_data_stream();

        let bytes = tokio::time::timeout(Duration::from_secs(1), body_stream.next())
            .await
            .expect("expected SSE event within timeout")
            .expect("expected SSE body chunk")
            .expect("expected SSE body chunk ok");

        let text = std::str::from_utf8(&bytes).expect("valid utf8 SSE chunk");
        (text.to_string(), body_stream)
    }

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

    #[tokio::test]
    async fn reload_user_config_is_serialized_by_reload_lock() {
        let temp_root = TempRoot::new("vk-test-");
        let db = TestDb::sqlite_file(&temp_root);
        let _env_guard = TestEnvGuard::new(temp_root.path(), db.url().to_string());

        let deployment = <AppRuntime as Deployment>::new().await.unwrap();

        let lock_guard = deployment.config_reload_lock.lock().await;

        let deployment_clone = deployment.clone();
        let handle = tokio::spawn(async move { deployment_clone.reload_user_config().await });

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!handle.is_finished());

        drop(lock_guard);

        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .expect("reload should finish after lock release")
            .expect("join should succeed")
            .expect("reload should succeed");
    }

    #[tokio::test]
    async fn config_watcher_marks_dirty_until_manual_reload() {
        let temp_root = TempRoot::new("vk-test-");
        let db = TestDb::sqlite_file(&temp_root);
        let _env_guard = TestEnvGuard::new(temp_root.path(), db.url().to_string());

        let config_path = utils_core::vk_config_yaml_path();
        std::fs::write(&config_path, "git_branch_prefix: old\n").unwrap();

        let deployment = <AppRuntime as Deployment>::new().await.unwrap();
        assert_eq!(deployment.config.read().await.git_branch_prefix, "old");
        assert!(!deployment.config_status.read().await.dirty);

        let shutdown = CancellationToken::new();
        let watcher_deployment = deployment.clone();
        let watcher_shutdown = shutdown.clone();
        let watcher_handle = tokio::spawn(async move {
            watcher_deployment
                .run_config_auto_reload_watcher(
                    utils_core::vk_config_dir(),
                    utils_core::vk_config_yaml_path(),
                    utils_core::vk_secret_env_path(),
                    utils_core::vk_projects_yaml_path(),
                    utils_core::vk_projects_dir(),
                    watcher_shutdown,
                )
                .await
        });

        // Give the watcher time to subscribe before mutating files, otherwise we can miss the
        // initial filesystem event on slower machines/CI.
        tokio::time::sleep(super::CONFIG_WATCH_DEBOUNCE + Duration::from_millis(50)).await;

        std::fs::write(&config_path, "git_branch_prefix: new\n").unwrap();

        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if deployment.config_status.read().await.dirty {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("dirty should be observed");

        // Watcher never applies changes automatically.
        assert_eq!(deployment.config.read().await.git_branch_prefix, "old");

        deployment
            .reload_user_config()
            .await
            .expect("reload config");

        // Give the watcher time to flush any pending events before asserting the final state.
        tokio::time::sleep(Duration::from_millis(300)).await;

        assert!(!deployment.config_status.read().await.dirty);
        assert_eq!(deployment.config.read().await.git_branch_prefix, "new");

        shutdown.cancel();
        tokio::time::timeout(Duration::from_secs(5), watcher_handle)
            .await
            .expect("watcher should exit after shutdown")
            .expect("watcher join should succeed")
            .expect("watcher should return ok");
    }

    #[tokio::test]
    async fn stream_events_history_emits_only_invalidate_for_patch_with_hints() {
        let temp_root = TempRoot::new("vk-test-");
        let db = TestDb::sqlite_file(&temp_root);
        let _env_guard = TestEnvGuard::new(temp_root.path(), db.url().to_string());

        let deployment = <AppRuntime as Deployment>::new().await.unwrap();
        let patch: Patch = serde_json::from_value(serde_json::json!([
            { "op": "replace", "path": "/tasks/task-1", "value": { "id": "task-1" } }
        ]))
        .expect("valid json patch");
        deployment.events().msg_store().push_patch(patch);

        let stream = deployment.stream_events(Some(0)).await;
        let (chunk, mut body_stream) = next_sse_event_text(stream).await;
        let (event, id, data) = parse_sse_chunk(&chunk);

        assert_eq!(event, Some("invalidate"));
        assert_eq!(id, Some("1"));
        let data = data.expect("expected invalidate payload");
        let value: Value = serde_json::from_str(&data).expect("valid invalidate payload json");
        assert_eq!(value["taskIds"], serde_json::json!(["task-1"]));

        let second = tokio::time::timeout(Duration::from_millis(100), body_stream.next()).await;
        assert!(
            second.is_err(),
            "expected no second SSE event for the same seq"
        );
    }

    #[tokio::test]
    async fn stream_events_history_falls_back_to_json_patch_when_hints_unavailable() {
        let temp_root = TempRoot::new("vk-test-");
        let db = TestDb::sqlite_file(&temp_root);
        let _env_guard = TestEnvGuard::new(temp_root.path(), db.url().to_string());

        let deployment = <AppRuntime as Deployment>::new().await.unwrap();
        let patch: Patch = serde_json::from_value(serde_json::json!([
            { "op": "replace", "path": "/unrelated", "value": { "hello": "world" } }
        ]))
        .expect("valid json patch");
        deployment.events().msg_store().push_patch(patch);

        let stream = deployment.stream_events(Some(0)).await;
        let (chunk, mut body_stream) = next_sse_event_text(stream).await;
        let (event, id, data) = parse_sse_chunk(&chunk);

        assert_eq!(event, Some("json_patch"));
        assert_eq!(id, Some("1"));
        let data = data.expect("expected json patch payload");
        let value: Value = serde_json::from_str(&data).expect("valid json patch payload json");
        assert_eq!(value[0]["path"], "/unrelated");

        let second = tokio::time::timeout(Duration::from_millis(100), body_stream.next()).await;
        assert!(
            second.is_err(),
            "expected no second SSE event for the same seq"
        );
    }

    #[tokio::test]
    async fn stream_events_live_emits_only_invalidate_for_patch_with_hints() {
        let temp_root = TempRoot::new("vk-test-");
        let db = TestDb::sqlite_file(&temp_root);
        let _env_guard = TestEnvGuard::new(temp_root.path(), db.url().to_string());

        let deployment = <AppRuntime as Deployment>::new().await.unwrap();
        let stream = deployment.stream_events(None).await;

        let patch: Patch = serde_json::from_value(serde_json::json!([
            { "op": "replace", "path": "/workspaces/workspace-1", "value": { "task_id": "task-1" } }
        ]))
        .expect("valid json patch");
        deployment.events().msg_store().push_patch(patch);

        let (chunk, mut body_stream) = next_sse_event_text(stream).await;
        let (event, id, data) = parse_sse_chunk(&chunk);

        assert_eq!(event, Some("invalidate"));
        assert_eq!(id, Some("1"));
        let data = data.expect("expected invalidate payload");
        let value: Value = serde_json::from_str(&data).expect("valid invalidate payload json");
        assert_eq!(value["workspaceIds"], serde_json::json!(["workspace-1"]));
        assert_eq!(value["taskIds"], serde_json::json!(["task-1"]));

        let second = tokio::time::timeout(Duration::from_millis(100), body_stream.next()).await;
        assert!(
            second.is_err(),
            "expected no second SSE event for the same seq"
        );
    }

    #[tokio::test]
    async fn stream_events_live_emits_only_invalidate_for_execution_process_patch() {
        let temp_root = TempRoot::new("vk-test-");
        let db = TestDb::sqlite_file(&temp_root);
        let _env_guard = TestEnvGuard::new(temp_root.path(), db.url().to_string());

        let deployment = <AppRuntime as Deployment>::new().await.unwrap();
        let stream = deployment.stream_events(None).await;

        let patch: Patch = serde_json::from_value(serde_json::json!([
            { "op": "add", "path": "/execution_processes/process-1", "value": { "id": "process-1" } }
        ]))
        .expect("valid json patch");
        deployment.events().msg_store().push_patch(patch);

        let (chunk, mut body_stream) = next_sse_event_text(stream).await;
        let (event, id, data) = parse_sse_chunk(&chunk);

        assert_eq!(event, Some("invalidate"));
        assert_eq!(id, Some("1"));
        let data = data.expect("expected invalidate payload");
        let value: Value = serde_json::from_str(&data).expect("valid invalidate payload json");
        assert_eq!(value["taskIds"], serde_json::json!([]));
        assert_eq!(value["workspaceIds"], serde_json::json!([]));
        assert_eq!(value["hasExecutionProcess"], serde_json::json!(true));

        let second = tokio::time::timeout(Duration::from_millis(100), body_stream.next()).await;
        assert!(
            second.is_err(),
            "expected no second SSE event for the same seq"
        );
    }

    #[tokio::test]
    async fn stream_events_resume_unavailable_emits_invalidate_all_with_watermark_id() {
        let temp_root = TempRoot::new("vk-test-");
        let db = TestDb::sqlite_file(&temp_root);
        let _env_guard = TestEnvGuard::new(temp_root.path(), db.url().to_string());

        let deployment = <AppRuntime as Deployment>::new().await.unwrap();
        let stream = deployment.stream_events(Some(1)).await;

        let (chunk, mut body_stream) = next_sse_event_text(stream).await;
        let (event, id, data) = parse_sse_chunk(&chunk);

        assert_eq!(event, Some("invalidate_all"));
        assert_eq!(id, Some("0"));
        let data = data.expect("expected invalidate_all payload");
        let value: Value = serde_json::from_str(&data).expect("valid invalidate_all payload json");
        assert_eq!(value["reason"], "resume_unavailable");
        assert_eq!(value["requested_after_seq"], 1);
        assert_eq!(value["watermark"], 0);

        let second = tokio::time::timeout(Duration::from_millis(100), body_stream.next()).await;
        assert!(
            second.is_err(),
            "expected no extra SSE events without new messages"
        );
    }

    #[tokio::test]
    async fn stream_events_lagged_emits_invalidate_all_with_watermark_id() {
        let temp_root = TempRoot::new("vk-test-");
        let db = TestDb::sqlite_file(&temp_root);
        let _env_guard = TestEnvGuard::new(temp_root.path(), db.url().to_string());

        let deployment = <AppRuntime as Deployment>::new().await.unwrap();
        let stream = deployment.stream_events(None).await;

        // Push a burst without polling the stream to induce a broadcast lag.
        for _ in 0..6000 {
            deployment.events().msg_store().push_stdout("x");
        }

        let (chunk, _body_stream) = next_sse_event_text(stream).await;
        let (event, id, data) = parse_sse_chunk(&chunk);

        assert_eq!(event, Some("invalidate_all"));
        let id: u64 = id
            .expect("expected invalidate_all id")
            .parse()
            .expect("expected numeric id");
        let data = data.expect("expected invalidate_all payload");
        let value: Value = serde_json::from_str(&data).expect("valid invalidate_all payload json");
        assert_eq!(value["reason"], "lagged");
        assert_eq!(value["watermark"].as_u64(), Some(id));
        assert!(value.get("skipped").is_some(), "expected skipped field");
    }
}
