use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use repos::{
    GitHubRepoInfo, GitHubRepoInfoError,
    git::{DiffContentPolicy, DiffTarget, GitCli, GitService},
};
use tempfile::TempDir;
use utils_core::diff::DiffChangeKind;

mod git_test_utils;
use git_test_utils::{
    git_branch_force, git_checkout, git_config_user, git_detach_head, git_rev_parse,
    git_show_author,
};

fn add_path(repo_path: &Path, path: &str) {
    let git = GitCli::new();
    git.git(repo_path, ["add", path]).unwrap();
}

fn get_commit_author(repo_path: &Path, commit_sha: &str) -> (Option<String>, Option<String>) {
    git_show_author(repo_path, commit_sha)
}

fn get_head_author(repo_path: &Path) -> (Option<String>, Option<String>) {
    let head = git_rev_parse(repo_path, "HEAD");
    git_show_author(repo_path, &head)
}

fn write_file<P: AsRef<Path>>(base: P, rel: &str, content: &str) {
    let path = base.as_ref().join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut f = fs::File::create(&path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
}

fn configure_user(repo_path: &Path, name: &str, email: &str) {
    git_config_user(repo_path, name, email);
}

fn init_repo_main(root: &TempDir) -> PathBuf {
    let path = root.path().join("repo");
    let s = GitService::new();
    s.initialize_repo_with_main_branch(&path).unwrap();
    configure_user(&path, "Test User", "test@example.com");
    checkout_branch(&path, "main");
    path
}

fn checkout_branch(repo_path: &Path, name: &str) {
    git_checkout(repo_path, name);
}

fn create_branch(repo_path: &Path, name: &str) {
    git_branch_force(repo_path, name, "HEAD");
}

#[test]
fn commit_empty_message_behaviour() {
    let td = TempDir::new().unwrap();
    let repo_path = init_repo_main(&td);
    write_file(&repo_path, "x.txt", "x\n");
    let s = GitService::new();
    let res = s.commit(&repo_path, "");
    // Some environments disallow empty commit messages by default.
    // Accept either success or a clear error.
    if let Err(e) = &res {
        let msg = format!("{e}");
        assert!(msg.contains("empty commit message") || msg.contains("git commit failed"));
    }
}

fn has_global_git_identity() -> bool {
    let name_ok = std::process::Command::new("git")
        .args(["config", "--global", "--get", "user.name"])
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false);
    let email_ok = std::process::Command::new("git")
        .args(["config", "--global", "--get", "user.email"])
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false);
    name_ok && email_ok
}

#[test]
fn initialize_repo_without_user_creates_initial_commit() {
    let td = TempDir::new().unwrap();
    let repo_path = td.path().join("repo_no_user_init");
    let s = GitService::new();
    // No configure_user call; rely on fallback signature for initial commit
    s.initialize_repo_with_main_branch(&repo_path).unwrap();
    let head = s.get_head_info(&repo_path).unwrap();
    assert_eq!(head.branch, "main");
    assert!(!head.oid.is_empty());
    // Verify author is set: either global identity (if configured) or fallback
    let (name, email) = get_head_author(&repo_path);
    if has_global_git_identity() {
        assert!(name.is_some() && email.is_some());
    } else {
        assert_eq!(name.as_deref(), Some("Vibe Kanban"));
        assert_eq!(email.as_deref(), Some("noreply@localhost"));
    }
}

#[test]
fn commit_without_user_config_succeeds() {
    let td = TempDir::new().unwrap();
    let repo_path = td.path().join("repo_no_user");
    let s = GitService::new();
    s.initialize_repo_with_main_branch(&repo_path).unwrap();
    write_file(&repo_path, "f.txt", "x\n");
    // No configure_user call here
    let res = s.commit(&repo_path, "no user config");
    assert!(res.is_ok());
}

