#![allow(dead_code)]

use std::{
    ffi::OsStr,
    path::Path,
    process::{Command, Output},
};

fn run_git<I, S>(repo_path: Option<&Path>, args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new("git");
    if let Some(repo_path) = repo_path {
        cmd.arg("-C").arg(repo_path);
    }
    cmd.args(args);
    cmd.output().expect("run git")
}

pub fn git_ok<I, S>(repo_path: &Path, args: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let out = run_git(Some(repo_path), args);
    if !out.status.success() {
        panic!(
            "git failed (code={:?})\n--- stderr\n{}\n--- stdout\n{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout)
        );
    }
    String::from_utf8_lossy(&out.stdout).trim_end().to_string()
}

pub fn git_ok_bytes<I, S>(repo_path: &Path, args: I) -> Vec<u8>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let out = run_git(Some(repo_path), args);
    if !out.status.success() {
        panic!(
            "git failed (code={:?})\n--- stderr\n{}\n--- stdout\n{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout)
        );
    }
    out.stdout
}

pub fn git_try<I, S>(repo_path: &Path, args: I) -> Result<String, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let out = run_git(Some(repo_path), args);
    if !out.status.success() {
        return Err(format!(
            "git failed (code={:?})\n--- stderr\n{}\n--- stdout\n{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).trim_end(),
            String::from_utf8_lossy(&out.stdout).trim_end()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

pub fn git_init_bare(path: &Path) {
    std::fs::create_dir_all(path).expect("mkdir bare repo dir");
    git_ok(path, ["init", "--bare"]);
}

pub fn git_clone(remote: &str, dest: &Path) {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).expect("mkdir clone parent");
    }
    let out = Command::new("git")
        .arg("clone")
        .arg(remote)
        .arg(dest)
        .output()
        .expect("run git clone");
    if !out.status.success() {
        panic!(
            "git clone failed (code={:?})\n--- stderr\n{}\n--- stdout\n{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout)
        );
    }
}

pub fn git_config(repo_path: &Path, key: &str, value: &str) {
    git_ok(repo_path, ["config", key, value]);
}

pub fn git_config_user(repo_path: &Path, name: &str, email: &str) {
    git_config(repo_path, "user.name", name);
    git_config(repo_path, "user.email", email);
}

pub fn git_checkout(repo_path: &Path, branch: &str) {
    git_ok(repo_path, ["checkout", branch]);
}

pub fn git_checkout_new(repo_path: &Path, branch: &str) {
    git_ok(repo_path, ["checkout", "-B", branch]);
}

pub fn git_detach_head(repo_path: &Path, sha: &str) {
    git_ok(repo_path, ["checkout", "--detach", sha]);
}

pub fn git_branch_force(repo_path: &Path, branch: &str, target: &str) {
    git_ok(repo_path, ["branch", "-f", branch, target]);
}

pub fn git_rev_parse(repo_path: &Path, rev: &str) -> String {
    git_ok(repo_path, ["rev-parse", "--verify", rev])
        .trim()
        .to_string()
}

pub fn git_show_author(repo_path: &Path, commit_sha: &str) -> (Option<String>, Option<String>) {
    // NUL-separated "name\0email"
    let bytes = git_ok_bytes(
        repo_path,
        [
            "show",
            "-s",
            "--format=%an%x00%ae",
            "--no-patch",
            commit_sha,
        ],
    );
    let mut parts = bytes.split(|b| *b == 0);
    let name = parts.next().and_then(|p| {
        (!p.is_empty()).then(|| {
            String::from_utf8_lossy(p)
                .trim_end_matches(['\n', '\r'])
                .to_string()
        })
    });
    let email = parts.next().and_then(|p| {
        (!p.is_empty()).then(|| {
            String::from_utf8_lossy(p)
                .trim_end_matches(['\n', '\r'])
                .to_string()
        })
    });
    (name, email)
}

pub fn git_commit_all(repo_path: &Path, message: &str) {
    git_ok(repo_path, ["add", "-A"]);
    let out = run_git(Some(repo_path), ["commit", "-m", message]);
    if out.status.success() {
        return;
    }

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{stderr}\n{stdout}");
    if combined.to_ascii_lowercase().contains("nothing to commit") {
        git_ok(repo_path, ["commit", "--allow-empty", "-m", message]);
        return;
    }

    panic!(
        "git commit failed (code={:?})\n--- stderr\n{}\n--- stdout\n{}",
        out.status.code(),
        stderr,
        stdout
    );
}

pub fn git_commit_allow_empty(repo_path: &Path, message: &str) {
    git_ok(repo_path, ["commit", "--allow-empty", "-m", message]);
}

pub fn git_remote_add(repo_path: &Path, name: &str, url: &str) {
    git_ok(repo_path, ["remote", "add", name, url]);
}

pub fn git_push(repo_path: &Path, remote: &str, refspec: &str) {
    git_ok(repo_path, ["push", remote, refspec]);
}

pub fn git_set_symbolic_head(repo_path: &Path, refname: &str) {
    git_ok(repo_path, ["symbolic-ref", "HEAD", refname]);
}
