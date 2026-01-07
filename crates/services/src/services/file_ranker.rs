use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use db::models::project::{SearchMatchType, SearchResult};
use once_cell::sync::Lazy;
use tokio::task;

use super::{
    cache_budget::{cache_budgets, should_warn, warn_threshold},
    git::{GitService, GitServiceError},
};

/// Statistics for a single file based on git history
#[derive(Clone, Debug)]
pub struct FileStat {
    /// Index in the commit history (0 = HEAD, 1 = parent of HEAD, ...)
    pub last_index: usize,
    /// Number of times this file was changed in recent commits
    pub commit_count: u32,
    /// Timestamp of the most recent change
    pub last_time: DateTime<Utc>,
}

/// File statistics for a repository
pub type FileStats = HashMap<String, FileStat>;

/// Cache entry for repository history
#[derive(Clone)]
struct RepoHistoryCache {
    head_sha: String,
    stats: Arc<FileStats>,
    cached_at: Instant,
}

/// Global cache for file ranking statistics
static FILE_STATS_CACHE: Lazy<DashMap<PathBuf, RepoHistoryCache>> = Lazy::new(DashMap::new);

/// Configuration constants for ranking algorithm
const DEFAULT_COMMIT_LIMIT: usize = 100;
const BASE_MATCH_SCORE_FILENAME: i64 = 100;
const BASE_MATCH_SCORE_DIRNAME: i64 = 10;
const BASE_MATCH_SCORE_FULLPATH: i64 = 1;
const RECENCY_WEIGHT: i64 = 2;
const FREQUENCY_WEIGHT: i64 = 1;

/// Service for ranking files based on git history
#[derive(Clone)]
pub struct FileRanker {
    git_service: GitService,
}

impl Default for FileRanker {
    fn default() -> Self {
        Self::new()
    }
}

impl FileRanker {
    pub fn new() -> Self {
        Self {
            git_service: GitService::new(),
        }
    }

    /// Get file statistics for a repository, using cache when possible
    pub async fn get_stats(&self, repo_path: &Path) -> Result<Arc<FileStats>, GitServiceError> {
        let repo_path = repo_path.to_path_buf();
        let budgets = cache_budgets();

        // Check if we have a valid cache entry
        if let Some(cache_entry) = FILE_STATS_CACHE.get(&repo_path) {
            let expired =
                is_cache_entry_expired(cache_entry.cached_at, budgets.file_stats_cache_ttl);
            if !expired {
                // Verify cache is still valid by checking HEAD
                if let Ok(head_info) = self.git_service.get_head_info(&repo_path)
                    && head_info.oid == cache_entry.head_sha
                {
                    return Ok(Arc::clone(&cache_entry.stats));
                }
            } else {
                drop(cache_entry);
                FILE_STATS_CACHE.remove(&repo_path);
            }
        }

        // Cache miss or invalid - compute new stats
        let stats = self.compute_stats(&repo_path).await?;
        Ok(stats)
    }

    /// Re-rank search results based on git history statistics
    pub fn rerank(&self, results: &mut [SearchResult], stats: &FileStats) {
        results.sort_by(|a, b| {
            let score_a = self.calculate_score(a, stats);
            let score_b = self.calculate_score(b, stats);
            score_b.cmp(&score_a) // Higher scores first
        });
    }

    /// Calculate relevance score for a search result
    fn calculate_score(&self, result: &SearchResult, stats: &FileStats) -> i64 {
        let base_score = match result.match_type {
            SearchMatchType::FileName => BASE_MATCH_SCORE_FILENAME,
            SearchMatchType::DirectoryName => BASE_MATCH_SCORE_DIRNAME,
            SearchMatchType::FullPath => BASE_MATCH_SCORE_FULLPATH,
        };

        if let Some(stat) = stats.get(&result.path) {
            let recency_bonus = (100 - stat.last_index.min(99) as i64) * RECENCY_WEIGHT;
            let frequency_bonus = stat.commit_count as i64 * FREQUENCY_WEIGHT;

            // Multiply base score to maintain hierarchy, add git-based bonuses
            base_score * 1000 + recency_bonus * 10 + frequency_bonus
        } else {
            // Files not in git history get base score only
            base_score * 1000
        }
    }

    /// Compute file statistics from git history
    async fn compute_stats(&self, repo_path: &Path) -> Result<Arc<FileStats>, GitServiceError> {
        let repo_path = repo_path.to_path_buf();
        let repo_path_for_error = repo_path.clone();
        let git_service = self.git_service.clone();

        // Run git analysis in blocking task to avoid blocking async runtime
        let stats = task::spawn_blocking(move || {
            git_service.collect_recent_file_stats(&repo_path, DEFAULT_COMMIT_LIMIT)
        })
        .await
        .map_err(|e| GitServiceError::InvalidRepository(format!("Task join error: {e}")))?;

        let stats = match stats {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    "Failed to collect file stats for {:?}: {}",
                    repo_path_for_error,
                    e
                );
                // Return empty stats on error - search will still work without ranking
                HashMap::new()
            }
        };

        let stats_arc = Arc::new(stats);

        // Update cache
        if let Ok(head_info) = self.git_service.get_head_info(&repo_path_for_error) {
            FILE_STATS_CACHE.insert(
                repo_path_for_error,
                RepoHistoryCache {
                    head_sha: head_info.oid,
                    stats: Arc::clone(&stats_arc),
                    cached_at: Instant::now(),
                },
            );
        }

        prune_cache();

        Ok(stats_arc)
    }
}

pub fn file_stats_cache_len() -> usize {
    FILE_STATS_CACHE.len()
}

fn is_cache_entry_expired(cached_at: Instant, ttl: Duration) -> bool {
    !ttl.is_zero() && cached_at.elapsed() > ttl
}

fn prune_cache() {
    let budgets = cache_budgets();
    let ttl = budgets.file_stats_cache_ttl;
    let max = budgets.file_stats_cache_max_repos;

    let mut expired = Vec::new();
    if !ttl.is_zero() {
        for entry in FILE_STATS_CACHE.iter() {
            if is_cache_entry_expired(entry.value().cached_at, ttl) {
                expired.push(entry.key().clone());
            }
        }
    }

    for key in &expired {
        FILE_STATS_CACHE.remove(key);
    }

    if !expired.is_empty() && should_warn("file_stats_cache") {
        tracing::warn!(
            "Removed {} expired file stats cache entries (ttl={}s)",
            expired.len(),
            ttl.as_secs()
        );
    }

    let len = FILE_STATS_CACHE.len();
    if max > 0 && len > max {
        let mut entries: Vec<(PathBuf, Instant)> = FILE_STATS_CACHE
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().cached_at))
            .collect();
        entries.sort_by_key(|(_, cached_at)| *cached_at);

        let to_remove = len - max;
        for (key, _) in entries.into_iter().take(to_remove) {
            FILE_STATS_CACHE.remove(&key);
        }

        if should_warn("file_stats_cache") {
            tracing::warn!("Evicted {to_remove} file stats entries to enforce budget {max}");
        }
    }

    let len = FILE_STATS_CACHE.len();
    if max > 0 {
        let threshold = warn_threshold(max);
        if len >= threshold && should_warn("file_stats_cache") {
            tracing::warn!(
                "File stats cache nearing budget: {len}/{max} entries (warn at {threshold})"
            );
        }
    }
}