#[test]
fn commit_fails_when_index_locked() {
    use std::fs::File;
    let td = TempDir::new().unwrap();
    let repo_path = init_repo_main(&td);
    write_file(&repo_path, "y.txt", "y\n");
    // Simulate index lock
    let git_dir = repo_path.join(".git");
    let _lock = File::create(git_dir.join("index.lock")).unwrap();
    let s = GitService::new();
    let res = s.commit(&repo_path, "should fail");
    assert!(res.is_err());
}

#[test]
fn staged_but_uncommitted_changes_is_dirty() {
    let td = TempDir::new().unwrap();
    let repo_path = init_repo_main(&td);
    let s = GitService::new();
    // seed tracked file
    write_file(&repo_path, "t1.txt", "a\n");
    let _ = s.commit(&repo_path, "seed").unwrap();
    // modify and stage
    write_file(&repo_path, "t1.txt", "b\n");
    add_path(&repo_path, "t1.txt");
    assert!(!s.is_worktree_clean(&repo_path).unwrap());
}

#[test]
fn worktree_clean_detects_staged_deleted_and_renamed() {
    let td = TempDir::new().unwrap();
    let repo_path = init_repo_main(&td);
    write_file(&repo_path, "t1.txt", "1\n");
    write_file(&repo_path, "t2.txt", "2\n");
    let s = GitService::new();
    let _ = s.commit(&repo_path, "seed").unwrap();

    // delete tracked file
    std::fs::remove_file(repo_path.join("t2.txt")).unwrap();
    assert!(!s.is_worktree_clean(&repo_path).unwrap());

    // restore and test rename
    write_file(&repo_path, "t2.txt", "2\n");
    let _ = s.commit(&repo_path, "restore t2").unwrap();
    std::fs::rename(repo_path.join("t2.txt"), repo_path.join("t2-renamed.txt")).unwrap();
    assert!(!s.is_worktree_clean(&repo_path).unwrap());
}

#[test]
fn diff_added_binary_file_has_no_content() {
    // ensure binary file content is not loaded (null byte guard)
    let td = TempDir::new().unwrap();
    let repo_path = init_repo_main(&td);
    // base
    let s = GitService::new();
    let _ = s.commit(&repo_path, "base").unwrap();
    // branch with binary file
    create_branch(&repo_path, "feature");
    checkout_branch(&repo_path, "feature");
    // write binary with null byte
    let mut f = fs::File::create(repo_path.join("bin.dat")).unwrap();
    f.write_all(&[0u8, 1, 2, 3]).unwrap();
    let _ = s.commit(&repo_path, "add binary").unwrap();

    let s = GitService::new();
    let diffs = s
        .get_diffs(
            DiffTarget::Branch {
                repo_path: Path::new(&repo_path),
                branch_name: "feature",
                base_branch: "main",
            },
            None,
            DiffContentPolicy::Full,
        )
        .unwrap();
    let bin = diffs
        .iter()
        .find(|d| d.new_path.as_deref() == Some("bin.dat"))
        .expect("binary diff present");
    assert!(bin.new_content.is_none());
}

#[test]
fn initialize_and_default_branch_and_head_info() {
    let td = TempDir::new().unwrap();
    let repo_path = init_repo_main(&td);

    let s = GitService::new();
    // Head info branch should be main
    let head = s.get_head_info(&repo_path).unwrap();
    assert_eq!(head.branch, "main");

    // Repo has an initial commit (OID parsable)
    assert!(!head.oid.is_empty());
}

#[test]
fn commit_and_is_worktree_clean() {
    let td = TempDir::new().unwrap();
    let repo_path = init_repo_main(&td);
    write_file(&repo_path, "foo.txt", "hello\n");

    let s = GitService::new();
    let committed = s.commit(&repo_path, "add foo").unwrap();
    assert!(committed);
    assert!(s.is_worktree_clean(&repo_path).unwrap());

    // Verify commit contains file
    let diffs = s
        .get_diffs(
            DiffTarget::Commit {
                repo_path: Path::new(&repo_path),
                commit_sha: &s.get_head_info(&repo_path).unwrap().oid,
            },
            None,
            DiffContentPolicy::Full,
        )
        .unwrap();
    assert!(
        diffs
            .iter()
            .any(|d| d.new_path.as_deref() == Some("foo.txt"))
    );
}

