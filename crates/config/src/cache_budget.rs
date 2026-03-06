use std::{
    collections::HashMap,
    env,
    sync::Mutex,
    time::{Duration, Instant},
};

use once_cell::sync::Lazy;
use tracing::warn;

const DEFAULT_FILE_SEARCH_CACHE_MAX_REPOS: usize = 25;
const DEFAULT_FILE_SEARCH_CACHE_TTL_SECS: u64 = 3600;
const DEFAULT_FILE_SEARCH_MAX_FILES: usize = 200_000;
const DEFAULT_FILE_SEARCH_WATCHERS_MAX: usize = 25;
const DEFAULT_FILE_SEARCH_WATCHER_TTL_SECS: u64 = 21600;
const DEFAULT_FILE_STATS_CACHE_MAX_REPOS: usize = 25;
const DEFAULT_FILE_STATS_CACHE_TTL_SECS: u64 = 3600;
const DEFAULT_APPROVALS_COMPLETED_TTL_SECS: u64 = 86400;
const DEFAULT_QUEUED_MESSAGES_TTL_SECS: u64 = 86400;
const DEFAULT_LOG_BACKFILL_COMPLETION_MAX_ENTRIES: usize = 10000;
const DEFAULT_LOG_BACKFILL_COMPLETION_TTL_SECS: u64 = 86400;
const DEFAULT_CACHE_WARN_AT_RATIO: f64 = 0.9;
const DEFAULT_CACHE_WARN_SAMPLE_SECS: u64 = 300;

#[derive(Debug, Clone)]
pub struct CacheBudgetConfig {
    pub file_search_cache_max_repos: usize,
    pub file_search_cache_ttl: Duration,
    pub file_search_max_files: usize,
    pub file_search_watchers_max: usize,
    pub file_search_watcher_ttl: Duration,
    pub file_stats_cache_max_repos: usize,
    pub file_stats_cache_ttl: Duration,
    pub approvals_completed_ttl: Duration,
    pub queued_messages_ttl: Duration,
    pub log_backfill_completion_max_entries: usize,
    pub log_backfill_completion_ttl: Duration,
    pub cache_warn_at_ratio: f64,
    pub cache_warn_sample: Duration,
}

impl Default for CacheBudgetConfig {
    fn default() -> Self {
        Self {
            file_search_cache_max_repos: DEFAULT_FILE_SEARCH_CACHE_MAX_REPOS,
            file_search_cache_ttl: Duration::from_secs(DEFAULT_FILE_SEARCH_CACHE_TTL_SECS),
            file_search_max_files: DEFAULT_FILE_SEARCH_MAX_FILES,
            file_search_watchers_max: DEFAULT_FILE_SEARCH_WATCHERS_MAX,
            file_search_watcher_ttl: Duration::from_secs(DEFAULT_FILE_SEARCH_WATCHER_TTL_SECS),
            file_stats_cache_max_repos: DEFAULT_FILE_STATS_CACHE_MAX_REPOS,
            file_stats_cache_ttl: Duration::from_secs(DEFAULT_FILE_STATS_CACHE_TTL_SECS),
            approvals_completed_ttl: Duration::from_secs(DEFAULT_APPROVALS_COMPLETED_TTL_SECS),
            queued_messages_ttl: Duration::from_secs(DEFAULT_QUEUED_MESSAGES_TTL_SECS),
            log_backfill_completion_max_entries: DEFAULT_LOG_BACKFILL_COMPLETION_MAX_ENTRIES,
            log_backfill_completion_ttl: Duration::from_secs(
                DEFAULT_LOG_BACKFILL_COMPLETION_TTL_SECS,
            ),
            cache_warn_at_ratio: DEFAULT_CACHE_WARN_AT_RATIO,
            cache_warn_sample: Duration::from_secs(DEFAULT_CACHE_WARN_SAMPLE_SECS),
        }
    }
}

impl CacheBudgetConfig {
    pub fn from_env() -> Self {
        Self::from_env_with(|name| env::var(name).ok())
    }

