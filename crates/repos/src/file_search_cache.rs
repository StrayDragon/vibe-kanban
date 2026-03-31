use std::{
    collections::HashSet,
    io::Write as _,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use config::cache_budget::{cache_budgets, should_warn, warn_threshold};
use dashmap::DashMap;
use db::models::{
    project::{Project, SearchMatchType, SearchResult},
    project_repo::ProjectRepo,
};
use ignore::WalkBuilder;
use moka::future::Cache;
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use ts_rs::TS;

use super::{
    file_ranker::{FileRanker, FileStats},
    git::GitService,
};

/// Search mode for different use cases
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum SearchMode {
    #[default]
    TaskForm, // Default: exclude ignored files (clean results)
    Settings, // Include ignored files (for project config like .env)
}

/// Search query parameters for typed Axum extraction
#[derive(Debug, Clone, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(default)]
    pub mode: SearchMode,
}

#[derive(Debug)]
pub struct RepoSearchResponse {
    pub results: Vec<SearchResult>,
    pub index_truncated: bool,
}

/// File index entry used for search results
#[derive(Clone, Debug)]
pub struct IndexedFile {
    pub path: String,
    pub is_file: bool,
    pub path_lowercase: Option<Arc<str>>,
    pub is_ignored: bool, // Track if file is gitignored
}

impl IndexedFile {
    fn path_lowercase(&self) -> &str {
        self.path_lowercase.as_deref().unwrap_or(&self.path)
    }
}

/// File index build result containing indexed files
#[derive(Debug)]
pub struct FileIndex {
    pub files: Vec<IndexedFile>,
    pub index_truncated: bool,
}

