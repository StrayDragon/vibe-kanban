use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;
use utils_core::diff::{Diff, DiffChangeKind, DiffSummary, compute_line_change_counts};

mod cli;

use cli::{ChangeType, NumstatEntry, StatusDiffEntry, StatusDiffOptions};
pub use cli::{GitCli, GitCliError};

use super::file_ranker::FileStat;
use crate::GitHubRepoInfo;

#[derive(Debug, Error)]
pub enum GitServiceError {
    #[error(transparent)]
    GitCLI(#[from] GitCliError),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error("Invalid repository: {0}")]
    InvalidRepository(String),
    #[error("Invalid branch name: {0}")]
    InvalidBranchName(String),
    #[error("Branch not found: {0}")]
    BranchNotFound(String),
    #[error("Merge conflicts: {0}")]
    MergeConflicts(String),
    #[error("Branches diverged: {0}")]
    BranchesDiverged(String),
    #[error("{0} has uncommitted changes: {1}")]
    WorktreeDirty(String, String),
    #[error("Rebase in progress; resolve or abort it before retrying")]
    RebaseInProgress,
}

/// Service for managing Git operations in task execution workflows
#[derive(Clone)]
pub struct GitService {}

const GIT_REMOTE_STATUS_FETCH_TTL_ENV: &str = "VK_GIT_REMOTE_STATUS_FETCH_TTL_SECS";
const DEFAULT_GIT_REMOTE_STATUS_FETCH_TTL_SECS: u64 = 30;
const DEFAULT_GIT_REMOTE_STATUS_FETCH_FAILURE_COOLDOWN_SECS: u64 = 10;

static REMOTE_STATUS_FETCH_TTL: Lazy<Duration> = Lazy::new(|| {
    match std::env::var(GIT_REMOTE_STATUS_FETCH_TTL_ENV) {
        Ok(value) => match value.trim().parse::<u64>() {
            Ok(parsed) => Duration::from_secs(parsed),
            Err(err) => {
                tracing::warn!(
                    "Invalid {GIT_REMOTE_STATUS_FETCH_TTL_ENV}='{value}': {err}. Using default {DEFAULT_GIT_REMOTE_STATUS_FETCH_TTL_SECS}s."
                );
                Duration::from_secs(DEFAULT_GIT_REMOTE_STATUS_FETCH_TTL_SECS)
            }
        },
        Err(_) => Duration::from_secs(DEFAULT_GIT_REMOTE_STATUS_FETCH_TTL_SECS),
    }
});

fn remote_status_fetch_ttl() -> Duration {
    #[cfg(test)]
    if let Some(secs) = remote_status_fetch_test_support::ttl_override() {
        return Duration::from_secs(secs);
    }
    *REMOTE_STATUS_FETCH_TTL
}

fn remote_status_fetch_failure_cooldown(ttl: Duration) -> Duration {
    let default = Duration::from_secs(DEFAULT_GIT_REMOTE_STATUS_FETCH_FAILURE_COOLDOWN_SECS);
    if ttl.is_zero() {
        default
    } else {
        default.min(ttl)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RemoteStatusFetchKey {
    repo_path: PathBuf,
    remote: String,
    branch: String,
}

#[derive(Debug, Default, Clone, Copy)]
struct RemoteStatusFetchState {
    in_progress: bool,
    last_success: Option<Instant>,
    last_failure: Option<Instant>,
}

static REMOTE_STATUS_FETCH_STATE: Lazy<
    DashMap<RemoteStatusFetchKey, Arc<Mutex<RemoteStatusFetchState>>>,
> = Lazy::new(DashMap::new);

#[cfg(test)]
mod remote_status_fetch_test_support {
    use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

    use super::REMOTE_STATUS_FETCH_STATE;

    pub(super) static REMOTE_STATUS_FETCH_ATTEMPTS: AtomicU64 = AtomicU64::new(0);
    static REMOTE_STATUS_FETCH_TTL_OVERRIDE_SECS: AtomicI64 = AtomicI64::new(-1);

    pub(super) fn record_attempt() {
        REMOTE_STATUS_FETCH_ATTEMPTS.fetch_add(1, Ordering::SeqCst);
    }

    pub(super) fn set_ttl_override(secs: u64) {
        let value = i64::try_from(secs).unwrap_or(i64::MAX);
        REMOTE_STATUS_FETCH_TTL_OVERRIDE_SECS.store(value, Ordering::SeqCst);
    }

    pub(super) fn ttl_override() -> Option<u64> {
        let value = REMOTE_STATUS_FETCH_TTL_OVERRIDE_SECS.load(Ordering::SeqCst);
        if value < 0 {
            None
        } else {
            u64::try_from(value).ok()
        }
    }

    pub(super) fn reset_state() {
        REMOTE_STATUS_FETCH_STATE.clear();
        REMOTE_STATUS_FETCH_ATTEMPTS.store(0, Ordering::SeqCst);
        REMOTE_STATUS_FETCH_TTL_OVERRIDE_SECS.store(-1, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitBranchType {
    Local,
    Remote,
}

// Max inline diff size for UI (in bytes). Files larger than this will have
// their contents omitted from the diff stream to avoid UI crashes.
const MAX_INLINE_DIFF_BYTES: usize = 2 * 1024 * 1024; // ~2MB

type NumstatIndexKey = (Option<String>, String);
type NumstatIndexValue = (Option<usize>, Option<usize>);
type NumstatIndex = HashMap<NumstatIndexKey, NumstatIndexValue>;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ConflictOp {
    Rebase,
    Merge,
    CherryPick,
    Revert,
}

#[derive(Debug, Serialize, TS)]
pub struct GitBranch {
    pub name: String,
    pub is_current: bool,
    pub is_remote: bool,
    #[ts(type = "Date")]
    pub last_commit_date: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct HeadInfo {
    pub branch: String,
    pub oid: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Commit(String);

impl Commit {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Commit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WorktreeResetOptions {
    pub perform_reset: bool,
    pub force_when_dirty: bool,
    pub is_dirty: bool,
    pub log_skip_when_dirty: bool,
}

impl WorktreeResetOptions {
    pub fn new(
        perform_reset: bool,
        force_when_dirty: bool,
        is_dirty: bool,
        log_skip_when_dirty: bool,
    ) -> Self {
        Self {
            perform_reset,
            force_when_dirty,
            is_dirty,
            log_skip_when_dirty,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct WorktreeResetOutcome {
    pub needed: bool,
    pub applied: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct GitCommitOptions {
    pub no_verify: bool,
}

impl GitCommitOptions {
    pub fn new(no_verify: bool) -> Self {
        Self { no_verify }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct GitMergeOptions {
    pub no_verify: bool,
}

impl GitMergeOptions {
    pub fn new(no_verify: bool) -> Self {
        Self { no_verify }
    }
}

/// Target for diff generation
pub enum DiffTarget<'p> {
    /// Work-in-progress branch checked out in this worktree
    Worktree {
        worktree_path: &'p Path,
        base_commit: &'p Commit,
    },
    /// Fully committed branch vs base branch
    Branch {
        repo_path: &'p Path,
        branch_name: &'p str,
        base_branch: &'p str,
    },
    /// Specific commit vs base branch
    Commit {
        repo_path: &'p Path,
        commit_sha: &'p str,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum DiffContentPolicy {
    Full,
    OmitContents,
}

#[derive(Debug)]
pub struct WorktreeDiffPlan {
    worktree_path: PathBuf,
    base_rev: String,
    entries: Vec<StatusDiffEntry>,
    numstat_index: NumstatIndex,
    additions: usize,
    deletions: usize,
    total_bytes: usize,
    numstat_error: Option<String>,
}

impl WorktreeDiffPlan {
    pub fn stats_error(&self) -> Option<&str> {
        self.numstat_error.as_deref()
    }

    pub fn summary(&self) -> DiffSummary {
        DiffSummary {
            file_count: self.entries.len(),
            added: self.additions,
            deleted: self.deletions,
            total_bytes: self.total_bytes,
        }
    }

    pub fn listed_paths(&self) -> Vec<String> {
        let mut seen = std::collections::BTreeSet::new();
        for entry in &self.entries {
            let (old_path_opt, new_path_opt) = GitService::entry_paths(entry);
            if let Some(path) = new_path_opt.or(old_path_opt) {
                seen.insert(path);
            }
        }
        seen.into_iter().collect()
    }

    pub fn diffs(
        &self,
        content_policy: DiffContentPolicy,
        path_filter: Option<&[&str]>,
    ) -> Vec<Diff> {
        let git = GitCli::new();
        let filter: Option<std::collections::HashSet<&str>> =
            path_filter.map(|paths| paths.iter().copied().collect());

        self.entries
            .iter()
            .filter(|entry| {
                let Some(filter) = filter.as_ref() else {
                    return true;
                };
                if filter.contains(entry.path.as_str()) {
                    return true;
                }
                entry
                    .old_path
                    .as_deref()
                    .is_some_and(|old| filter.contains(old))
            })
            .cloned()
            .map(|entry| {
                GitService::status_entry_to_diff_worktree(
                    &git,
                    &self.worktree_path,
                    &self.base_rev,
                    entry,
                    content_policy,
                    &self.numstat_index,
                )
            })
            .collect()
    }
}

impl Default for GitService {
    fn default() -> Self {
        Self::new()
    }
}

impl GitService {
    pub fn new() -> Self {
        Self {}
    }

    pub fn is_branch_name_valid(&self, name: &str) -> bool {
        GitCli::new().check_ref_format_branch(name).unwrap_or(false)
    }

    pub fn default_remote_name(&self, repo_path: &Path) -> String {
        GitCli::new()
            .remote_names(repo_path)
            .ok()
            .and_then(|mut remotes| remotes.drain(..).next())
            .unwrap_or_else(|| "origin".to_string())
    }

    fn default_remote_name_checked(&self, repo_path: &Path) -> Result<String, GitServiceError> {
        let mut remotes = GitCli::new()
            .remote_names(repo_path)
            .map_err(|e| GitServiceError::InvalidRepository(format!("git remote failed: {e}")))?;
        remotes
            .drain(..)
            .next()
            .ok_or_else(|| GitServiceError::InvalidRepository("No git remotes found".to_string()))
    }

    fn remote_url(&self, repo_path: &Path, remote_name: &str) -> Result<String, GitServiceError> {
        GitCli::new()
            .remote_get_url(repo_path, remote_name)
            .map_err(|e| {
                GitServiceError::InvalidRepository(format!("git remote get-url failed: {e}"))
            })
    }

    pub fn get_worktree_diff_plan(
        &self,
        worktree_path: &Path,
        base_commit: &Commit,
        path_filter: Option<&[&str]>,
    ) -> Result<WorktreeDiffPlan, GitServiceError> {
        let git = GitCli::new();
        let cli_opts = StatusDiffOptions {
            path_filter: path_filter.map(|fs| fs.iter().map(|s| s.to_string()).collect()),
        };

        let (entries, numstats, numstat_err) = git
            .diff_status_and_numstat_entries_best_effort(worktree_path, base_commit, cli_opts)
            .map_err(|e| GitServiceError::InvalidRepository(format!("git diff failed: {e}")))?;

        let additions = numstats
            .iter()
            .map(|e| e.additions.unwrap_or(0))
            .sum::<usize>();
        let deletions = numstats
            .iter()
            .map(|e| e.deletions.unwrap_or(0))
            .sum::<usize>();

        let total_bytes = Self::compute_entry_total_bytes_worktree(
            &git,
            worktree_path,
            base_commit.as_str(),
            &entries,
        );

        Ok(WorktreeDiffPlan {
            worktree_path: worktree_path.to_path_buf(),
            base_rev: base_commit.as_str().to_string(),
            entries,
            numstat_index: Self::index_numstats(numstats),
            additions,
            deletions,
            total_bytes,
            numstat_error: numstat_err.map(|e| e.to_string()),
        })
    }

    /// Ensure a local branch exists, creating it from `base_branch` when missing.
    ///
    /// - If the branch already exists locally, this is a no-op.
    /// - `base_branch` may be local or remote (for example `origin/main`).
    /// - This does not check out the branch or mutate the worktree.
    pub fn ensure_local_branch_from_base(
        &self,
        repo_path: &Path,
        branch_name: &str,
        base_branch: &str,
    ) -> Result<(), GitServiceError> {
        let branch_name = branch_name.trim();
        if branch_name.is_empty() || !self.is_branch_name_valid(branch_name) {
            return Err(GitServiceError::InvalidBranchName(branch_name.to_string()));
        }

        let base_branch = base_branch.trim();
        if base_branch.is_empty() {
            return Err(GitServiceError::BranchNotFound(
                "base branch cannot be empty".to_string(),
            ));
        }

        let git = GitCli::new();

        // Already exists locally.
        let local_ref = format!("refs/heads/{branch_name}");
        if git
            .git(repo_path, ["show-ref", "--verify", "--quiet", &local_ref])
            .is_ok()
        {
            return Ok(());
        }

        // Resolve base ref. If base is not remote-qualified, also try <default_remote>/<base>.
        let mut base_ref = base_branch.to_string();
        if git.rev_parse(repo_path, base_branch).is_err() {
            if !base_branch.contains('/') {
                let remote_name = self.default_remote_name(repo_path);
                let candidate = format!("{remote_name}/{base_branch}");
                if git.rev_parse(repo_path, &candidate).is_ok() {
                    base_ref = candidate;
                } else {
                    return Err(GitServiceError::BranchNotFound(base_branch.to_string()));
                }
            } else {
                return Err(GitServiceError::BranchNotFound(base_branch.to_string()));
            }
        }

        match git.git(repo_path, ["branch", branch_name, &base_ref]) {
            Ok(_) => Ok(()),
            Err(GitCliError::CommandFailed(msg))
                if msg.to_ascii_lowercase().contains("already exists") =>
            {
                // Another concurrent request may have created it between our check and create.
                Ok(())
            }
            Err(e) => Err(GitServiceError::InvalidRepository(format!(
                "git branch failed: {e}"
            ))),
        }?;

        Ok(())
    }

    /// Push a local branch ref to the default remote (if any).
    ///
    /// This is intended for "integration branch" publishing and does not require
    /// the worktree to be clean.
    pub fn push_branch_ref(
        &self,
        repo_path: &Path,
        branch_name: &str,
        force: bool,
    ) -> Result<(), GitServiceError> {
        let remote_name = self.default_remote_name_checked(repo_path)?;
        GitCli::new()
            .push(repo_path, &remote_name, branch_name, force)
            .map_err(GitServiceError::from)?;
        Ok(())
    }

    /// Initialize a new git repository with a main branch and initial commit
    pub fn initialize_repo_with_main_branch(
        &self,
        repo_path: &Path,
    ) -> Result<(), GitServiceError> {
        let git = GitCli::new();
        git.init(repo_path, "main")?;
        git.commit_allow_empty(repo_path, "Initial commit")?;
        Ok(())
    }

    /// Ensure an existing repository has a main branch (for empty repos)
    pub fn ensure_main_branch_exists(&self, repo_path: &Path) -> Result<(), GitServiceError> {
        let git = GitCli::new();
        if git.rev_parse(repo_path, "HEAD").is_ok() {
            return Ok(());
        }
        let _ = git.git(repo_path, ["symbolic-ref", "HEAD", "refs/heads/main"]);
        git.commit_allow_empty(repo_path, "Initial commit")?;
        Ok(())
    }

    pub fn commit(&self, path: &Path, message: &str) -> Result<bool, GitServiceError> {
        self.commit_with_options(path, message, GitCommitOptions::default())
    }

    pub fn commit_with_options(
        &self,
        path: &Path,
        message: &str,
        options: GitCommitOptions,
    ) -> Result<bool, GitServiceError> {
        // Use Git CLI to respect sparse-checkout semantics for staging and commit
        let git = GitCli::new();
        let has_changes = git
            .has_changes(path)
            .map_err(|e| GitServiceError::InvalidRepository(format!("git status failed: {e}")))?;
        if !has_changes {
            tracing::debug!("No changes to commit!");
            return Ok(false);
        }

        git.add_all(path)
            .map_err(|e| GitServiceError::InvalidRepository(format!("git add failed: {e}")))?;
        git.commit_with_options(path, message, options)
            .map_err(|e| GitServiceError::InvalidRepository(format!("git commit failed: {e}")))?;
        Ok(true)
    }

    /// Get diffs between branches or worktree changes
    pub fn get_diffs(
        &self,
        target: DiffTarget,
        path_filter: Option<&[&str]>,
        content_policy: DiffContentPolicy,
    ) -> Result<Vec<Diff>, GitServiceError> {
        let git = GitCli::new();
        match target {
            DiffTarget::Worktree {
                worktree_path,
                base_commit,
            } => {
                let plan = self.get_worktree_diff_plan(worktree_path, base_commit, path_filter)?;
                Ok(plan.diffs(content_policy, None))
            }
            DiffTarget::Branch {
                repo_path,
                branch_name,
                base_branch,
            } => {
                let entries =
                    git.diff_name_status_between(repo_path, base_branch, branch_name, path_filter)?;
                let numstats = git
                    .diff_numstat_between(repo_path, base_branch, branch_name, path_filter)
                    .unwrap_or_default();
                let stats = Self::index_numstats(numstats);

                Ok(entries
                    .into_iter()
                    .map(|e| {
                        Self::status_entry_to_diff_between_revs(
                            &git,
                            repo_path,
                            base_branch,
                            branch_name,
                            e,
                            content_policy,
                            &stats,
                        )
                    })
                    .collect())
            }
            DiffTarget::Commit {
                repo_path,
                commit_sha,
            } => {
                let parents = git
                    .show_format(repo_path, commit_sha, "%P")
                    .unwrap_or_default();
                let parent = parents
                    .split_whitespace()
                    .next()
                    .ok_or_else(|| {
                        GitServiceError::InvalidRepository(
                            "Commit has no parent; cannot diff without a baseline".into(),
                        )
                    })?
                    .to_string();

                let entries =
                    git.diff_name_status_between(repo_path, &parent, commit_sha, path_filter)?;
                let numstats = git
                    .diff_numstat_between(repo_path, &parent, commit_sha, path_filter)
                    .unwrap_or_default();
                let stats = Self::index_numstats(numstats);

                Ok(entries
                    .into_iter()
                    .map(|e| {
                        Self::status_entry_to_diff_between_revs(
                            &git,
                            repo_path,
                            &parent,
                            commit_sha,
                            e,
                            content_policy,
                            &stats,
                        )
                    })
                    .collect())
            }
        }
    }

    pub fn get_worktree_diff_summary(
        &self,
        worktree_path: &Path,
        base_commit: &Commit,
        path_filter: Option<&[&str]>,
    ) -> Result<DiffSummary, GitServiceError> {
        let plan = self.get_worktree_diff_plan(worktree_path, base_commit, path_filter)?;
        if let Some(err) = plan.stats_error() {
            return Err(GitServiceError::InvalidRepository(format!(
                "git diff failed: {err}"
            )));
        }
        Ok(plan.summary())
    }

    /// Extract file path from a Diff (for indexing and ConversationPatch)
    pub fn diff_path(diff: &Diff) -> String {
        diff.new_path
            .clone()
            .or_else(|| diff.old_path.clone())
            .unwrap_or_default()
    }

    fn index_numstats(entries: Vec<NumstatEntry>) -> NumstatIndex {
        let mut out: NumstatIndex = HashMap::new();
        for e in entries {
            out.insert(
                (e.old_path.clone(), e.path.clone()),
                (e.additions, e.deletions),
            );
        }
        out
    }

    fn status_entry_to_diff_worktree(
        git: &GitCli,
        worktree_path: &Path,
        base_rev: &str,
        e: StatusDiffEntry,
        content_policy: DiffContentPolicy,
        numstat_index: &NumstatIndex,
    ) -> Diff {
        let omit_contents = matches!(content_policy, DiffContentPolicy::OmitContents);
        let mut change = Self::map_change_type(&e.change);

        let (old_path_opt, new_path_opt) = Self::entry_paths(&e);
        let key = (e.old_path.clone(), e.path.clone());

        let mut content_omitted = omit_contents;
        if !content_omitted {
            if let Some(ref oldp) = old_path_opt
                && let Some(size) = Self::git_blob_size(git, worktree_path, base_rev, oldp)
                && size > MAX_INLINE_DIFF_BYTES
            {
                content_omitted = true;
            }
            if let Some(ref newp) = new_path_opt
                && let Some(size) = Self::fs_file_size(worktree_path, newp)
                && size > MAX_INLINE_DIFF_BYTES
            {
                content_omitted = true;
            }
        }

        let (old_content, new_content) = if content_omitted {
            (None, None)
        } else {
            let old = old_path_opt
                .as_deref()
                .and_then(|p| Self::read_git_file_to_string(git, worktree_path, base_rev, p));
            let new = new_path_opt
                .as_deref()
                .and_then(|p| Self::read_fs_file_to_string(worktree_path, p));
            (old, new)
        };

        if matches!(change, DiffChangeKind::Modified)
            && old_content.is_some()
            && new_content.is_some()
            && old_content == new_content
        {
            change = DiffChangeKind::PermissionChange;
        }

        let (additions, deletions) = if content_omitted {
            numstat_index.get(&key).cloned().unwrap_or((None, None))
        } else if omit_contents {
            (None, None)
        } else {
            Self::compute_line_stats(&old_content, &new_content)
        };

        Diff {
            change,
            old_path: old_path_opt,
            new_path: new_path_opt,
            old_content,
            new_content,
            content_omitted,
            additions,
            deletions,
        }
    }

    fn status_entry_to_diff_between_revs(
        git: &GitCli,
        repo_path: &Path,
        from_rev: &str,
        to_rev: &str,
        e: StatusDiffEntry,
        content_policy: DiffContentPolicy,
        numstat_index: &NumstatIndex,
    ) -> Diff {
        let omit_contents = matches!(content_policy, DiffContentPolicy::OmitContents);
        let mut change = Self::map_change_type(&e.change);

        let (old_path_opt, new_path_opt) = Self::entry_paths(&e);
        let key = (e.old_path.clone(), e.path.clone());

        let mut content_omitted = omit_contents;
        if !content_omitted {
            if let Some(ref oldp) = old_path_opt
                && let Some(size) = Self::git_blob_size(git, repo_path, from_rev, oldp)
                && size > MAX_INLINE_DIFF_BYTES
            {
                content_omitted = true;
            }
            if let Some(ref newp) = new_path_opt
                && let Some(size) = Self::git_blob_size(git, repo_path, to_rev, newp)
                && size > MAX_INLINE_DIFF_BYTES
            {
                content_omitted = true;
            }
        }

        let (old_content, new_content) = if content_omitted {
            (None, None)
        } else {
            let old = old_path_opt
                .as_deref()
                .and_then(|p| Self::read_git_file_to_string(git, repo_path, from_rev, p));
            let new = new_path_opt
                .as_deref()
                .and_then(|p| Self::read_git_file_to_string(git, repo_path, to_rev, p));
            (old, new)
        };

        if matches!(change, DiffChangeKind::Modified)
            && old_content.is_some()
            && new_content.is_some()
            && old_content == new_content
        {
            change = DiffChangeKind::PermissionChange;
        }

        let (additions, deletions) = if content_omitted {
            numstat_index.get(&key).cloned().unwrap_or((None, None))
        } else if omit_contents {
            (None, None)
        } else {
            Self::compute_line_stats(&old_content, &new_content)
        };

        Diff {
            change,
            old_path: old_path_opt,
            new_path: new_path_opt,
            old_content,
            new_content,
            content_omitted,
            additions,
            deletions,
        }
    }

    fn map_change_type(kind: &ChangeType) -> DiffChangeKind {
        match kind {
            ChangeType::Added => DiffChangeKind::Added,
            ChangeType::Deleted => DiffChangeKind::Deleted,
            ChangeType::Modified => DiffChangeKind::Modified,
            ChangeType::Renamed => DiffChangeKind::Renamed,
            ChangeType::Copied => DiffChangeKind::Copied,
            ChangeType::TypeChanged | ChangeType::Unmerged | ChangeType::Unknown(_) => {
                DiffChangeKind::Modified
            }
        }
    }

    fn entry_paths(e: &StatusDiffEntry) -> (Option<String>, Option<String>) {
        match &e.change {
            ChangeType::Added => (None, Some(e.path.clone())),
            ChangeType::Deleted => (
                Some(e.old_path.clone().unwrap_or_else(|| e.path.clone())),
                None,
            ),
            ChangeType::Modified | ChangeType::TypeChanged | ChangeType::Unmerged => (
                Some(e.old_path.clone().unwrap_or_else(|| e.path.clone())),
                Some(e.path.clone()),
            ),
            ChangeType::Renamed | ChangeType::Copied => (e.old_path.clone(), Some(e.path.clone())),
            ChangeType::Unknown(_) => (e.old_path.clone(), Some(e.path.clone())),
        }
    }

    fn git_blob_size(git: &GitCli, repo_path: &Path, rev: &str, path: &str) -> Option<usize> {
        let spec = format!("{rev}:{path}");
        let oid = git.rev_parse(repo_path, &spec).ok()?;
        git.cat_file_size(repo_path, &oid).ok()
    }

    fn fs_file_size(worktree_path: &Path, rel_path: &str) -> Option<usize> {
        let abs = worktree_path.join(rel_path);
        let md = std::fs::metadata(&abs).ok()?;
        Some(md.len() as usize)
    }

    fn read_fs_file_to_string(worktree_path: &Path, rel_path: &str) -> Option<String> {
        let abs = worktree_path.join(rel_path);
        let bytes = std::fs::read(&abs).ok()?;
        if bytes.len() > MAX_INLINE_DIFF_BYTES {
            return None;
        }
        if bytes.contains(&0) {
            return None;
        }
        String::from_utf8(bytes).ok()
    }

    fn read_git_file_to_string(
        git: &GitCli,
        repo_path: &Path,
        rev: &str,
        rel_path: &str,
    ) -> Option<String> {
        let bytes = git.show_file_at_rev(repo_path, rev, rel_path).ok()?;
        if bytes.len() > MAX_INLINE_DIFF_BYTES {
            return None;
        }
        if bytes.contains(&0) {
            return None;
        }
        String::from_utf8(bytes).ok()
    }

    fn compute_line_stats(
        old_content: &Option<String>,
        new_content: &Option<String>,
    ) -> (Option<usize>, Option<usize>) {
        match (old_content, new_content) {
            (Some(old), Some(new)) => {
                let (adds, dels) = compute_line_change_counts(old, new);
                (Some(adds), Some(dels))
            }
            (Some(old), None) => (Some(0), Some(old.lines().count())),
            (None, Some(new)) => (Some(new.lines().count()), Some(0)),
            (None, None) => (None, None),
        }
    }

    fn compute_entry_total_bytes_worktree(
        git: &GitCli,
        worktree_path: &Path,
        base_rev: &str,
        entries: &[StatusDiffEntry],
    ) -> usize {
        let mut total = 0usize;

        for entry in entries {
            let old_path = entry.old_path.as_deref().unwrap_or(&entry.path);
            let new_path = entry.path.as_str();

            let (include_old, include_new) = match &entry.change {
                ChangeType::Added => (false, true),
                ChangeType::Deleted => (true, false),
                ChangeType::Modified
                | ChangeType::Renamed
                | ChangeType::Copied
                | ChangeType::TypeChanged
                | ChangeType::Unmerged
                | ChangeType::Unknown(_) => (true, true),
            };

            if include_old
                && let Some(size) = Self::git_blob_size(git, worktree_path, base_rev, old_path)
            {
                total = total.saturating_add(size);
            }
            if include_new && let Some(size) = Self::fs_file_size(worktree_path, new_path) {
                total = total.saturating_add(size);
            }
        }

        total
    }

    /// Find where a branch is currently checked out
    fn find_checkout_path_for_branch(
        &self,
        repo_path: &Path,
        branch_name: &str,
    ) -> Result<Option<PathBuf>, GitServiceError> {
        let git_cli = GitCli::new();
        let worktrees = git_cli.list_worktrees(repo_path).map_err(|e| {
            GitServiceError::InvalidRepository(format!("git worktree list failed: {e}"))
        })?;

        for worktree in worktrees {
            if let Some(ref branch) = worktree.branch
                && branch == branch_name
            {
                return Ok(Some(PathBuf::from(worktree.path)));
            }
        }
        Ok(None)
    }

    /// Merge changes from a task branch into the base branch.
    pub fn merge_changes(
        &self,
        base_worktree_path: &Path,
        task_worktree_path: &Path,
        task_branch_name: &str,
        base_branch_name: &str,
        commit_message: &str,
    ) -> Result<String, GitServiceError> {
        self.merge_changes_with_options(
            base_worktree_path,
            task_worktree_path,
            task_branch_name,
            base_branch_name,
            commit_message,
            GitMergeOptions::default(),
        )
    }

    pub fn merge_changes_with_options(
        &self,
        base_worktree_path: &Path,
        _task_worktree_path: &Path,
        task_branch_name: &str,
        base_branch_name: &str,
        commit_message: &str,
        options: GitMergeOptions,
    ) -> Result<String, GitServiceError> {
        // Check if base branch is ahead of task branch - this indicates the base has moved
        // ahead since the task was created, which should block the merge.
        let (_, task_behind) =
            self.get_branch_status(base_worktree_path, task_branch_name, base_branch_name)?;
        if task_behind > 0 {
            return Err(GitServiceError::BranchesDiverged(format!(
                "Cannot merge: base branch '{base_branch_name}' is {task_behind} commits ahead of task branch '{task_branch_name}'. The base branch has moved forward since the task was created.",
            )));
        }

        let git = GitCli::new();

        let existing_checkout =
            self.find_checkout_path_for_branch(base_worktree_path, base_branch_name)?;

        let mut tmp_worktree: Option<tempfile::TempDir> = None;
        let merge_worktree_path = match existing_checkout {
            Some(path) => path,
            None => {
                // Base branch is not checked out anywhere: create a temporary worktree to run the squash merge.
                let tmp = tempfile::TempDir::new().map_err(|e| {
                    GitServiceError::InvalidRepository(format!("temp dir create failed: {e}"))
                })?;
                git.worktree_add(base_worktree_path, tmp.path(), base_branch_name)
                    .map_err(|e| {
                        GitServiceError::InvalidRepository(format!("git worktree add failed: {e}"))
                    })?;
                let path = tmp.path().to_path_buf();
                tmp_worktree = Some(tmp);
                path
            }
        };

        // Safety check: base worktree has no staged changes; attempt to clean stale merge state.
        let mut has_staged = git.has_staged_changes(&merge_worktree_path).map_err(|e| {
            GitServiceError::InvalidRepository(format!("git diff --cached failed: {e}"))
        })?;
        if has_staged
            && let Ok(conflicts) = git.get_conflicted_files(&merge_worktree_path)
            && !conflicts.is_empty()
        {
            if let Err(err) = git.reset_merge(&merge_worktree_path) {
                tracing::warn!(
                    "Failed to reset stale merge state in {}: {}",
                    merge_worktree_path.display(),
                    err
                );
            }
            has_staged = git.has_staged_changes(&merge_worktree_path).map_err(|e| {
                GitServiceError::InvalidRepository(format!("git diff --cached failed: {e}"))
            })?;
        }
        if has_staged {
            return Err(GitServiceError::WorktreeDirty(
                base_branch_name.to_string(),
                "staged changes present".to_string(),
            ));
        }

        let sha = git
            .merge_squash_commit_with_options(
                &merge_worktree_path,
                base_branch_name,
                task_branch_name,
                commit_message,
                options,
            )
            .map_err(|e| GitServiceError::InvalidRepository(format!("CLI merge failed: {e}")))?;

        // Update task branch ref for continuity.
        let task_refname = format!("refs/heads/{task_branch_name}");
        git.update_ref(base_worktree_path, &task_refname, &sha)
            .map_err(|e| {
                GitServiceError::InvalidRepository(format!("git update-ref failed: {e}"))
            })?;

        // If we created a temporary worktree, remove it best-effort (drop will also attempt to delete the dir).
        if tmp_worktree.is_some() {
            let _ = git.worktree_remove(base_worktree_path, &merge_worktree_path, true);
            let _ = git.worktree_prune(base_worktree_path);
        }

        Ok(sha)
    }

    pub fn get_branch_status(
        &self,
        repo_path: &Path,
        branch_name: &str,
        base_branch_name: &str,
    ) -> Result<(usize, usize), GitServiceError> {
        GitCli::new()
            .rev_list_left_right_count(repo_path, branch_name, base_branch_name)
            .map_err(GitServiceError::from)
    }

    pub fn get_base_commit(
        &self,
        repo_path: &Path,
        branch_name: &str,
        base_branch_name: &str,
    ) -> Result<Commit, GitServiceError> {
        let sha = GitCli::new().merge_base(repo_path, branch_name, base_branch_name)?;
        Ok(Commit::new(sha))
    }

    pub fn get_remote_branch_status(
        &self,
        repo_path: &Path,
        branch_name: &str,
        base_branch_name: Option<&str>,
    ) -> Result<(usize, usize), GitServiceError> {
        fn split_remote_tracking_branch(base: &str) -> Option<(&str, &str)> {
            let (remote, rest) = base.split_once('/')?;
            if remote.is_empty() || rest.is_empty() {
                return None;
            }
            Some((remote, rest))
        }

        fn maybe_refresh_remote_tracking_branch(
            repo_path: &Path,
            remote: &str,
            remote_branch: &str,
        ) {
            let ttl = remote_status_fetch_ttl();
            let failure_cooldown = remote_status_fetch_failure_cooldown(ttl);
            let now = Instant::now();

            let key = RemoteStatusFetchKey {
                repo_path: repo_path.to_path_buf(),
                remote: remote.to_string(),
                branch: remote_branch.to_string(),
            };

            let state = REMOTE_STATUS_FETCH_STATE
                .entry(key)
                .or_insert_with(|| Arc::new(Mutex::new(RemoteStatusFetchState::default())))
                .clone();

            {
                let mut locked = state.lock().unwrap();
                if locked.in_progress {
                    return;
                }

                if !ttl.is_zero()
                    && locked
                        .last_success
                        .is_some_and(|last| now.duration_since(last) < ttl)
                {
                    return;
                }

                if locked
                    .last_failure
                    .is_some_and(|last| now.duration_since(last) < failure_cooldown)
                {
                    return;
                }

                locked.in_progress = true;
            }

            let git = GitCli::new();
            let refspec =
                format!("+refs/heads/{remote_branch}:refs/remotes/{remote}/{remote_branch}");

            #[cfg(test)]
            remote_status_fetch_test_support::record_attempt();

            let result = git.fetch_with_refspec(repo_path, remote, &refspec);

            {
                let mut locked = state.lock().unwrap();
                locked.in_progress = false;
                match result {
                    Ok(()) => {
                        locked.last_success = Some(now);
                        locked.last_failure = None;
                    }
                    Err(_) => {
                        locked.last_failure = Some(now);
                    }
                }
            }

            if let Err(err) = result {
                tracing::debug!(
                    repo = ?repo_path,
                    remote = remote,
                    remote_branch = remote_branch,
                    error = %err,
                    "remote tracking ref refresh failed; using existing refs"
                );
            }
        }

        let git = GitCli::new();

        let base = if let Some(bn) = base_branch_name {
            bn.to_string()
        } else {
            let refname = format!("refs/heads/{branch_name}");
            let upstream = git
                .for_each_ref(repo_path, &[&refname], "%(upstream:short)")
                .unwrap_or_default();
            let upstream = upstream.lines().next().unwrap_or("").trim().to_string();
            if upstream.is_empty() {
                return Err(GitServiceError::InvalidRepository(format!(
                    "Branch '{branch_name}' has no upstream configured"
                )));
            }
            upstream
        };

        let (remote, remote_branch) = split_remote_tracking_branch(&base).ok_or_else(|| {
            GitServiceError::InvalidRepository(format!(
                "Remote-tracking branch '{base}' is not in '<remote>/<branch>' form"
            ))
        })?;

        // Best-effort: refresh the specific remote-tracking branch, TTL-gated.
        maybe_refresh_remote_tracking_branch(repo_path, remote, remote_branch);

        git.rev_list_left_right_count(repo_path, branch_name, &base)
            .map_err(GitServiceError::from)
    }

    pub fn is_worktree_clean(&self, worktree_path: &Path) -> Result<bool, GitServiceError> {
        match self.check_worktree_clean(worktree_path) {
            Ok(()) => Ok(true),
            Err(GitServiceError::WorktreeDirty(_, _)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Check if the worktree is clean (no uncommitted changes to tracked files).
    /// Untracked files are allowed.
    fn check_worktree_clean(&self, worktree_path: &Path) -> Result<(), GitServiceError> {
        let git = GitCli::new();
        let st = git
            .get_worktree_status(worktree_path)
            .map_err(|e| GitServiceError::InvalidRepository(format!("git status failed: {e}")))?;

        if st.uncommitted_tracked == 0 {
            return Ok(());
        }

        let branch_name = git
            .git(
                worktree_path,
                ["symbolic-ref", "--quiet", "--short", "HEAD"],
            )
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown branch".to_string());

        let mut dirty_files = Vec::new();
        for entry in st.entries {
            if entry.is_untracked {
                continue;
            }
            if entry.staged != ' ' || entry.unstaged != ' ' {
                dirty_files.push(String::from_utf8_lossy(&entry.path).to_string());
            }
        }
        dirty_files.sort();
        dirty_files.dedup();

        Err(GitServiceError::WorktreeDirty(
            branch_name,
            dirty_files.join(", "),
        ))
    }

    /// Get current HEAD information including branch name and commit OID.
    pub fn get_head_info(&self, repo_path: &Path) -> Result<HeadInfo, GitServiceError> {
        let git = GitCli::new();

        let branch = git
            .git(repo_path, ["symbolic-ref", "--quiet", "--short", "HEAD"])
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "HEAD".to_string());

        let oid = self.get_head_oid(repo_path)?;

        Ok(HeadInfo { branch, oid })
    }

    /// Get current HEAD commit OID as a hex string.
    pub fn get_head_oid(&self, repo_path: &Path) -> Result<String, GitServiceError> {
        GitCli::new().rev_parse(repo_path, "HEAD").map_err(|_| {
            GitServiceError::InvalidRepository("Repository HEAD has no target commit".to_string())
        })
    }

    /// Best-effort HEAD OID resolution that avoids spawning `git` when possible.
    ///
    /// This resolves the HEAD OID by reading git metadata files in common layouts:
    /// - normal repos (`.git/HEAD` + `.git/refs/*`)
    /// - worktrees (`.git` file pointing at `gitdir`, with `commondir`)
    /// - packed refs (`packed-refs`)
    ///
    /// Falls back to `git rev-parse HEAD` when metadata parsing is unavailable.
    pub fn get_head_oid_fast(&self, repo_path: &Path) -> Result<String, GitServiceError> {
        match try_resolve_head_oid_via_git_files(repo_path) {
            Ok(Some(oid)) => Ok(oid),
            Ok(None) => self.get_head_oid(repo_path),
            Err(_) => self.get_head_oid(repo_path),
        }
    }

    /// Get the commit OID (as hex string) for a given ref without modifying HEAD.
    pub fn get_branch_oid(
        &self,
        repo_path: &Path,
        branch_name: &str,
    ) -> Result<String, GitServiceError> {
        GitCli::new()
            .rev_parse(repo_path, branch_name)
            .map_err(|_| GitServiceError::BranchNotFound(branch_name.to_string()))
    }

    /// Get the subject/summary line for a given commit OID
    pub fn get_commit_subject(
        &self,
        repo_path: &Path,
        commit_sha: &str,
    ) -> Result<String, GitServiceError> {
        GitCli::new()
            .show_format(repo_path, commit_sha, "%s")
            .map_err(|e| GitServiceError::InvalidRepository(format!("git show failed: {e}")))
    }

    /// Compare two OIDs and return (ahead, behind) counts: how many commits
    /// `from_oid` is ahead of and behind `to_oid`.
    pub fn ahead_behind_commits_by_oid(
        &self,
        repo_path: &Path,
        from_oid: &str,
        to_oid: &str,
    ) -> Result<(usize, usize), GitServiceError> {
        GitCli::new()
            .rev_list_left_right_count(repo_path, from_oid, to_oid)
            .map_err(GitServiceError::from)
    }

    /// Return (uncommitted_tracked_changes, untracked_files) counts in worktree
    pub fn get_worktree_change_counts(
        &self,
        worktree_path: &Path,
    ) -> Result<(usize, usize), GitServiceError> {
        let cli = GitCli::new();
        let st = cli
            .get_worktree_status(worktree_path)
            .map_err(|e| GitServiceError::InvalidRepository(format!("git status failed: {e}")))?;
        Ok((st.uncommitted_tracked, st.untracked))
    }

    /// Evaluate whether any action is needed to reset to `target_commit_oid` and
    /// optionally perform the actions.
    pub fn reconcile_worktree_to_commit(
        &self,
        worktree_path: &Path,
        target_commit_oid: &str,
        options: WorktreeResetOptions,
    ) -> WorktreeResetOutcome {
        let WorktreeResetOptions {
            perform_reset,
            force_when_dirty,
            is_dirty,
            log_skip_when_dirty,
        } = options;

        let head_oid = self.get_head_info(worktree_path).ok().map(|h| h.oid);
        let mut outcome = WorktreeResetOutcome::default();

        if head_oid.as_deref() != Some(target_commit_oid) || is_dirty {
            outcome.needed = true;

            if perform_reset {
                if is_dirty && !force_when_dirty {
                    if log_skip_when_dirty {
                        tracing::warn!("Worktree dirty; skipping reset as not forced");
                    }
                } else if let Err(e) = self.reset_worktree_to_commit(
                    worktree_path,
                    target_commit_oid,
                    force_when_dirty,
                ) {
                    tracing::error!("Failed to reset worktree: {}", e);
                } else {
                    outcome.applied = true;
                }
            }
        }

        outcome
    }

    /// Reset the given worktree to the specified commit SHA.
    /// If `force` is false and the worktree is dirty, returns WorktreeDirty error.
    pub fn reset_worktree_to_commit(
        &self,
        worktree_path: &Path,
        commit_sha: &str,
        force: bool,
    ) -> Result<(), GitServiceError> {
        if !force {
            self.check_worktree_clean(worktree_path)?;
        }
        let cli = GitCli::new();
        cli.git(worktree_path, ["reset", "--hard", commit_sha])
            .map_err(|e| {
                GitServiceError::InvalidRepository(format!("git reset --hard failed: {e}"))
            })?;
        // Reapply sparse-checkout if configured (non-fatal)
        let _ = cli.git(worktree_path, ["sparse-checkout", "reapply"]);
        Ok(())
    }

    /// Add a worktree for a branch, optionally creating the branch
    pub fn add_worktree(
        &self,
        repo_path: &Path,
        worktree_path: &Path,
        branch: &str,
    ) -> Result<(), GitServiceError> {
        let git = GitCli::new();
        git.worktree_add(repo_path, worktree_path, branch)
            .map_err(|e| GitServiceError::InvalidRepository(e.to_string()))?;
        Ok(())
    }

    /// Remove a worktree
    pub fn remove_worktree(
        &self,
        repo_path: &Path,
        worktree_path: &Path,
        force: bool,
    ) -> Result<(), GitServiceError> {
        let git = GitCli::new();
        git.worktree_remove(repo_path, worktree_path, force)
            .map_err(|e| GitServiceError::InvalidRepository(e.to_string()))?;
        Ok(())
    }

    /// Move a worktree to a new location
    pub fn move_worktree(
        &self,
        repo_path: &Path,
        old_path: &Path,
        new_path: &Path,
    ) -> Result<(), GitServiceError> {
        let git = GitCli::new();
        git.worktree_move(repo_path, old_path, new_path)
            .map_err(|e| GitServiceError::InvalidRepository(e.to_string()))?;
        Ok(())
    }

    pub fn prune_worktrees(&self, repo_path: &Path) -> Result<(), GitServiceError> {
        let git = GitCli::new();
        git.worktree_prune(repo_path)
            .map_err(|e| GitServiceError::InvalidRepository(e.to_string()))?;
        Ok(())
    }

    pub fn get_all_branches(&self, repo_path: &Path) -> Result<Vec<GitBranch>, GitServiceError> {
        let git = GitCli::new();
        let current_branch = git
            .git(repo_path, ["symbolic-ref", "--quiet", "--short", "HEAD"])
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_default();

        let out = git.for_each_ref(
            repo_path,
            &["refs/heads", "refs/remotes"],
            "%(refname)\t%(refname:short)\t%(committerdate:unix)",
        )?;

        let mut branches = Vec::new();
        for line in out.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut cols = line.splitn(3, '\t');
            let full_ref = cols.next().unwrap_or("");
            let short = cols.next().unwrap_or("");
            let ts = cols.next().unwrap_or("");

            if short.is_empty() {
                continue;
            }

            let is_remote = full_ref.starts_with("refs/remotes/");
            if is_remote && short.ends_with("/HEAD") {
                continue;
            }

            let seconds = ts.parse::<i64>().unwrap_or(0);
            let last_commit_date = DateTime::from_timestamp(seconds, 0).unwrap_or_else(Utc::now);

            branches.push(GitBranch {
                name: short.to_string(),
                is_current: !is_remote && short == current_branch,
                is_remote,
                last_commit_date,
            });
        }

        branches.sort_by(|a, b| {
            if a.is_current && !b.is_current {
                std::cmp::Ordering::Less
            } else if !a.is_current && b.is_current {
                std::cmp::Ordering::Greater
            } else {
                b.last_commit_date.cmp(&a.last_commit_date)
            }
        });

        Ok(branches)
    }

    /// Rebase a worktree branch onto a new base
    pub fn rebase_branch(
        &self,
        repo_path: &Path,
        worktree_path: &Path,
        new_base_branch: &str,
        old_base_branch: &str,
        task_branch: &str,
    ) -> Result<String, GitServiceError> {
        // Safety guard: never operate on a dirty worktree (tracked changes).
        // Untracked files are allowed.
        self.check_worktree_clean(worktree_path)?;

        let git = GitCli::new();
        if git.is_rebase_in_progress(worktree_path).unwrap_or(false) {
            return Err(GitServiceError::RebaseInProgress);
        }

        if matches!(
            self.find_branch_type(repo_path, new_base_branch),
            Ok(GitBranchType::Remote)
        ) {
            let remote = new_base_branch
                .split('/')
                .next()
                .unwrap_or("origin")
                .to_string();
            let refspec = format!("+refs/heads/*:refs/remotes/{remote}/*");
            let _ = git.fetch_with_refspec(repo_path, &remote, &refspec);
        }

        match git.rebase_onto(worktree_path, new_base_branch, old_base_branch, task_branch) {
            Ok(()) => {}
            Err(GitCliError::RebaseInProgress) => return Err(GitServiceError::RebaseInProgress),
            Err(GitCliError::CommandFailed(stderr)) => {
                let looks_like_conflict = stderr.contains("could not apply")
                    || stderr.contains("CONFLICT")
                    || stderr.to_lowercase().contains("resolve all conflicts");
                if looks_like_conflict {
                    let attempt_branch = git
                        .git(worktree_path, ["rev-parse", "--abbrev-ref", "HEAD"])
                        .ok()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "(unknown)".to_string());
                    let conflicts = git.get_conflicted_files(worktree_path).unwrap_or_default();
                    let files_part = if conflicts.is_empty() {
                        "".to_string()
                    } else {
                        let mut sample = conflicts.clone();
                        let total = sample.len();
                        sample.truncate(10);
                        let list = sample.join(", ");
                        if total > sample.len() {
                            format!(
                                " Conflicted files (showing {} of {}): {}.",
                                sample.len(),
                                total,
                                list
                            )
                        } else {
                            format!(" Conflicted files: {list}.")
                        }
                    };
                    let msg = format!(
                        "Rebase encountered merge conflicts while rebasing '{attempt_branch}' onto '{new_base_branch}'.{files_part} Resolve conflicts and then continue or abort."
                    );
                    return Err(GitServiceError::MergeConflicts(msg));
                }
                return Err(GitServiceError::InvalidRepository(format!(
                    "Rebase failed: {}",
                    stderr.lines().next().unwrap_or("")
                )));
            }
            Err(e) => {
                return Err(GitServiceError::InvalidRepository(format!(
                    "git rebase failed: {e}"
                )));
            }
        }

        Ok(git.rev_parse(worktree_path, "HEAD").unwrap_or_default())
    }

    pub fn find_branch_type(
        &self,
        repo_path: &Path,
        branch_name: &str,
    ) -> Result<GitBranchType, GitServiceError> {
        if Self::ref_exists(repo_path, &format!("refs/heads/{branch_name}"))? {
            return Ok(GitBranchType::Local);
        }
        if Self::ref_exists(repo_path, &format!("refs/remotes/{branch_name}"))? {
            return Ok(GitBranchType::Remote);
        }
        Err(GitServiceError::BranchNotFound(branch_name.to_string()))
    }

    pub fn check_branch_exists(
        &self,
        repo_path: &Path,
        branch_name: &str,
    ) -> Result<bool, GitServiceError> {
        Ok(
            Self::ref_exists(repo_path, &format!("refs/heads/{branch_name}"))?
                || Self::ref_exists(repo_path, &format!("refs/remotes/{branch_name}"))?,
        )
    }

    fn ref_exists(repo_path: &Path, refname: &str) -> Result<bool, GitServiceError> {
        let git = GitCli::new();
        match git.git(repo_path, ["show-ref", "--verify", "--quiet", refname]) {
            Ok(_) => Ok(true),
            Err(GitCliError::CommandFailed(_)) => Ok(false),
            Err(e) => Err(GitServiceError::InvalidRepository(format!(
                "git show-ref failed: {e}"
            ))),
        }
    }

    pub fn check_remote_branch_exists(
        &self,
        repo_path: &Path,
        branch_name: &str,
    ) -> Result<bool, GitServiceError> {
        let git = GitCli::new();
        let default_remote_name = self.default_remote_name_checked(repo_path)?;
        let stripped_branch_name = branch_name
            .strip_prefix(&format!("{default_remote_name}/"))
            .unwrap_or(branch_name);
        git.check_remote_branch_exists(repo_path, &default_remote_name, stripped_branch_name)
            .map_err(GitServiceError::from)
    }

    pub fn rename_local_branch(
        &self,
        worktree_path: &Path,
        old_branch_name: &str,
        new_branch_name: &str,
    ) -> Result<(), GitServiceError> {
        let new_branch_name = new_branch_name.trim();
        if new_branch_name.is_empty() || !self.is_branch_name_valid(new_branch_name) {
            return Err(GitServiceError::InvalidBranchName(
                new_branch_name.to_string(),
            ));
        }
        let git = GitCli::new();
        git.git(
            worktree_path,
            ["branch", "-m", old_branch_name, new_branch_name],
        )
        .map_err(|e| GitServiceError::InvalidRepository(format!("git branch -m failed: {e}")))?;
        Ok(())
    }

    /// Return true if a rebase is currently in progress in this worktree.
    pub fn is_rebase_in_progress(&self, worktree_path: &Path) -> Result<bool, GitServiceError> {
        let git = GitCli::new();
        git.is_rebase_in_progress(worktree_path).map_err(|e| {
            GitServiceError::InvalidRepository(format!("git rebase state check failed: {e}"))
        })
    }

    pub fn detect_conflict_op(
        &self,
        worktree_path: &Path,
    ) -> Result<Option<ConflictOp>, GitServiceError> {
        let git = GitCli::new();
        if git.is_rebase_in_progress(worktree_path).unwrap_or(false) {
            return Ok(Some(ConflictOp::Rebase));
        }
        if git.is_merge_in_progress(worktree_path).unwrap_or(false) {
            return Ok(Some(ConflictOp::Merge));
        }
        if git
            .is_cherry_pick_in_progress(worktree_path)
            .unwrap_or(false)
        {
            return Ok(Some(ConflictOp::CherryPick));
        }
        if git.is_revert_in_progress(worktree_path).unwrap_or(false) {
            return Ok(Some(ConflictOp::Revert));
        }
        Ok(None)
    }

    /// List conflicted (unmerged) files in the worktree.
    pub fn get_conflicted_files(
        &self,
        worktree_path: &Path,
    ) -> Result<Vec<String>, GitServiceError> {
        let git = GitCli::new();
        git.get_conflicted_files(worktree_path).map_err(|e| {
            GitServiceError::InvalidRepository(format!("git diff for conflicts failed: {e}"))
        })
    }

    /// Abort an in-progress rebase in this worktree (no-op if none).
    pub fn abort_rebase(&self, worktree_path: &Path) -> Result<(), GitServiceError> {
        let git = GitCli::new();
        git.abort_rebase(worktree_path).map_err(|e| {
            GitServiceError::InvalidRepository(format!("git rebase --abort failed: {e}"))
        })
    }

    pub fn abort_conflicts(&self, worktree_path: &Path) -> Result<(), GitServiceError> {
        let git = GitCli::new();
        if git.is_rebase_in_progress(worktree_path).unwrap_or(false) {
            // If there are no conflicted files, prefer `git rebase --quit` to clean up metadata
            let has_conflicts = !self
                .get_conflicted_files(worktree_path)
                .unwrap_or_default()
                .is_empty();
            if has_conflicts {
                return self.abort_rebase(worktree_path);
            } else {
                return git.quit_rebase(worktree_path).map_err(|e| {
                    GitServiceError::InvalidRepository(format!("git rebase --quit failed: {e}"))
                });
            }
        }
        if git.is_merge_in_progress(worktree_path).unwrap_or(false) {
            return git.abort_merge(worktree_path).map_err(|e| {
                GitServiceError::InvalidRepository(format!("git merge --abort failed: {e}"))
            });
        }
        if git
            .is_cherry_pick_in_progress(worktree_path)
            .unwrap_or(false)
        {
            return git.abort_cherry_pick(worktree_path).map_err(|e| {
                GitServiceError::InvalidRepository(format!("git cherry-pick --abort failed: {e}"))
            });
        }
        if git.is_revert_in_progress(worktree_path).unwrap_or(false) {
            return git.abort_revert(worktree_path).map_err(|e| {
                GitServiceError::InvalidRepository(format!("git revert --abort failed: {e}"))
            });
        }
        Ok(())
    }

    /// Extract GitHub owner and repo name from git repo path
    pub fn get_github_repo_info(
        &self,
        repo_path: &Path,
    ) -> Result<GitHubRepoInfo, GitServiceError> {
        let remote_name = self.default_remote_name_checked(repo_path)?;
        let url = self.remote_url(repo_path, &remote_name)?;
        GitHubRepoInfo::from_remote_url(&url).map_err(|e| {
            GitServiceError::InvalidRepository(format!("Failed to parse remote URL: {e}"))
        })
    }

    pub fn get_remote_name_from_branch_name(
        &self,
        repo_path: &Path,
        branch_name: &str,
    ) -> Result<String, GitServiceError> {
        match self.find_branch_type(repo_path, branch_name) {
            Ok(GitBranchType::Remote) => Ok(branch_name
                .split('/')
                .next()
                .unwrap_or("origin")
                .to_string()),
            Ok(GitBranchType::Local) => {
                let git = GitCli::new();
                let key = format!("branch.{branch_name}.remote");
                let out = git
                    .git(repo_path, ["config", "--get", &key])
                    .unwrap_or_default();
                let out = out.trim();
                if out.is_empty() {
                    Ok(self.default_remote_name(repo_path))
                } else {
                    Ok(out.to_string())
                }
            }
            Err(e) => Err(e),
        }
    }

    pub fn push_to_github(
        &self,
        worktree_path: &Path,
        branch_name: &str,
        force: bool,
    ) -> Result<(), GitServiceError> {
        self.check_worktree_clean(worktree_path)?;

        let remote_name = self.default_remote_name_checked(worktree_path)?;
        let git = GitCli::new();
        git.push(worktree_path, &remote_name, branch_name, force)?;

        // Best-effort: refresh the remote tracking ref for subsequent comparisons.
        let refspec = format!("+refs/heads/{branch_name}:refs/remotes/{remote_name}/{branch_name}");
        let _ = git.fetch_with_refspec(worktree_path, &remote_name, &refspec);

        Ok(())
    }

    /// Clone a repository to the specified directory
    #[cfg(feature = "cloud")]
    pub fn clone_repository(
        clone_url: &str,
        target_path: &Path,
        token: Option<&str>,
    ) -> Result<(), GitServiceError> {
        // NOTE: This intentionally prefers system git authentication (credential helper / SSH agent).
        // If a token is provided, use a temporary GIT_ASKPASS helper to avoid embedding it in URLs.
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let git_path = utils_core::shell::resolve_executable_path_blocking("git")
            .ok_or(GitCliError::NotAvailable)?;

        let mut cmd = std::process::Command::new(&git_path);
        cmd.arg("clone")
            .arg(clone_url)
            .arg(target_path)
            .env("GIT_TERMINAL_PROMPT", "0");

        let _askpass_dir;
        if let Some(token) = token {
            let dir = tempfile::TempDir::new().map_err(|e| {
                GitServiceError::InvalidRepository(format!("temp dir create failed: {e}"))
            })?;
            let askpass_path = dir.path().join(if cfg!(windows) {
                "askpass.bat"
            } else {
                "askpass.sh"
            });

            if cfg!(windows) {
                std::fs::write(
                    &askpass_path,
                    format!(
                        "@echo off\r\nset PROMPT=%1\r\nif not \"%%PROMPT:Username=%%\"==\"%%PROMPT%%\" (echo git& exit /b 0)\r\nif not \"%%PROMPT:Password=%%\"==\"%%PROMPT%%\" (echo {token}& exit /b 0)\r\necho.\r\n"
                    ),
                )?;
            } else {
                std::fs::write(
                    &askpass_path,
                    format!(
                        "#!/bin/sh\ncase \"$1\" in\n  *Username*) echo git ;;\n  *Password*) echo '{token}' ;;\n  *) echo '' ;;\nesac\n"
                    ),
                )?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = std::fs::metadata(&askpass_path)?.permissions();
                    perms.set_mode(0o700);
                    std::fs::set_permissions(&askpass_path, perms)?;
                }
            }

            cmd.env("GIT_ASKPASS", &askpass_path);
            _askpass_dir = dir;
        }

        let out = cmd
            .output()
            .map_err(|e| GitServiceError::InvalidRepository(format!("git clone failed: {e}")))?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let combined = match (stdout.is_empty(), stderr.is_empty()) {
                (true, true) => "Command failed with no output".to_string(),
                (false, false) => format!("--- stderr\n{stderr}\n--- stdout\n{stdout}"),
                (false, true) => format!("--- stderr\n{stdout}"),
                (true, false) => format!("--- stdout\n{stderr}"),
            };
            return Err(GitServiceError::InvalidRepository(format!(
                "git clone failed: {combined}"
            )));
        }

        Ok(())
    }

    /// Collect file statistics from recent commits for ranking purposes
    pub fn collect_recent_file_stats(
        &self,
        repo_path: &Path,
        commit_limit: usize,
    ) -> Result<HashMap<String, FileStat>, GitServiceError> {
        let git = GitCli::new();
        let out = git
            .git(
                repo_path,
                [
                    "--no-optional-locks",
                    "log",
                    "-z",
                    "--name-only",
                    "--pretty=format:COMMIT:%ct%x00",
                    "-n",
                    &commit_limit.to_string(),
                ],
            )
            .map_err(|e| GitServiceError::InvalidRepository(format!("git log failed: {e}")))?;

        let mut stats: HashMap<String, FileStat> = HashMap::new();
        let mut commit_index: usize = 0;
        let mut saw_commit = false;
        let mut current_time: Option<DateTime<Utc>> = None;

        for part in out.split('\0') {
            if part.is_empty() {
                continue;
            }
            if let Some(ts) = part.strip_prefix("COMMIT:") {
                if saw_commit {
                    commit_index = commit_index.saturating_add(1);
                } else {
                    saw_commit = true;
                }
                let ts = ts.trim();
                let seconds = ts.parse::<i64>().unwrap_or(0);
                current_time = Some(DateTime::from_timestamp(seconds, 0).unwrap_or_else(Utc::now));
                continue;
            }

            let Some(time) = current_time else {
                continue;
            };

            let path = part.trim_start_matches('\n').trim();
            if path.is_empty() {
                continue;
            }

            let stat = stats.entry(path.to_string()).or_insert(FileStat {
                last_index: commit_index,
                commit_count: 0,
                last_time: time,
            });

            stat.commit_count = stat.commit_count.saturating_add(1);
            if commit_index < stat.last_index {
                stat.last_index = commit_index;
                stat.last_time = time;
            }
        }

        // NOTE: We intentionally do not attempt to resolve renames here; we rank by current paths.
        Ok(stats)
    }
}

fn try_resolve_head_oid_via_git_files(repo_path: &Path) -> Result<Option<String>, std::io::Error> {
    fn is_hex_oid(value: &str) -> bool {
        value.len() == 40 && value.as_bytes().iter().all(|b| b.is_ascii_hexdigit())
    }

    fn read_trimmed(path: &Path) -> Result<String, std::io::Error> {
        Ok(std::fs::read_to_string(path)?.trim().to_string())
    }

    fn resolve_git_dir(repo_path: &Path) -> Result<Option<PathBuf>, std::io::Error> {
        let dot_git = repo_path.join(".git");
        let meta = match std::fs::metadata(&dot_git) {
            Ok(meta) => meta,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };

        if meta.is_dir() {
            return Ok(Some(dot_git));
        }

        if !meta.is_file() {
            return Ok(None);
        }

        let contents = read_trimmed(&dot_git)?;
        let gitdir = contents
            .lines()
            .find_map(|line| line.trim().strip_prefix("gitdir:"))
            .map(str::trim)
            .filter(|s| !s.is_empty());

        let Some(gitdir) = gitdir else {
            return Ok(None);
        };

        let gitdir_path = PathBuf::from(gitdir);
        if gitdir_path.is_absolute() {
            Ok(Some(gitdir_path))
        } else {
            Ok(Some(repo_path.join(gitdir_path)))
        }
    }

    fn resolve_common_dir(git_dir: &Path) -> Result<PathBuf, std::io::Error> {
        let commondir_path = git_dir.join("commondir");
        let commondir = match read_trimmed(&commondir_path) {
            Ok(commondir) => commondir,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(git_dir.to_path_buf());
            }
            Err(err) => return Err(err),
        };

        let commondir_path = PathBuf::from(commondir);
        if commondir_path.is_absolute() {
            Ok(commondir_path)
        } else {
            Ok(git_dir.join(commondir_path))
        }
    }

    fn find_packed_ref_oid(
        packed_refs_path: &Path,
        ref_name: &str,
    ) -> Result<Option<String>, std::io::Error> {
        use std::io::BufRead as _;

        let file = match std::fs::File::open(packed_refs_path) {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };

        let reader = std::io::BufReader::new(file);
        for line in reader.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty()
                || line.starts_with('#')
                || line.starts_with('^')
                || line.starts_with("@@")
            {
                continue;
            }

            let Some((oid, name)) = line.split_once(' ') else {
                continue;
            };
            if name == ref_name && is_hex_oid(oid) {
                return Ok(Some(oid.to_string()));
            }
        }
        Ok(None)
    }

    let Some(git_dir) = resolve_git_dir(repo_path)? else {
        return Ok(None);
    };
    let common_dir = resolve_common_dir(&git_dir)?;

    let head_path = git_dir.join("HEAD");
    let head = match read_trimmed(&head_path) {
        Ok(head) => head,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };

    if let Some(rest) = head.strip_prefix("ref:") {
        let ref_name = rest.trim();
        if ref_name.is_empty() {
            return Ok(None);
        }

        let ref_path = common_dir.join(ref_name);
        match read_trimmed(&ref_path) {
            Ok(oid) if is_hex_oid(&oid) => return Ok(Some(oid)),
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }

        return find_packed_ref_oid(&common_dir.join("packed-refs"), ref_name);
    }

    if is_hex_oid(&head) {
        Ok(Some(head))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::Path,
        process::Command,
        sync::{Mutex, OnceLock},
    };

    use tempfile::TempDir;

    use super::*;

    fn git_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn git(workdir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(workdir)
            .args(args)
            .output()
            .expect("git command should spawn");
        if !output.status.success() {
            panic!(
                "git {:?} failed (code={:?})\nstdout:\n{}\nstderr:\n{}",
                args,
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    fn git_config_identity(repo: &Path) {
        git(repo, &["config", "user.email", "vk-test@example.com"]);
        git(repo, &["config", "user.name", "vk-test"]);
    }

    fn commit_and_push(repo: &Path, msg: &str) {
        std::fs::write(repo.join("file.txt"), format!("{msg}\n")).expect("write file");
        git(repo, &["add", "file.txt"]);
        git(repo, &["commit", "-m", msg]);
        git(repo, &["push"]);
    }

    #[test]
    fn remote_branch_status_fetch_is_ttl_gated() {
        let _guard = git_test_lock();
        remote_status_fetch_test_support::reset_state();
        remote_status_fetch_test_support::set_ttl_override(3600);

        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let remote = root.join("remote.git");
        let local = root.join("local");
        let other = root.join("other");

        git(root, &["init", "--bare", remote.to_str().unwrap()]);

        git(
            root,
            &["clone", remote.to_str().unwrap(), local.to_str().unwrap()],
        );
        git_config_identity(&local);
        git(&local, &["checkout", "-b", "main"]);
        std::fs::write(local.join("file.txt"), "init\n").expect("write file");
        git(&local, &["add", "file.txt"]);
        git(&local, &["commit", "-m", "init"]);
        git(&local, &["push", "-u", "origin", "main"]);

        git(
            root,
            &[
                "clone",
                "--branch",
                "main",
                remote.to_str().unwrap(),
                other.to_str().unwrap(),
            ],
        );
        git_config_identity(&other);
        commit_and_push(&other, "remote-1");

        let service = GitService::new();
        let (ahead, behind) = service
            .get_remote_branch_status(&local, "main", None)
            .expect("remote status should succeed");
        assert_eq!((ahead, behind), (0, 1));
        assert_eq!(
            remote_status_fetch_test_support::REMOTE_STATUS_FETCH_ATTEMPTS
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );

        commit_and_push(&other, "remote-2");

        // TTL-gated: should not refetch, therefore stays stale (behind=1).
        let (ahead, behind) = service
            .get_remote_branch_status(&local, "main", None)
            .expect("remote status should still succeed");
        assert_eq!((ahead, behind), (0, 1));
        assert_eq!(
            remote_status_fetch_test_support::REMOTE_STATUS_FETCH_ATTEMPTS
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );

        // Clearing state forces a refresh, which observes the second remote commit.
        remote_status_fetch_test_support::reset_state();
        remote_status_fetch_test_support::set_ttl_override(3600);
        let (ahead, behind) = service
            .get_remote_branch_status(&local, "main", None)
            .expect("remote status should succeed after refresh");
        assert_eq!((ahead, behind), (0, 2));
        assert_eq!(
            remote_status_fetch_test_support::REMOTE_STATUS_FETCH_ATTEMPTS
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    #[test]
    fn remote_branch_status_supports_slash_branch_names() {
        let _guard = git_test_lock();
        remote_status_fetch_test_support::reset_state();
        remote_status_fetch_test_support::set_ttl_override(3600);

        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let remote = root.join("remote.git");
        let local = root.join("local");
        let other = root.join("other");

        git(root, &["init", "--bare", remote.to_str().unwrap()]);
        git(
            root,
            &["clone", remote.to_str().unwrap(), local.to_str().unwrap()],
        );
        git_config_identity(&local);

        git(&local, &["checkout", "-b", "main"]);
        std::fs::write(local.join("file.txt"), "init\n").expect("write file");
        git(&local, &["add", "file.txt"]);
        git(&local, &["commit", "-m", "init"]);
        git(&local, &["push", "-u", "origin", "main"]);

        git(&local, &["checkout", "-b", "feature/test"]);
        std::fs::write(local.join("file.txt"), "feature\n").expect("write file");
        git(&local, &["add", "file.txt"]);
        git(&local, &["commit", "-m", "feature-init"]);
        git(&local, &["push", "-u", "origin", "feature/test"]);

        git(
            root,
            &[
                "clone",
                "--branch",
                "feature/test",
                remote.to_str().unwrap(),
                other.to_str().unwrap(),
            ],
        );
        git_config_identity(&other);
        commit_and_push(&other, "remote-1");

        let service = GitService::new();
        let (ahead, behind) = service
            .get_remote_branch_status(&local, "feature/test", None)
            .expect("remote status should succeed for slash branch");
        assert_eq!((ahead, behind), (0, 1));
        assert_eq!(
            remote_status_fetch_test_support::REMOTE_STATUS_FETCH_ATTEMPTS
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    #[test]
    fn worktree_diff_plan_stages_once_and_materializes_without_restaging() {
        let _guard = git_test_lock();
        super::cli::worktree_diff_test_support::reset_prepare_count();

        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let repo = root.join("repo");

        git(root, &["init", repo.to_str().unwrap()]);
        git_config_identity(&repo);
        git(&repo, &["checkout", "-b", "main"]);
        std::fs::write(repo.join("a.txt"), "a\n").expect("write file");
        git(&repo, &["add", "a.txt"]);
        git(&repo, &["commit", "-m", "init"]);

        let base_oid = git(&repo, &["rev-parse", "HEAD"]);
        let base_commit = Commit::new(base_oid.trim());

        std::fs::write(repo.join("a.txt"), "b\n").expect("write file");
        std::fs::write(repo.join("b.txt"), "new\n").expect("write file");

        let service = GitService::new();
        let plan = service
            .get_worktree_diff_plan(&repo, &base_commit, None)
            .expect("plan should succeed");
        assert!(plan.stats_error().is_none());
        assert_eq!(super::cli::worktree_diff_test_support::prepare_count(), 1);

        let _diffs = plan.diffs(DiffContentPolicy::Full, None);
        assert_eq!(super::cli::worktree_diff_test_support::prepare_count(), 1);
    }

    #[test]
    fn remote_branch_status_without_upstream_returns_error() {
        let _guard = git_test_lock();
        remote_status_fetch_test_support::reset_state();
        remote_status_fetch_test_support::set_ttl_override(3600);

        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let repo = root.join("repo");
        std::fs::create_dir_all(&repo).expect("create repo dir");

        git(root, &["init", repo.to_str().unwrap()]);
        git_config_identity(&repo);
        git(&repo, &["checkout", "-b", "main"]);
        std::fs::write(repo.join("file.txt"), "init\n").expect("write file");
        git(&repo, &["add", "file.txt"]);
        git(&repo, &["commit", "-m", "init"]);

        let service = GitService::new();
        let err = service
            .get_remote_branch_status(&repo, "main", None)
            .expect_err("missing upstream should be an error");
        assert!(
            matches!(
                err,
                GitServiceError::InvalidRepository(ref msg) if msg.contains("no upstream")
            ),
            "unexpected error: {err:?}"
        );
        assert_eq!(
            remote_status_fetch_test_support::REMOTE_STATUS_FETCH_ATTEMPTS
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }

    #[test]
    fn head_oid_fast_matches_git_rev_parse_in_normal_repo() {
        let _guard = git_test_lock();

        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let repo = root.join("repo");

        git(root, &["init", repo.to_str().unwrap()]);
        git_config_identity(&repo);
        git(&repo, &["checkout", "-b", "main"]);
        std::fs::write(repo.join("file.txt"), "init\n").expect("write file");
        git(&repo, &["add", "file.txt"]);
        git(&repo, &["commit", "-m", "init"]);

        let service = GitService::new();
        let fast = service.get_head_oid_fast(&repo).expect("fast head oid");
        let cli = git(&repo, &["rev-parse", "HEAD"]);
        assert_eq!(fast, cli.trim());
    }

    #[test]
    fn head_oid_fast_resolves_in_worktree_layout() {
        let _guard = git_test_lock();

        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let repo = root.join("repo");
        let wt = root.join("worktree");

        git(root, &["init", repo.to_str().unwrap()]);
        git_config_identity(&repo);
        git(&repo, &["checkout", "-b", "main"]);
        std::fs::write(repo.join("file.txt"), "init\n").expect("write file");
        git(&repo, &["add", "file.txt"]);
        git(&repo, &["commit", "-m", "init"]);

        git(
            &repo,
            &["worktree", "add", wt.to_str().unwrap(), "-b", "wt-branch"],
        );

        let service = GitService::new();
        let fast = service.get_head_oid_fast(&wt).expect("fast head oid");
        let cli = git(&wt, &["rev-parse", "HEAD"]);
        assert_eq!(fast, cli.trim());
    }

    #[test]
    fn head_oid_fast_resolves_packed_refs_when_loose_ref_is_missing() {
        let _guard = git_test_lock();

        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let repo = root.join("repo");

        git(root, &["init", repo.to_str().unwrap()]);
        git_config_identity(&repo);
        git(&repo, &["checkout", "-b", "main"]);
        std::fs::write(repo.join("file.txt"), "init\n").expect("write file");
        git(&repo, &["add", "file.txt"]);
        git(&repo, &["commit", "-m", "init"]);

        git(&repo, &["pack-refs", "--all", "--prune"]);
        let loose_ref = repo.join(".git/refs/heads/main");
        if loose_ref.exists() {
            std::fs::remove_file(&loose_ref).expect("remove loose ref");
        }

        let service = GitService::new();
        let fast = service.get_head_oid_fast(&repo).expect("fast head oid");
        let cli = git(&repo, &["rev-parse", "HEAD"]);
        assert_eq!(fast, cli.trim());
    }

    #[test]
    fn head_oid_fast_resolves_detached_head() {
        let _guard = git_test_lock();

        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let repo = root.join("repo");

        git(root, &["init", repo.to_str().unwrap()]);
        git_config_identity(&repo);
        git(&repo, &["checkout", "-b", "main"]);
        std::fs::write(repo.join("file.txt"), "init\n").expect("write file");
        git(&repo, &["add", "file.txt"]);
        git(&repo, &["commit", "-m", "init"]);

        git(&repo, &["checkout", "--detach"]);

        let service = GitService::new();
        let fast = service.get_head_oid_fast(&repo).expect("fast head oid");
        let cli = git(&repo, &["rev-parse", "HEAD"]);
        assert_eq!(fast, cli.trim());
    }
}