    fn from_env_with<F>(get_env: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        let defaults = Self::default();

        let file_search_cache_max_repos = read_env_usize(
            "VK_FILE_SEARCH_CACHE_MAX_REPOS",
            defaults.file_search_cache_max_repos,
            &get_env,
        );
        let file_search_max_files = read_env_usize(
            "VK_FILE_SEARCH_MAX_FILES",
            defaults.file_search_max_files,
            &get_env,
        );
        let file_search_watchers_max = read_env_usize(
            "VK_FILE_SEARCH_WATCHERS_MAX",
            defaults.file_search_watchers_max,
            &get_env,
        );
        let file_stats_cache_max_repos = read_env_usize(
            "VK_FILE_STATS_CACHE_MAX_REPOS",
            defaults.file_stats_cache_max_repos,
            &get_env,
        );
        let log_backfill_completion_max_entries = read_env_usize(
            "VK_LOG_BACKFILL_COMPLETION_MAX_ENTRIES",
            defaults.log_backfill_completion_max_entries,
            &get_env,
        );
        let cache_warn_at_ratio = clamp_ratio(read_env_f64(
            "VK_CACHE_WARN_AT_RATIO",
            defaults.cache_warn_at_ratio,
            &get_env,
        ));

        Self {
            file_search_cache_max_repos: normalize_max(
                file_search_cache_max_repos,
                "VK_FILE_SEARCH_CACHE_MAX_REPOS",
                defaults.file_search_cache_max_repos,
            ),
            file_search_cache_ttl: read_env_duration(
                "VK_FILE_SEARCH_CACHE_TTL_SECS",
                defaults.file_search_cache_ttl,
                &get_env,
            ),
            file_search_max_files: normalize_max(
                file_search_max_files,
                "VK_FILE_SEARCH_MAX_FILES",
                defaults.file_search_max_files,
            ),
            file_search_watchers_max: normalize_max(
                file_search_watchers_max,
                "VK_FILE_SEARCH_WATCHERS_MAX",
                defaults.file_search_watchers_max,
            ),
            file_search_watcher_ttl: read_env_duration(
                "VK_FILE_SEARCH_WATCHER_TTL_SECS",
                defaults.file_search_watcher_ttl,
                &get_env,
            ),
            file_stats_cache_max_repos: normalize_max(
                file_stats_cache_max_repos,
                "VK_FILE_STATS_CACHE_MAX_REPOS",
                defaults.file_stats_cache_max_repos,
            ),
            file_stats_cache_ttl: read_env_duration(
                "VK_FILE_STATS_CACHE_TTL_SECS",
                defaults.file_stats_cache_ttl,
                &get_env,
            ),
            approvals_completed_ttl: read_env_duration(
                "VK_APPROVALS_COMPLETED_TTL_SECS",
                defaults.approvals_completed_ttl,
                &get_env,
            ),
            queued_messages_ttl: read_env_duration(
                "VK_QUEUED_MESSAGES_TTL_SECS",
                defaults.queued_messages_ttl,
                &get_env,
            ),
            log_backfill_completion_max_entries: normalize_max(
                log_backfill_completion_max_entries,
                "VK_LOG_BACKFILL_COMPLETION_MAX_ENTRIES",
                defaults.log_backfill_completion_max_entries,
            ),
            log_backfill_completion_ttl: read_env_duration(
                "VK_LOG_BACKFILL_COMPLETION_TTL_SECS",
                defaults.log_backfill_completion_ttl,
                &get_env,
            ),
            cache_warn_at_ratio,
            cache_warn_sample: read_env_duration(
                "VK_CACHE_WARN_SAMPLE_SECS",
                defaults.cache_warn_sample,
                &get_env,
            ),
        }
    }
}

static CACHE_BUDGETS: Lazy<CacheBudgetConfig> = Lazy::new(CacheBudgetConfig::from_env);

pub fn cache_budgets() -> &'static CacheBudgetConfig {
    &CACHE_BUDGETS
}

static LAST_WARN: Lazy<Mutex<HashMap<&'static str, Instant>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn should_warn(cache_name: &'static str) -> bool {
    let sample = cache_budgets().cache_warn_sample;
    if sample.is_zero() {
        return true;
    }

    let mut last_warn = LAST_WARN.lock().unwrap();
    let now = Instant::now();

    match last_warn.get(cache_name) {
        Some(prev) if now.duration_since(*prev) < sample => false,
        _ => {
            last_warn.insert(cache_name, now);
            true
        }
    }
}

pub fn warn_threshold(max_entries: usize) -> usize {
    if max_entries == 0 {
        return 0;
    }

    let ratio = cache_budgets().cache_warn_at_ratio;
    let threshold = ((max_entries as f64) * ratio).ceil() as usize;
    threshold.max(1)
}

fn read_env_usize<F>(name: &str, default: usize, get_env: &F) -> usize
where
    F: Fn(&str) -> Option<String>,
{
    match get_env(name) {
        Some(value) => match value.parse::<usize>() {
            Ok(parsed) => parsed,
            Err(err) => {
                warn!("Invalid {name}='{value}': {err}. Using default {default}.");
                default
            }
        },
        None => default,
    }
}

fn read_env_f64<F>(name: &str, default: f64, get_env: &F) -> f64
where
    F: Fn(&str) -> Option<String>,
{
    match get_env(name) {
        Some(value) => match value.parse::<f64>() {
            Ok(parsed) => parsed,
            Err(err) => {
                warn!("Invalid {name}='{value}': {err}. Using default {default}.");
                default
            }
        },
        None => default,
    }
}