/// Errors that can occur during file index building
#[derive(Error, Debug)]
pub enum FileIndexError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Walk(#[from] ignore::Error),
    #[error(transparent)]
    StripPrefix(#[from] std::path::StripPrefixError),
}

/// Cached repository data with indexed files and git stats
pub struct CachedRepo {
    pub head_sha: String,
    pub indexed_files: Vec<IndexedFile>,
    pub stats: Arc<FileStats>,
    pub index_truncated: bool,
    pub build_ts: Instant,
}

struct RepoWatcher {
    // Keep the debouncer alive while the watcher is registered.
    #[allow(dead_code)]
    debouncer: Arc<Mutex<Debouncer<RecommendedWatcher, RecommendedCache>>>,
    created_at: Instant,
}

/// Cache miss error
#[derive(Debug)]
pub enum CacheError {
    Miss,
    BuildError(String),
}

/// File search cache with indexed files
pub struct FileSearchCache {
    cache: Cache<PathBuf, Arc<CachedRepo>>,
    file_ranker: FileRanker,
    build_queue: mpsc::Sender<PathBuf>,
    pending_builds: Arc<DashMap<PathBuf, ()>>,
    head_check_queue: mpsc::Sender<PathBuf>,
    pending_head_checks: Arc<DashMap<PathBuf, ()>>,
    last_head_check: Arc<DashMap<PathBuf, Instant>>,
    watchers: DashMap<PathBuf, RepoWatcher>,
}

impl FileSearchCache {
    pub fn new() -> Self {
        let budgets = cache_budgets();
        let build_queue_capacity = budgets
            .file_search_cache_max_repos
            .saturating_mul(4)
            .clamp(1, 1024);
        let (build_sender, build_receiver) = mpsc::channel(build_queue_capacity);

        let head_check_queue_capacity = budgets
            .file_search_cache_max_repos
            .saturating_mul(2)
            .clamp(1, 1024);
        let (head_check_sender, head_check_receiver) = mpsc::channel(head_check_queue_capacity);
        let mut cache_builder =
            Cache::builder().max_capacity(budgets.file_search_cache_max_repos as u64);
        if !budgets.file_search_cache_ttl.is_zero() {
            cache_builder = cache_builder.time_to_live(budgets.file_search_cache_ttl);
        }
        let cache = cache_builder.build();

        let pending_builds = Arc::new(DashMap::new());
        let pending_head_checks = Arc::new(DashMap::new());
        let last_head_check = Arc::new(DashMap::new());

        let cache_for_worker = cache.clone();
        let git_service = GitService::new();
        let file_ranker = FileRanker::new();

        // Spawn background worker
        let worker_git_service = git_service.clone();
        let worker_file_ranker = file_ranker.clone();
        let worker_pending_builds = Arc::clone(&pending_builds);
        tokio::spawn(async move {
            Self::background_worker(
                build_receiver,
                cache_for_worker,
                worker_git_service,
                worker_file_ranker,
                worker_pending_builds,
            )
            .await;
        });

        let head_worker_cache = cache.clone();
        let head_worker_git_service = git_service.clone();
        let head_worker_build_queue = build_sender.clone();
        let head_worker_pending_builds = Arc::clone(&pending_builds);
        let head_worker_pending_head_checks = Arc::clone(&pending_head_checks);
        let head_worker_last_head_check = Arc::clone(&last_head_check);
        tokio::spawn(async move {
            Self::head_check_worker(
                head_check_receiver,
                head_worker_cache,
                head_worker_git_service,
                head_worker_build_queue,
                head_worker_pending_builds,
                head_worker_pending_head_checks,
                head_worker_last_head_check,
            )
            .await;
        });

        Self {
            cache,
            file_ranker,
            build_queue: build_sender,
            pending_builds,
            head_check_queue: head_check_sender,
            pending_head_checks,
            last_head_check,
            watchers: DashMap::new(),
        }
    }

    pub fn cache_entry_count(&self) -> u64 {
        self.cache.entry_count()
    }

    pub fn watcher_count(&self) -> usize {
        self.watchers.len()
    }

    fn warn_if_cache_near_capacity(current: usize) {
        let budgets = cache_budgets();
        let max = budgets.file_search_cache_max_repos;
        if max == 0 {
            return;
        }

        let threshold = warn_threshold(max);
        if current >= threshold && should_warn("file_search_cache") {
            warn!(
                "File search cache nearing budget: {current}/{max} entries (warn at {threshold})"
            );
        }
    }

    fn warn_if_watchers_near_capacity(current: usize, max: usize) {
        if max == 0 {
            return;
        }

        let threshold = warn_threshold(max);
        if current >= threshold && should_warn("file_search_watchers") {
            warn!(
                "File search watchers nearing budget: {current}/{max} entries (warn at {threshold})"
            );
        }
    }

    fn is_watcher_expired(created_at: Instant) -> bool {
        let ttl = cache_budgets().file_search_watcher_ttl;
        !ttl.is_zero() && created_at.elapsed() > ttl
    }

    fn prune_watchers(&self) {
        let budgets = cache_budgets();
        let max = budgets.file_search_watchers_max;

        let mut expired = Vec::new();
        if !budgets.file_search_watcher_ttl.is_zero() {
            for entry in self.watchers.iter() {
                if Self::is_watcher_expired(entry.value().created_at) {
                    expired.push(entry.key().clone());
                }
            }
        }

        for key in &expired {
            self.watchers.remove(key);
        }

        if !expired.is_empty() && should_warn("file_search_watchers") {
            warn!(
                "Removed {} expired file search watchers (ttl={}s)",
                expired.len(),
                budgets.file_search_watcher_ttl.as_secs()
            );
        }

        let len = self.watchers.len();
        if len > max {
            let mut entries: Vec<(PathBuf, Instant)> = self
                .watchers
                .iter()
                .map(|entry| (entry.key().clone(), entry.value().created_at))
                .collect();
            entries.sort_by_key(|(_, created_at)| *created_at);

            let to_remove = len - max;
            for (path, _) in entries.into_iter().take(to_remove) {
                self.watchers.remove(&path);
            }

            if should_warn("file_search_watchers") {
                warn!("Evicted {to_remove} file search watchers to enforce budget {max}");
            }
        }

        Self::warn_if_watchers_near_capacity(self.watchers.len(), max);
    }

    fn enqueue_build(&self, repo_path: PathBuf) {
        if self.pending_builds.insert(repo_path.clone(), ()).is_some() {
            return;
        }

        if let Err(err) = self.build_queue.try_send(repo_path.clone()) {
            self.pending_builds.remove(&repo_path);
            if should_warn("file_search_cache_build_queue") {
                warn!(
                    repo = ?repo_path,
                    error = %err,
                    "Failed to enqueue repo cache build"
                );
            }
        }
    }

    fn enqueue_head_check(&self, repo_path: PathBuf) {
        let ttl = cache_budgets().file_search_head_check_ttl;
        if ttl.is_zero() {
            return;
        }

        if self.pending_builds.contains_key(&repo_path) {
            return;
        }

        if let Some(last) = self.last_head_check.get(&repo_path)
            && last.elapsed() < ttl
        {
            return;
        }

        if self
            .pending_head_checks
            .insert(repo_path.clone(), ())
            .is_some()
        {
            return;
        }

        if let Err(err) = self.head_check_queue.try_send(repo_path.clone()) {
            self.pending_head_checks.remove(&repo_path);
            if should_warn("file_search_head_check_queue") {
                warn!(
                    repo = ?repo_path,
                    error = %err,
                    "Failed to enqueue repo HEAD check"
                );
            }
        }
    }

    /// Search files in repository using cache
    pub async fn search(
        &self,
        repo_path: &Path,
        query: &str,
        mode: SearchMode,
    ) -> Result<RepoSearchResponse, CacheError> {
        let repo_path_buf = repo_path.to_path_buf();

        // Cache hit - return results immediately, and validate HEAD asynchronously.
        if let Some(cached) = self.cache.get(&repo_path_buf).await {
            self.enqueue_head_check(repo_path_buf.clone());
            return Ok(RepoSearchResponse {
                results: self.search_in_cache(cached.as_ref(), query, mode).await,
                index_truncated: cached.index_truncated,
            });
        }

        // Cache miss - trigger background refresh and return error
        self.enqueue_build(repo_path_buf);

        Err(CacheError::Miss)
    }

    /// Pre-warm cache for given repositories
    pub async fn warm_repos(&self, repo_paths: Vec<PathBuf>) -> Result<(), String> {
        for repo_path in repo_paths {
            self.enqueue_build(repo_path);
        }
        Ok(())
    }

    /// Pre-warm cache for most active projects
    pub async fn warm_most_active(&self, db_pool: &db::DbPool, limit: i32) -> Result<(), String> {
        info!("Starting file search cache warming...");

        // Get most active projects
        let active_projects = Project::find_most_active(db_pool, limit)
            .await
            .map_err(|e| format!("Failed to fetch active projects: {e}"))?;

        if active_projects.is_empty() {
            info!("No active projects found, skipping cache warming");
            return Ok(());
        }

        // Collect all repository paths from active projects
        let mut repo_paths: Vec<PathBuf> = Vec::new();
        for project in &active_projects {
            let repos = ProjectRepo::find_repos_for_project(db_pool, project.id)
                .await
                .map_err(|e| format!("Failed to fetch repositories for project: {e}"))?;
            for repo in repos {
                repo_paths.push(repo.path);
            }
        }

        if repo_paths.is_empty() {
            info!("No repositories found for active projects, skipping cache warming");
            return Ok(());
        }

        info!(
            "Warming cache for {} repositories: {:?}",
            repo_paths.len(),
            repo_paths
        );

        // Warm the cache
        self.warm_repos(repo_paths.clone())
            .await
            .map_err(|e| format!("Failed to warm cache: {e}"))?;

        // NOTE: Temporarily disabled; HEAD-change refresh is too frequent/noisy.
        // Re-enable when refresh is limited to specific scenarios.
        // for repo_path in &repo_paths {
        //     if let Err(e) = self.setup_watcher(repo_path).await {
        //         warn!("Failed to setup watcher for {:?}: {}", repo_path, e);
        //     }
        // }

        info!("File search cache warming completed");
        Ok(())
    }

    /// Search within cached index with mode-based filtering
    async fn search_in_cache(
        &self,
        cached: &CachedRepo,
        query: &str,
        mode: SearchMode,
    ) -> Vec<SearchResult> {
        struct ScoredResult {
            score: i64,
            path: String,
            is_file: bool,
            match_type: SearchMatchType,
        }

        fn last_path_segment(path: &str) -> &str {
            path.rsplit(['/', '\\']).next().unwrap_or(path)
        }

        fn parent_dir_name(path: &str) -> Option<&str> {
            let parent_end = path.rfind(['/', '\\'])?;
            let parent = &path[..parent_end];
            if parent.is_empty() {
                return None;
            }
            Some(last_path_segment(parent))
        }

        let query_lower: std::borrow::Cow<'_, str> = if query.chars().any(|c| c.is_uppercase()) {
            std::borrow::Cow::Owned(query.to_lowercase())
        } else {
            std::borrow::Cow::Borrowed(query)
        };
        let query_lower = query_lower.as_ref();

        const TOP_K: usize = 10;
        let mut top: Vec<ScoredResult> = Vec::new();

        let task_form = matches!(mode, SearchMode::TaskForm);

        for indexed_file in &cached.indexed_files {
            if task_form && indexed_file.is_ignored {
                continue;
            }

            let path_lower = indexed_file.path_lowercase();
            if !path_lower.contains(query_lower) {
                continue;
            }

            let file_name_lower = last_path_segment(path_lower);
            let match_type = if file_name_lower.contains(query_lower) {
                SearchMatchType::FileName
            } else if parent_dir_name(path_lower).is_some_and(|p| p.contains(query_lower)) {
                SearchMatchType::DirectoryName
            } else {
                SearchMatchType::FullPath
            };

            let score =
                self.file_ranker
                    .score(&match_type, indexed_file.path.as_str(), &cached.stats);

            if top.len() < TOP_K {
                top.push(ScoredResult {
                    score,
                    path: indexed_file.path.clone(),
                    is_file: indexed_file.is_file,
                    match_type,
                });
                continue;
            }

            let worst_idx = top
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.score.cmp(&b.score).then_with(|| b.path.cmp(&a.path)))
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            let worst = &top[worst_idx];
            let better = score > worst.score
                || (score == worst.score && indexed_file.path.as_str() < worst.path.as_str());
            if better {
                top[worst_idx] = ScoredResult {
                    score,
                    path: indexed_file.path.clone(),
                    is_file: indexed_file.is_file,
                    match_type,
                };
            }
        }

        top.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.cmp(&b.path)));
        top.into_iter()
            .map(|r| SearchResult {
                path: r.path,
                is_file: r.is_file,
                match_type: r.match_type,
            })
            .collect()
    }

    /// Build cache entry for a repository
    async fn build_repo_cache(
        repo_path: &Path,
        git_service: &GitService,
        file_ranker: &FileRanker,
    ) -> Result<CachedRepo, String> {
        let repo_path_buf = repo_path.to_path_buf();

        info!("Building cache for repo: {:?}", repo_path);

        // Get current HEAD
        let head_oid = git_service
            .get_head_oid_fast(&repo_path_buf)
            .map_err(|e| format!("Failed to get HEAD info: {e}"))?;

        // Get git stats
        let stats = file_ranker
            .get_stats(repo_path)
            .await
            .map_err(|e| format!("Failed to get git stats: {e}"))?;

        // Build file index
        let max_files = cache_budgets().file_search_max_files;
        let repo_path_for_build = repo_path_buf.clone();
        let file_index = tokio::task::spawn_blocking(move || {
            Self::build_file_index(&repo_path_for_build, max_files)
        })
        .await
        .map_err(|e| format!("Failed to build file index: join error: {e}"))?
        .map_err(|e| format!("Failed to build file index: {e}"))?;

        if file_index.index_truncated && should_warn("file_search_index_truncated") {
            warn!(
                "File search index truncated for repo {:?}: indexed {} entries (cap={})",
                repo_path,
                file_index.files.len(),
                max_files
            );
        }

        Ok(CachedRepo {
            head_sha: head_oid,
            indexed_files: file_index.files,
            stats,
            index_truncated: file_index.index_truncated,
            build_ts: Instant::now(),
        })
    }

    /// Build file index from filesystem traversal using superset approach
    fn build_file_index(repo_path: &Path, max_files: usize) -> Result<FileIndex, FileIndexError> {
        #[derive(Debug)]
        struct PreIndexedFile {
            path: String,
            is_file: bool,
            path_lowercase: Option<Arc<str>>,
        }

        fn git_check_ignored_paths(
            repo_path: &Path,
            files: &[PreIndexedFile],
        ) -> Result<HashSet<String>, std::io::Error> {
            if files.is_empty() {
                return Ok(HashSet::new());
            }

            let mut child = Command::new("git")
                .arg("-C")
                .arg(repo_path)
                .arg("check-ignore")
                .arg("-z")
                .arg("--stdin")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;

            if let Some(mut stdin) = child.stdin.take() {
                for file in files {
                    if let Err(err) = stdin.write_all(file.path.as_bytes()) {
                        if matches!(
                            err.kind(),
                            std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::Interrupted
                        ) {
                            let _ = child.wait();
                            return Ok(HashSet::new());
                        }
                        return Err(err);
                    }
                    if let Err(err) = stdin.write_all(b"\0") {
                        if matches!(
                            err.kind(),
                            std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::Interrupted
                        ) {
                            let _ = child.wait();
                            return Ok(HashSet::new());
                        }
                        return Err(err);
                    }
                }
            }

            let out = match child.wait_with_output() {
                Ok(out) => out,
                Err(err)
                    if matches!(
                        err.kind(),
                        std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    return Ok(HashSet::new());
                }
                Err(err) => return Err(err),
            };
            // `git check-ignore` exits with 1 when no paths are ignored.
            let status_code = out.status.code();
            if !out.status.success() && status_code != Some(1) {
                // If the process gets interrupted (SIGINT/SIGTERM), treat as a best-effort skip.
                if matches!(status_code, Some(130) | Some(143) | None) {
                    return Ok(HashSet::new());
                }
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                return Err(std::io::Error::other(format!(
                    "git check-ignore failed: {stderr}"
                )));
            }

            let mut ignored = HashSet::new();
            for part in out.stdout.split(|b| *b == 0) {
                if part.is_empty() {
                    continue;
                }
                ignored.insert(String::from_utf8_lossy(part).to_string());
            }
            Ok(ignored)
        }

        let max_files = max_files.max(1);
        let mut pre_indexed = Vec::new();
        let mut index_truncated = false;

        // Build superset walker - include ignored files but exclude .git and performance killers
        let mut builder = WalkBuilder::new(repo_path);
        builder
            .git_ignore(false) // Include all files initially
            .git_global(false)
            .git_exclude(false)
            .hidden(false) // Show hidden files like .env
            .filter_entry(|entry| {
                let name = entry.file_name().to_string_lossy();
                // Always exclude .git directories
                if name == ".git" {
                    return false;
                }
                // Exclude performance killers even when including ignored files
                if name == "node_modules" || name == "target" || name == "dist" || name == "build" {
                    return false;
                }
                true
            });

        let walker = builder.build();
        for result in walker {
            if pre_indexed.len() >= max_files {
                index_truncated = true;
                break;
            }

            let entry = result?;
            let path = entry.path();

            if path == repo_path {
                continue;
            }

            let relative_path = path.strip_prefix(repo_path)?;
            let relative_path_str = relative_path.to_string_lossy().to_string();
            if relative_path_str.is_empty() {
                continue;
            }

            let needs_lowercase = relative_path_str.chars().any(|c| c.is_uppercase());
            let path_lowercase = if needs_lowercase {
                Some(Arc::from(relative_path_str.to_lowercase()))
            } else {
                None
            };

            let is_file = entry
                .file_type()
                .map(|ft| ft.is_file())
                .unwrap_or_else(|| path.is_file());

            pre_indexed.push(PreIndexedFile {
                path: relative_path_str,
                is_file,
                path_lowercase,
            });
        }

        let ignored_paths = match git_check_ignored_paths(repo_path, &pre_indexed) {
            Ok(paths) => paths,
            Err(err) => {
                warn!(
                    repo = ?repo_path,
                    error = %err,
                    "Failed to detect ignored paths via `git check-ignore`; treating all as non-ignored"
                );
                HashSet::new()
            }
        };

        let indexed_files = pre_indexed
            .into_iter()
            .map(|file| IndexedFile {
                is_ignored: ignored_paths.contains(&file.path),
                path: file.path,
                is_file: file.is_file,
                path_lowercase: file.path_lowercase,
            })
            .collect();

        Ok(FileIndex {
            files: indexed_files,
            index_truncated,
        })
    }

    /// Background worker for cache building
    async fn background_worker(
        mut build_receiver: mpsc::Receiver<PathBuf>,
        cache: Cache<PathBuf, Arc<CachedRepo>>,
        git_service: GitService,
        file_ranker: FileRanker,
        pending_builds: Arc<DashMap<PathBuf, ()>>,
    ) {
        while let Some(repo_path) = build_receiver.recv().await {
            match Self::build_repo_cache(&repo_path, &git_service, &file_ranker).await {
                Ok(cached_repo) => {
                    cache.insert(repo_path.clone(), Arc::new(cached_repo)).await;
                    Self::warn_if_cache_near_capacity(cache.entry_count() as usize);
                    info!("Successfully cached repo: {:?}", repo_path);
                }
                Err(e) => {
                    error!("Failed to cache repo {:?}: {}", repo_path, e);
                }
            }

            pending_builds.remove(&repo_path);
        }
    }

    async fn head_check_worker(
        mut head_receiver: mpsc::Receiver<PathBuf>,
        cache: Cache<PathBuf, Arc<CachedRepo>>,
        git_service: GitService,
        build_queue: mpsc::Sender<PathBuf>,
        pending_builds: Arc<DashMap<PathBuf, ()>>,
        pending_head_checks: Arc<DashMap<PathBuf, ()>>,
        last_head_check: Arc<DashMap<PathBuf, Instant>>,
    ) {
        while let Some(repo_path) = head_receiver.recv().await {
            let now = Instant::now();
            let _pending = pending_head_checks.remove(&repo_path);
            last_head_check.insert(repo_path.clone(), now);

            if pending_builds.contains_key(&repo_path) {
                continue;
            }

            let Some(cached) = cache.get(&repo_path).await else {
                continue;
            };

            let head_oid = match git_service.get_head_oid_fast(&repo_path) {
                Ok(oid) => oid,
                Err(err) => {
                    if should_warn("file_search_head_check_failed") {
                        warn!(repo = ?repo_path, error = %err, "Failed to check repo HEAD");
                    }
                    continue;
                }
            };

            if head_oid == cached.head_sha {
                continue;
            }

            let min_interval = cache_budgets().file_search_truncated_rebuild_min_interval;
            if cached.index_truncated
                && !min_interval.is_zero()
                && cached.build_ts.elapsed() < min_interval
            {
                continue;
            }

            if pending_builds.insert(repo_path.clone(), ()).is_some() {
                continue;
            }

            if let Err(err) = build_queue.try_send(repo_path.clone()) {
                pending_builds.remove(&repo_path);
                if should_warn("file_search_cache_build_queue") {
                    warn!(
                        repo = ?repo_path,
                        error = %err,
                        "Failed to enqueue repo cache rebuild after HEAD change"
                    );
                }
            }
        }
    }

    /// Setup file watcher for repository
    pub async fn setup_watcher(&self, repo_path: &Path) -> Result<(), String> {
        let repo_path_buf = repo_path.to_path_buf();

        self.prune_watchers();
        if self.watchers.contains_key(&repo_path_buf) {
            return Ok(()); // Already watching
        }

        if let Some(cached) = self.cache.get(&repo_path_buf).await
            && cached.index_truncated
        {
            if should_warn("file_search_watcher_skip_truncated") {
                warn!(
                    "Skipping file watcher registration for repo {:?}: file search index truncated",
                    repo_path
                );
            }
            return Ok(());
        }

        let git_dir = repo_path.join(".git");
        if !git_dir.exists() {
            return Err("Not a git repository".to_string());
        }

        let build_queue = self.build_queue.clone();
        let pending_builds = Arc::clone(&self.pending_builds);
        let watched_path = repo_path_buf.clone();

        // Bounded queue to coalesce HEAD-change events.
        let (tx, mut rx) = mpsc::channel(1);

        let mut debouncer = new_debouncer(
            Duration::from_millis(500),
            None,
            move |res: DebounceEventResult| {
                if let Ok(events) = res {
                    for event in events {
                        // Check if any path contains HEAD file
                        for path in &event.event.paths {
                            if path.file_name().is_some_and(|name| name == "HEAD") {
                                let _ = tx.try_send(());
                                break;
                            }
                        }
                    }
                }
            },
        )
        .map_err(|e| format!("Failed to create file watcher: {e}"))?;

        debouncer
            .watch(git_dir.join("HEAD"), RecursiveMode::NonRecursive)
            .map_err(|e| format!("Failed to watch HEAD file: {e}"))?;

        self.watchers.insert(
            repo_path_buf.clone(),
            RepoWatcher {
                debouncer: Arc::new(Mutex::new(debouncer)),
                created_at: Instant::now(),
            },
        );
        self.prune_watchers();

        // Spawn task to handle HEAD changes
        tokio::spawn(async move {
            while rx.recv().await.is_some() {
                info!("HEAD changed for repo: {:?}", watched_path);
                if pending_builds.insert(watched_path.clone(), ()).is_some() {
                    continue;
                }
                if let Err(err) = build_queue.try_send(watched_path.clone()) {
                    pending_builds.remove(&watched_path);
                    error!(repo = ?watched_path, error = %err, "Failed to enqueue cache refresh");
                }
            }
        });

        info!("Setup file watcher for repo: {:?}", repo_path);
        Ok(())
    }
}