#[test]
fn commit_in_detached_head_succeeds_via_service() {
    let td = TempDir::new().unwrap();
    let repo_path = init_repo_main(&td);
    // initial parent
    write_file(&repo_path, "a.txt", "a\n");
    let s = GitService::new();
    let _ = s.commit(&repo_path, "add a").unwrap();
    // detach via service
    let head = git_rev_parse(&repo_path, "HEAD");
    git_detach_head(&repo_path, &head);
    // commit while detached
    write_file(&repo_path, "b.txt", "b\n");
    let ok = s.commit(&repo_path, "detached commit").unwrap();
    assert!(ok);
}

#[test]
fn branch_status_ahead_and_behind() {
    let td = TempDir::new().unwrap();
    let repo_path = init_repo_main(&td);
    let s = GitService::new();

    // main: initial commit
    write_file(&repo_path, "base.txt", "base\n");
    let _ = s.commit(&repo_path, "base").unwrap();

    // create feature from main
    create_branch(&repo_path, "feature");
    // advance feature by 1
    checkout_branch(&repo_path, "feature");
    write_file(&repo_path, "feature.txt", "f1\n");
    let _ = s.commit(&repo_path, "f1").unwrap();

    // advance main by 1
    checkout_branch(&repo_path, "main");
    write_file(&repo_path, "main.txt", "m1\n");
    let _ = s.commit(&repo_path, "m1").unwrap();

    let s = GitService::new();
    let (ahead, behind) = s.get_branch_status(&repo_path, "feature", "main").unwrap();
    assert_eq!((ahead, behind), (1, 1));

    // advance feature by one more (ahead 2, behind 1)
    checkout_branch(&repo_path, "feature");
    write_file(&repo_path, "feature2.txt", "f2\n");
    let _ = s.commit(&repo_path, "f2").unwrap();
    let (ahead2, behind2) = s.get_branch_status(&repo_path, "feature", "main").unwrap();
    assert_eq!((ahead2, behind2), (2, 1));
}

#[test]
fn get_all_branches_lists_current_and_others() {
    let td = TempDir::new().unwrap();
    let repo_path = init_repo_main(&td);
    create_branch(&repo_path, "feature");

    let s = GitService::new();
    let branches = s.get_all_branches(&repo_path).unwrap();
    let names: Vec<_> = branches.iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"main"));
    assert!(names.contains(&"feature"));
    // current should be main
    let main_entry = branches.iter().find(|b| b.name == "main").unwrap();
    assert!(main_entry.is_current);
}

#[test]
fn get_branch_diffs_between_branches() {
    let td = TempDir::new().unwrap();
    let repo_path = init_repo_main(&td);
    let s = GitService::new();
    // base commit on main
    write_file(&repo_path, "a.txt", "a\n");
    let _ = s.commit(&repo_path, "add a").unwrap();

    // create branch and add new file
    create_branch(&repo_path, "feature");
    checkout_branch(&repo_path, "feature");
    write_file(&repo_path, "b.txt", "b\n");
    let _ = s.commit(&repo_path, "add b").unwrap();

    let s = GitService::new();
    let diffs = s
        .get_diffs(
            DiffTarget::Branch {
                repo_path: Path::new(&repo_path),
                branch_name: "feature",
                base_branch: "main",
            },
            None,
            DiffContentPolicy::Full,
        )
        .unwrap();
    assert!(diffs.iter().any(|d| d.new_path.as_deref() == Some("b.txt")));
}