fn read_env_duration<F>(name: &str, default: Duration, get_env: &F) -> Duration
where
    F: Fn(&str) -> Option<String>,
{
    match get_env(name) {
        Some(value) => match value.parse::<u64>() {
            Ok(parsed) => Duration::from_secs(parsed),
            Err(err) => {
                warn!(
                    "Invalid {name}='{value}': {err}. Using default {}.",
                    default.as_secs()
                );
                default
            }
        },
        None => default,
    }
}

fn normalize_max(value: usize, name: &str, default: usize) -> usize {
    if value == 0 {
        warn!("{name} set to 0. Using minimum value 1 instead of default {default}.");
        1
    } else {
        value
    }
}

fn clamp_ratio(value: f64) -> f64 {
    if !(0.0..=1.0).contains(&value) {
        warn!("VK_CACHE_WARN_AT_RATIO out of range ({value}); clamping to 0.0-1.0.");
    }
    value.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn defaults_are_used_without_env() {
        let cfg = CacheBudgetConfig::from_env_with(|_| None);

        assert_eq!(
            cfg.file_search_cache_max_repos,
            DEFAULT_FILE_SEARCH_CACHE_MAX_REPOS
        );
        assert_eq!(
            cfg.file_search_cache_ttl.as_secs(),
            DEFAULT_FILE_SEARCH_CACHE_TTL_SECS
        );
        assert_eq!(cfg.file_search_max_files, DEFAULT_FILE_SEARCH_MAX_FILES);
        assert_eq!(
            cfg.file_search_watchers_max,
            DEFAULT_FILE_SEARCH_WATCHERS_MAX
        );
        assert_eq!(
            cfg.file_search_watcher_ttl.as_secs(),
            DEFAULT_FILE_SEARCH_WATCHER_TTL_SECS
        );
        assert_eq!(
            cfg.file_stats_cache_max_repos,
            DEFAULT_FILE_STATS_CACHE_MAX_REPOS
        );
        assert_eq!(
            cfg.file_stats_cache_ttl.as_secs(),
            DEFAULT_FILE_STATS_CACHE_TTL_SECS
        );
        assert_eq!(
            cfg.approvals_completed_ttl.as_secs(),
            DEFAULT_APPROVALS_COMPLETED_TTL_SECS
        );
        assert_eq!(
            cfg.queued_messages_ttl.as_secs(),
            DEFAULT_QUEUED_MESSAGES_TTL_SECS
        );
        assert_eq!(
            cfg.log_backfill_completion_max_entries,
            DEFAULT_LOG_BACKFILL_COMPLETION_MAX_ENTRIES
        );
        assert_eq!(
            cfg.log_backfill_completion_ttl.as_secs(),
            DEFAULT_LOG_BACKFILL_COMPLETION_TTL_SECS
        );
        assert_eq!(cfg.cache_warn_at_ratio, DEFAULT_CACHE_WARN_AT_RATIO);
        assert_eq!(
            cfg.cache_warn_sample.as_secs(),
            DEFAULT_CACHE_WARN_SAMPLE_SECS
        );
    }

    #[test]
    fn overrides_apply_and_normalize() {
        let mut envs = HashMap::new();
        envs.insert("VK_FILE_SEARCH_CACHE_MAX_REPOS", "10".to_string());
        envs.insert("VK_FILE_SEARCH_MAX_FILES", "100".to_string());
        envs.insert("VK_FILE_SEARCH_WATCHERS_MAX", "0".to_string());
        envs.insert("VK_FILE_STATS_CACHE_TTL_SECS", "120".to_string());
        envs.insert("VK_LOG_BACKFILL_COMPLETION_MAX_ENTRIES", "0".to_string());
        envs.insert("VK_LOG_BACKFILL_COMPLETION_TTL_SECS", "45".to_string());
        envs.insert("VK_CACHE_WARN_AT_RATIO", "0.5".to_string());

        let cfg = CacheBudgetConfig::from_env_with(|key| envs.get(key).cloned());

        assert_eq!(cfg.file_search_cache_max_repos, 10);
        assert_eq!(cfg.file_search_max_files, 100);
        assert_eq!(cfg.file_search_watchers_max, 1);
        assert_eq!(cfg.file_stats_cache_ttl.as_secs(), 120);
        assert_eq!(cfg.log_backfill_completion_max_entries, 1);
        assert_eq!(cfg.log_backfill_completion_ttl.as_secs(), 45);
        assert_eq!(cfg.cache_warn_at_ratio, 0.5);
    }
}
