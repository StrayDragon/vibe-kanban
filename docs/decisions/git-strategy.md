# Decision: Git strategy (git2 vs Git CLI)

## Context

This repository currently uses both:

- **Git CLI** (via `crates/repos/src/git/cli.rs`)
- **libgit2** (via the `git2` crate in `crates/repos/src/git/mod.rs` and `crates/repos/src/worktree_manager.rs`)

The `workspace-deps-slimdown` change aims to reduce build complexity and system
dependencies over time (notably the libgit2/OpenSSL chain), and to avoid
maintaining two competing implementations of the same Git capabilities.

## Current capability inventory

### Git CLI (preferred for working-tree touching operations)

`crates/repos/src/git/cli.rs` centralizes operations that mutate the worktree or
rely on Git's own safety checks, including (non-exhaustive):

- `git worktree add|remove|move|prune`
- staging/diff via temporary index (`GIT_INDEX_FILE`) to include untracked changes
- merge/rebase/commit flows where we want CLI semantics and protections
- fetch/push plumbing where the CLI already has mature behavior and messages

This aligns with the rationale documented at the top of `crates/repos/src/git/cli.rs`
(sparse-checkout correctness, safer semantics, cross-platform stability).

### git2 (currently used for read-heavy and metadata operations)

We still rely on git2 for:

- read-heavy graph queries and object access (commits/trees/blobs/diffs)
- some status/diff generation that is convenient with libgit2 APIs
- worktree metadata inspection/creation in `crates/repos/src/worktree_manager.rs`
- a credentialed `clone_repository` path (under `cloud` feature)

## Options considered

1. **git2-only**
   - Pros: no `git` executable requirement at runtime.
   - Cons: re-implement safety checks, sparse-checkout gaps, higher build/system
     dependency surface (libgit2/OpenSSL), and more platform-specific failures.

2. **Git CLI-only**
   - Pros: aligns with user expectations and Git's own semantics; avoids libgit2
     dependency chain; simpler Rust dependency graph.
   - Cons: requires `git` to be present at runtime; requires robust parsing and
     error handling for outputs; process spawning overhead.

3. **Hybrid (CLI for mutations + git2 for reads)** (status quo)
   - Pros: pragmatic, incremental, already implemented.
   - Cons: keeps libgit2/OpenSSL in the core build; tends to drift into two ways
     of doing the same thing unless boundaries are enforced.

## Decision

We will **standardize on the Git CLI as the long-term single implementation**.

Until the migration is complete:

- All new Git capabilities MUST be implemented on top of `GitCli`.
- git2 usage MUST be considered transitional and should not expand in scope.

## Migration plan (phased)

1. **Freeze scope**
   - Enforce “no new git2 usage” in reviews.
   - Keep all worktree-mutation operations routed through `GitCli`.

2. **Worktree manager migration**
   - Replace `crates/repos/src/worktree_manager.rs` git2 worktree detection and
     creation with `git worktree list --porcelain` + `git worktree add/remove`.

3. **Read-path migration**
   - Migrate remaining read operations to CLI equivalents, preferring stable,
     machine-readable output forms (porcelain, `--format=...`, `-z`).
   - Examples: `git merge-base`, `git rev-parse`, `git show`, `git diff`,
     `git status --porcelain=v2 -z`, `git for-each-ref`.

4. **Remove libgit2 dependency chain**
   - Remove `git2` from `crates/repos` once all call sites are migrated.
   - Re-evaluate the need for `openssl-sys` once libgit2 is gone.

## Consequences

- Server deployments must ensure `git` is installed and runnable.
- Credential flows (clone/fetch/push) should converge on a single CLI-based
  approach (for example `GIT_ASKPASS`, `http.extraheader`, or tokenized URLs with
  careful redaction), so we do not keep git2 solely for auth.

