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
        project::{CreateProject, Project, UpdateProject},
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
                    // Prefer the targeted backend hints when possible. The frontend can then skip
                    // (potentially expensive) patch-based invalidation for the same seq.
                    if let Some(invalidate) = msg.to_invalidate_sse_event() {
                        initial_events.push(invalidate);
                    }
                    initial_events.push(msg.to_sse_event());
                }
                initial_last_seq = last_history_seq.unwrap_or(requested_after_seq);
            }
        } else {
            for msg in history {
                if let Some(invalidate) = msg.to_invalidate_sse_event() {
                    initial_events.push(invalidate);
                }
                initial_events.push(msg.to_sse_event());
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
                            }
                            state.pending.push_back(msg.to_sse_event());
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

        deployment.sync_config_projects_to_db().await?;
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
    ) -> Result<(), notify::Error> {
        use notify::{RecursiveMode, Watcher};

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

            if path.parent().is_some_and(|parent| parent == config_dir) {
                return matches!(file_name, Some("config.yaml" | "secret.env" | "projects.yaml"));
            }

            if path.parent().is_some_and(|parent| parent == projects_dir) {
                return matches!(ext, Some("yaml" | "yml"));
            }

            false
        }

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<notify::Result<notify::Event>>();
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })?;

        // Watch the directory instead of individual files to handle atomic writes via rename.
        // Use recursive mode so newly-created `projects.d/` is picked up without a restart.
        watcher.watch(&config_dir, RecursiveMode::Recursive)?;

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
                            if event.kind.is_access() {
                                continue;
                            }

                            if !event.paths.iter().any(|path| is_relevant_path(path, &config_dir, &config_path, &secret_env_path, &projects_path, &projects_dir)) {
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

                    tracing::info!("Config change detected; reloading");
                    match self.reload_user_config().await {
                        Ok(()) => {
                            if let Err(err) = self.sync_config_projects_to_db().await {
                                tracing::warn!(error = %err, "Config reloaded but failed to sync projects to DB");
                            }
                            tracing::info!("Config auto-reload succeeded");
                        }
                        Err(err) => {
                            tracing::warn!(error = %err, "Config auto-reload failed");
                        }
                    }
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
        let schema_path = config_dir.join("config.schema.json");
        let projects_schema_path = config_dir.join("projects.schema.json");

        if let Err(err) = config::write_config_schema_json(&schema_path) {
            tracing::warn!(
                "Failed to write config schema '{}': {}",
                schema_path.display(),
                err
            );
        }
        if let Err(err) = config::write_projects_schema_json(&projects_schema_path) {
            tracing::warn!(
                "Failed to write projects schema '{}': {}",
                projects_schema_path.display(),
                err
            );
        }

        let loaded_at = SystemTime::now();

        let (mut raw_config, last_error) = match config::try_load_config_from_file(&config_path) {
            Ok(config) => (config, None),
            Err(err) => {
                tracing::warn!("Failed to load config from disk, using defaults: {}", err);
                (Config::default(), Some(err.to_string()))
            }
        };

        // Public config is used for API/UI display (no template expansion). If the runtime config
        // failed to load, keep the public view aligned with the last-known-good runtime (defaults
        // on cold start) to avoid showing a config that isn't actually applied.
        let mut public_config = if last_error.is_some() {
            Config::default()
        } else {
            config::try_load_public_config_from_file(&config_path).unwrap_or_else(|err| {
                tracing::warn!(
                    "Failed to load public config from disk, using defaults: {}",
                    err
                );
                Config::default()
            })
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

        match config::try_load_config_from_file(&config_path) {
            Ok(mut new_config) => {
                // Keep a public (non-templated) view in sync with the runtime config so API/UI
                // responses never need to re-read from disk (and won't leak expanded secrets).
                let mut new_public_config = config::try_load_public_config_from_file(&config_path)?;

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
                Ok(())
            }
            Err(err) => {
                let mut status = self.config_status.write().await;
                status.last_error = Some(err.to_string());
                Err(err)
            }
        }
    }

    pub async fn sync_config_projects_to_db(&self) -> Result<(), DbErr> {
        let config = self.config.read().await;

        for project in &config.projects {
            let Some(project_id) = project.id else {
                continue;
            };

            let existing = Project::find_by_id(&self.db.pool, project_id).await?;
            if let Some(existing) = existing {
                if existing.name != project.name {
                    Project::update(
                        &self.db.pool,
                        project_id,
                        &UpdateProject {
                            name: Some(project.name.clone()),
                            dev_script: None,
                            dev_script_working_dir: None,
                            default_agent_working_dir: None,
                            git_no_verify_override: None,
                            scheduler_max_concurrent: None,
                            scheduler_max_retries: None,
                            default_continuation_turns: None,
                            after_prepare_hook: None,
                            before_cleanup_hook: None,
                        },
                    )
                    .await?;
                }
                continue;
            }

            Project::create(
                &self.db.pool,
                &CreateProject {
                    name: project.name.clone(),
                    repositories: Vec::new(),
                },
                project_id,
            )
            .await?;
        }

        Ok(())
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
    use std::{
        path::Path,
        sync::{Mutex, MutexGuard, OnceLock},
        time::Duration,
    };

    use super::Deployment;
    use config::Config;
    use uuid::Uuid;

    use super::AppRuntime;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvGuard {
        _lock: MutexGuard<'static, ()>,
        prev_database_url: Option<String>,
        prev_asset_dir: Option<String>,
        prev_disable_background_tasks: Option<String>,
        prev_vk_config_dir: Option<String>,
    }

    impl EnvGuard {
        fn new(temp_root: &Path, db_url: String) -> Self {
            let lock = env_lock().lock().unwrap_or_else(|err| err.into_inner());
            let prev_database_url = std::env::var("DATABASE_URL").ok();
            let prev_asset_dir = std::env::var("VIBE_ASSET_DIR").ok();
            let prev_disable_background_tasks = std::env::var("VIBE_DISABLE_BACKGROUND_TASKS").ok();
            let prev_vk_config_dir = std::env::var("VK_CONFIG_DIR").ok();

            let vk_config_dir = temp_root.join("vk-config");
            std::fs::create_dir_all(&vk_config_dir).unwrap();

            // SAFETY: tests using EnvGuard are serialized by env_lock.
            unsafe {
                std::env::set_var("VIBE_ASSET_DIR", temp_root);
                std::env::set_var("DATABASE_URL", db_url);
                std::env::set_var("VIBE_DISABLE_BACKGROUND_TASKS", "1");
                std::env::set_var("VK_CONFIG_DIR", &vk_config_dir);
            }

            Self {
                _lock: lock,
                prev_database_url,
                prev_asset_dir,
                prev_disable_background_tasks,
                prev_vk_config_dir,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: tests using EnvGuard are serialized by env_lock.
            unsafe {
                match &self.prev_database_url {
                    Some(value) => std::env::set_var("DATABASE_URL", value),
                    None => std::env::remove_var("DATABASE_URL"),
                }
                match &self.prev_asset_dir {
                    Some(value) => std::env::set_var("VIBE_ASSET_DIR", value),
                    None => std::env::remove_var("VIBE_ASSET_DIR"),
                }
                match &self.prev_disable_background_tasks {
                    Some(value) => std::env::set_var("VIBE_DISABLE_BACKGROUND_TASKS", value),
                    None => std::env::remove_var("VIBE_DISABLE_BACKGROUND_TASKS"),
                }
                match &self.prev_vk_config_dir {
                    Some(value) => std::env::set_var("VK_CONFIG_DIR", value),
                    None => std::env::remove_var("VK_CONFIG_DIR"),
                }
            }
        }
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
        let temp_root = std::env::temp_dir().join(format!("vk-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let _env_guard = EnvGuard::new(&temp_root, db_url);

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
}