#[test]
fn worktree_diff_respects_path_filter() {
    // Use git CLI status diff under the hood
    let td = TempDir::new().unwrap();
    let repo_path = init_repo_main(&td);

    // main baseline
    write_file(&repo_path, "src/keep.txt", "k\n");
    write_file(&repo_path, "other/skip.txt", "s\n");
    let s = GitService::new();
    let _ = s.commit(&repo_path, "baseline").unwrap();

    // create feature and work in place (worktree is repo_path)
    create_branch(&repo_path, "feature");

    // modify files without committing
    write_file(&repo_path, "src/only.txt", "only\n");
    write_file(&repo_path, "other/skip2.txt", "skip\n");

    let s = GitService::new();
    let base_commit = s.get_base_commit(&repo_path, "feature", "main").unwrap();
    let diffs = s
        .get_diffs(
            DiffTarget::Worktree {
                worktree_path: Path::new(&repo_path),
                base_commit: &base_commit,
            },
            Some(&["src"]),
            DiffContentPolicy::Full,
        )
        .unwrap();
    assert!(
        diffs
            .iter()
            .any(|d| d.new_path.as_deref() == Some("src/only.txt"))
    );
    assert!(
        !diffs
            .iter()
            .any(|d| d.new_path.as_deref() == Some("other/skip2.txt"))
    );
}

#[test]
fn worktree_diff_and_summary_include_untracked_files() {
    let td = TempDir::new().unwrap();
    let repo_path = init_repo_main(&td);

    // baseline
    write_file(&repo_path, "tracked.txt", "base\n");
    let s = GitService::new();
    let _ = s.commit(&repo_path, "baseline").unwrap();

    // branch ref exists for base-commit computation; worktree remains on main
    create_branch(&repo_path, "feature");

    // untracked change
    write_file(&repo_path, "untracked.txt", "l1\nl2\nl3\n");

    let base_commit = s.get_base_commit(&repo_path, "feature", "main").unwrap();
    let diffs = s
        .get_diffs(
            DiffTarget::Worktree {
                worktree_path: Path::new(&repo_path),
                base_commit: &base_commit,
            },
            None,
            DiffContentPolicy::Full,
        )
        .unwrap();

    let d = diffs
        .iter()
        .find(|d| d.new_path.as_deref() == Some("untracked.txt"))
        .expect("untracked diff present");
    assert_eq!(d.old_content, None);
    assert_eq!(d.new_content.as_deref(), Some("l1\nl2\nl3\n"));

    let summary = s
        .get_worktree_diff_summary(&repo_path, &base_commit, None)
        .unwrap();
    assert!(summary.file_count >= 1);
    assert!(summary.added >= 3);
}

#[test]
fn get_branch_oid_nonexistent_errors() {
    let td = TempDir::new().unwrap();
    let repo_path = init_repo_main(&td);
    let s = GitService::new();
    let res = s.get_branch_oid(&repo_path, "no-such-branch");
    assert!(res.is_err());
}

#[test]
fn create_unicode_branch_and_list() {
    let td = TempDir::new().unwrap();
    let repo_path = init_repo_main(&td);
    let s = GitService::new();
    // base commit
    write_file(&repo_path, "file.txt", "ok\n");
    let _ = s.commit(&repo_path, "base");
    // unicode/slash branch name (valid ref)
    let bname = "feature/ünicode";
    create_branch(&repo_path, bname);
    let names: Vec<_> = s
        .get_all_branches(&repo_path)
        .unwrap()
        .into_iter()
        .map(|b| b.name)
        .collect();
    assert!(names.iter().any(|n| n == bname));
}