impl Default for FileSearchCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, fs, process::Command, time::Instant};

    use tempfile::tempdir;

    use super::*;

    fn git(repo_path: &Path, args: &[&str]) {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn git_commit_all(repo_path: &Path, message: &str) {
        git(repo_path, &["add", "."]);
        git(
            repo_path,
            &[
                "-c",
                "user.email=test@example.com",
                "-c",
                "user.name=vk-test",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                message,
            ],
        );
    }

    async fn wait_for_cached_repo(cache: &FileSearchCache, repo_path: &PathBuf) {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if cache.cache.get(repo_path).await.is_some() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("repo cached in time");
    }

    #[test]
    fn build_file_index_enforces_cap_and_records_truncation() {
        let dir = tempdir().expect("tempdir");
        for idx in 0..5 {
            fs::write(dir.path().join(format!("file-{idx}.txt")), "hello")
                .expect("write test file");
        }

        let index = FileSearchCache::build_file_index(dir.path(), 3).expect("build index");

        assert_eq!(index.files.len(), 3);
        assert!(index.index_truncated);
    }

    #[tokio::test]
    async fn setup_watcher_skips_truncated_repos() {
        let dir = tempdir().expect("tempdir");
        fs::create_dir_all(dir.path().join(".git")).expect("create .git dir");
        fs::write(dir.path().join(".git/HEAD"), "ref: refs/heads/main\n").expect("create HEAD");

        let cache = FileSearchCache::new();

        cache
            .cache
            .insert(
                dir.path().to_path_buf(),
                Arc::new(CachedRepo {
                    head_sha: "test".to_string(),
                    indexed_files: vec![],
                    stats: Arc::new(HashMap::new()),
                    index_truncated: true,
                    build_ts: Instant::now(),
                }),
            )
            .await;

        cache
            .setup_watcher(dir.path())
            .await
            .expect("setup_watcher should succeed");

        assert_eq!(cache.watcher_count(), 0);
    }

    #[tokio::test]
    async fn search_does_not_sync_miss_on_head_change_and_refreshes_in_background() {
        let dir = tempdir().expect("tempdir");
        git(dir.path(), &["init"]);
        fs::write(dir.path().join("a.txt"), "hello").expect("write a.txt");
        git_commit_all(dir.path(), "first");

        let repo_path = dir.path().to_path_buf();

        let cache = FileSearchCache::new();
        cache
            .warm_repos(vec![repo_path.clone()])
            .await
            .expect("warm repos");
        wait_for_cached_repo(&cache, &repo_path).await;

        let cached_before = cache
            .cache
            .get(&repo_path)
            .await
            .expect("cached repo entry");
        let head_before = cached_before.head_sha.clone();

        fs::write(dir.path().join("b.txt"), "world").expect("write b.txt");
        git_commit_all(dir.path(), "second");

        let head_after = GitService::new()
            .get_head_oid(&repo_path)
            .expect("get head oid");
        assert_ne!(head_before, head_after);

        // With the async head-check, this should still be a cache hit even when HEAD changed.
        let resp = cache
            .search(&repo_path, "a", SearchMode::TaskForm)
            .await
            .expect("cache hit search");
        assert!(!resp.results.is_empty());

        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let cached = cache
                    .cache
                    .get(&repo_path)
                    .await
                    .expect("cached repo entry");
                if cached.head_sha == head_after {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("head refresh rebuild completes");
    }

    #[tokio::test]
    async fn head_check_worker_throttles_truncated_repo_rebuilds() {
        let dir = tempdir().expect("tempdir");
        git(dir.path(), &["init"]);
        fs::write(dir.path().join("a.txt"), "hello").expect("write a.txt");
        git_commit_all(dir.path(), "first");

        let repo_path = dir.path().to_path_buf();

        let cache = Cache::builder().max_capacity(10).build();
        cache
            .insert(
                repo_path.clone(),
                Arc::new(CachedRepo {
                    head_sha: "stale".to_string(),
                    indexed_files: vec![],
                    stats: Arc::new(HashMap::new()),
                    index_truncated: true,
                    build_ts: Instant::now(),
                }),
            )
            .await;

        let (head_tx, head_rx) = tokio::sync::mpsc::channel(4);
        let (build_tx, mut build_rx) = tokio::sync::mpsc::channel(4);

        let pending_builds = Arc::new(DashMap::new());
        let pending_head_checks = Arc::new(DashMap::new());
        let last_head_check = Arc::new(DashMap::new());

        let cache_for_worker = cache.clone();
        let worker = tokio::spawn(FileSearchCache::head_check_worker(
            head_rx,
            cache_for_worker,
            GitService::new(),
            build_tx,
            Arc::clone(&pending_builds),
            Arc::clone(&pending_head_checks),
            Arc::clone(&last_head_check),
        ));

        // build_ts is "now" and index is truncated, so rebuild should be throttled by default.
        head_tx
            .send(repo_path.clone())
            .await
            .expect("send head check");
        assert!(
            tokio::time::timeout(Duration::from_millis(200), build_rx.recv())
                .await
                .is_err(),
            "expected no rebuild enqueue within throttle window"
        );

        // Simulate an older build to allow rebuild enqueue.
        cache
            .insert(
                repo_path.clone(),
                Arc::new(CachedRepo {
                    head_sha: "stale".to_string(),
                    indexed_files: vec![],
                    stats: Arc::new(HashMap::new()),
                    index_truncated: true,
                    build_ts: Instant::now() - Duration::from_secs(3600),
                }),
            )
            .await;

        head_tx
            .send(repo_path.clone())
            .await
            .expect("send head check");
        let enqueued = tokio::time::timeout(Duration::from_secs(1), build_rx.recv())
            .await
            .expect("rebuild enqueue arrives")
            .expect("rebuild enqueue has value");
        assert_eq!(enqueued, repo_path);

        drop(head_tx);
        worker.await.expect("head check worker");
    }
}
