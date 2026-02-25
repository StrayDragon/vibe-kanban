use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use db::DBService;
use deployment::{Deployment, DeploymentError};
use executors::profile::ExecutorConfigs;
use services::services::{
    approvals::Approvals,
    cache_budget::cache_budgets,
    config::{Config, load_config_from_file, save_config_to_file},
    container::{ContainerService, log_backfill_completion_cache_len},
    events::EventService,
    file_ranker::file_stats_cache_len,
    file_search_cache::FileSearchCache,
    filesystem::FilesystemService,
    git::GitService,
    image::ImageService,
    project::ProjectService,
    queued_message::QueuedMessageService,
    repo::RepoService,
};
use tokio::sync::RwLock;
use utils::{assets::config_path, msg_store::MsgStore};
use uuid::Uuid;

use crate::container::LocalContainerService;
mod command;
pub mod container;
mod copy;

#[derive(Clone)]
pub struct LocalDeployment {
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
}

#[async_trait]
impl Deployment for LocalDeployment {
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
}

impl LocalDeployment {
    async fn load_runtime_config() -> Result<Arc<RwLock<Config>>, DeploymentError> {
        let mut raw_config = load_config_from_file(&config_path()).await;

        let profiles = ExecutorConfigs::get_cached();
        executors::agent_command::agent_command_resolver().warm_cache();
        if !raw_config.onboarding_acknowledged
            && let Ok(recommended_executor) = profiles.get_recommended_executor_profile().await
        {
            raw_config.executor_profile = recommended_executor;
        }

        Self::update_release_notes_flags(&mut raw_config, utils::version::APP_VERSION);
        save_config_to_file(&raw_config, &config_path()).await?;

        Ok(Arc::new(RwLock::new(raw_config)))
    }

    fn update_release_notes_flags(config: &mut Config, current_version: &str) {
        let stored_version = config.last_app_version.as_deref();
        if stored_version != Some(current_version) {
            // Show release notes only for upgrades, not first install.
            config.show_release_notes = stored_version.is_some();
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
        let db = DBService::new().await?;
        let image = ImageService::new(db.clone().pool)?;
        Self::spawn_orphaned_image_cleanup(image.clone());

        let events = EventService::new(
            db.clone(),
            Arc::new(MsgStore::new()),
            Arc::new(RwLock::new(0)),
        );

        let container = LocalContainerService::new(
            db.clone(),
            core.msg_stores.clone(),
            config,
            core.git.clone(),
            image.clone(),
            core.approvals.clone(),
            core.queued_message_service.clone(),
        )
        .await;

        Ok(RuntimeServices {
            db,
            image,
            events,
            container,
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
        let approvals_completed = self.approvals.completed_len();
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
            cache = "approvals_completed",
            ttl_secs = budgets.approvals_completed_ttl.as_secs(),
            current_entries = approvals_completed,
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
}

#[cfg(test)]
mod tests {
    use services::services::config::Config;

    use super::LocalDeployment;

    #[test]
    fn update_release_notes_flags_sets_upgrade_state() {
        let mut config = Config {
            last_app_version: Some("0.0.100".to_string()),
            show_release_notes: false,
            ..Config::default()
        };

        LocalDeployment::update_release_notes_flags(&mut config, "0.0.101");

        assert_eq!(config.last_app_version.as_deref(), Some("0.0.101"));
        assert!(config.show_release_notes);
    }

    #[test]
    fn update_release_notes_flags_does_not_flip_on_same_version() {
        let mut config = Config {
            last_app_version: Some("0.0.101".to_string()),
            show_release_notes: true,
            ..Config::default()
        };

        LocalDeployment::update_release_notes_flags(&mut config, "0.0.101");

        assert_eq!(config.last_app_version.as_deref(), Some("0.0.101"));
        assert!(config.show_release_notes);
    }
}