#[cfg(unix)]
#[test]
fn worktree_diff_permission_only_change() {
    use std::os::unix::fs::PermissionsExt;
    let td = TempDir::new().unwrap();
    let repo_path = init_repo_main(&td);
    let s = GitService::new();
    // baseline commit
    write_file(&repo_path, "p.sh", "echo hi\n");
    let _ = s.commit(&repo_path, "add p.sh").unwrap();
    // create a feature branch baseline at HEAD
    create_branch(&repo_path, "feature");

    // change only the permission (chmod +x)
    let mut perms = std::fs::metadata(repo_path.join("p.sh"))
        .unwrap()
        .permissions();
    perms.set_mode(perms.mode() | 0o111);
    std::fs::set_permissions(repo_path.join("p.sh"), perms).unwrap();

    let base_commit = s.get_base_commit(&repo_path, "feature", "main").unwrap();
    // Compute worktree diff vs main on feature
    let diffs = s
        .get_diffs(
            DiffTarget::Worktree {
                worktree_path: Path::new(&repo_path),
                base_commit: &base_commit,
            },
            None,
            DiffContentPolicy::Full,
        )
        .unwrap();
    let d = diffs
        .into_iter()
        .find(|d| d.new_path.as_deref() == Some("p.sh"))
        .expect("p.sh diff present");
    assert!(matches!(d.change, DiffChangeKind::PermissionChange));
    assert_eq!(d.old_content, d.new_content);
}

#[test]
fn github_repo_info_parses_https_and_ssh_urls() {
    let info = GitHubRepoInfo::from_remote_url("https://github.com/owner/repo.git").unwrap();
    assert_eq!(info.owner, "owner");
    assert_eq!(info.repo_name, "repo");

    let info = GitHubRepoInfo::from_remote_url("git@github.com:owner/repo.git").unwrap();
    assert_eq!(info.owner, "owner");
    assert_eq!(info.repo_name, "repo");

    let info = GitHubRepoInfo::from_remote_url("https://github.com/owner/repo/pull/123").unwrap();
    assert_eq!(info.owner, "owner");
    assert_eq!(info.repo_name, "repo");

    let err = GitHubRepoInfo::from_remote_url("https://example.com/not/github").unwrap_err();
    let GitHubRepoInfoError::Repository(msg) = err;
    assert!(msg.contains("Invalid GitHub URL"));
}

#[test]
fn squash_merge_libgit2_sets_author_without_user() {
    // Verify merge_changes uses fallback author when no config exists
    let td = TempDir::new().unwrap();
    let repo_path = td.path().join("repo_fallback_merge");
    let worktree_path = td.path().join("wt_feature");
    let s = GitService::new();

    // Init repo without user config
    s.initialize_repo_with_main_branch(&repo_path).unwrap();

    // Create feature branch and worktree
    create_branch(&repo_path, "feature");
    s.add_worktree(&repo_path, &worktree_path, "feature")
        .unwrap();

    // Make a feature commit in the worktree using an explicit author identity
    write_file(&worktree_path, "f.txt", "feat\n");
    std::process::Command::new("git")
        .arg("-C")
        .arg(&worktree_path)
        .arg("add")
        .arg("-A")
        .status()
        .expect("git add")
        .success()
        .then_some(())
        .expect("git add ok");
    let mut cmd = std::process::Command::new("git");
    cmd.arg("-C").arg(&worktree_path);
    cmd.env("GIT_AUTHOR_NAME", "Other Author");
    cmd.env("GIT_AUTHOR_EMAIL", "other@example.com");
    cmd.env("GIT_COMMITTER_NAME", "Other Author");
    cmd.env("GIT_COMMITTER_EMAIL", "other@example.com");
    let out = cmd
        .args(["commit", "-m", "feat"])
        .output()
        .expect("git commit");
    assert!(
        out.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Ensure main repo is NOT on base branch so merge_changes takes libgit2 path
    create_branch(&repo_path, "dev");
    checkout_branch(&repo_path, "dev");

    // Merge feature -> main (libgit2 squash)
    let merge_sha = s
        .merge_changes(&repo_path, &worktree_path, "feature", "main", "squash")
        .unwrap();

    // The squash commit author should not be the feature commit's author, and must be present.
    let (name, email) = get_commit_author(&repo_path, &merge_sha);
    assert_ne!(name.as_deref(), Some("Other Author"));
    assert_ne!(email.as_deref(), Some("other@example.com"));
    if has_global_git_identity() {
        assert!(name.is_some() && email.is_some());
    } else {
        assert_eq!(name.as_deref(), Some("Vibe Kanban"));
        assert_eq!(email.as_deref(), Some("noreply@localhost"));
    }
}
