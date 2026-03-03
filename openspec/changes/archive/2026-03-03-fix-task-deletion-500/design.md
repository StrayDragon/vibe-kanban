## Context

Vibe Kanban uses git worktrees to create per-attempt workspaces. Many HTTP endpoints (diff, patch, file access, PR flows, etc.) call into `ContainerService::ensure_container_exists()`, which in turn calls `WorkspaceManager::ensure_workspace_exists()` and `WorktreeManager::ensure_worktree_exists()` to (re)create missing workspace directories.

When deleting a task, the server deletes database records first and then performs workspace cleanup in a background task. During and immediately after deletion, the UI may still issue follow-up requests for the task’s latest attempt (or stale in-flight queries). These follow-up requests can still attempt to “ensure” a workspace/worktree exists.

The current worktree ensure path has two failure modes that can surface as `500 Internal Server Error`:

1. **Missing `.git/worktrees/` is treated as fatal**
   - `WorktreeManager::find_worktree_git_internal_name()` scans `<repo>/.git/worktrees/*/gitdir` to map a filesystem path to a git “internal worktree name”.
   - For repos that have never created a worktree, `<repo>/.git/worktrees/` does not exist by design.
   - The current implementation returns `WorktreeError::Repository(...)` when `read_dir` fails, which bubbles up as `ContainerError` and then `ApiError::Container` → `500`.

2. **Ensure can fail when the attempt branch is missing**
   - Cold-restart / ensure flows currently call `ensure_worktree_exists()` (which assumes the branch already exists).
   - If the attempt branch is missing (e.g., manually deleted, cleaned up, or never created due to an earlier failure), `git worktree add <path> <branch>` fails with `fatal: invalid reference`.
   - The retry path attempts metadata cleanup, which again tries to read `.git/worktrees/` and can trigger the same `500` in (1).

## Goals / Non-Goals

**Goals:**
- Treat a missing `<repo>/.git/worktrees/` directory as “no worktree metadata yet”, not an invalid repository.
- Make workspace/worktree “ensure” idempotent and resilient during post-delete refresh requests.
- Ensure that if an attempt branch is missing, the server recreates it from the configured `target_branch` before creating the worktree.
- Keep changes localized to services/local-deployment code paths, with minimal behavioral surface area change.

**Non-Goals:**
- Changing task deletion semantics (DB vs filesystem ordering) or introducing UI-side delays.
- Adding new configuration or modifying workspace directory naming/layout.
- Broad refactors to the API error model beyond fixing this specific 500 failure mode.

## Decisions

### 1) Treat `.git/worktrees/` NotFound as “no worktrees”

**Decision:** In `WorktreeManager::find_worktree_git_internal_name()`, if `<repo>/.git/worktrees/` does not exist (`ErrorKind::NotFound`), return `Ok(None)` instead of an error.

**Rationale:** This directory is not guaranteed to exist until the first worktree is created. The ensure path should be able to create the first worktree without requiring pre-existing metadata.

**Alternatives considered:**
- Using `git worktree list --porcelain` for discovery (more robust but slower and adds shell dependency to a hot path).
- Using libgit2-only APIs to enumerate worktrees (not as reliable for mutation paths; the code already prefers git CLI for add/remove).

### 2) Unknown internal worktree name means “needs recreation”

**Decision:** If `find_worktree_git_internal_name()` returns `None`, `is_worktree_properly_set_up()` returns `Ok(false)` (not set up) instead of returning `InvalidPath`.

**Rationale:** “No metadata match” is a normal state for first creation, and can also happen after partial cleanups. Treating it as “recreate” makes ensure deterministic and avoids returning errors to callers.

### 3) Ensure workspace using `target_branch` to recreate missing attempt branches

**Decision:** Update `WorkspaceManager::ensure_workspace_exists()` to accept repo inputs that include each repo’s `target_branch` (i.e., `RepoWorkspaceInput { repo, target_branch }`). For each repo, call:
- `WorktreeManager::create_worktree(repo.path, attempt_branch, worktree_path, target_branch, create_branch=true)`

`WorktreeManager::create_worktree()` will be made idempotent: it creates the attempt branch only if it does not already exist.

**Rationale:** Ensure flows need enough information to recreate an attempt branch when missing. The database already stores `target_branch` per `workspace_repo`, so local deployment can build `RepoWorkspaceInput` cheaply.

**Alternatives considered:**
- Keep `ensure_workspace_exists(repos: &[Repo])` and on “invalid reference” retry by creating a branch and calling ensure again (more branching and harder to reason about).
- Query `workspace_repo` inside `services` (undesirable coupling: `services` should not pull DB models into worktree creation beyond existing `Repo` usage).

### 4) Centralize branch creation semantics; remove “create branch” from `git worktree add` helpers

**Decision:** Remove the `create_branch` flag from `GitCli::worktree_add()` / `GitService::add_worktree()` and make the only branch-creation logic live in `WorktreeManager::create_worktree()`.

**Rationale:** A single responsible place prevents inconsistent git CLI invocations and makes branch recreation policy explicit and testable.

## Risks / Trade-offs

- **[Risk] Auto-recreating a missing attempt branch may hide unexpected manual deletions** → **Mitigation:** only recreate if missing, and always recreate from `target_branch` (the configured base).
- **[Risk] Treating missing metadata as “recreate” could cause unnecessary recreations in rare corrupted states** → **Mitigation:** `is_worktree_properly_set_up()` still requires the filesystem path to exist and the worktree to be registered when metadata is present; otherwise recreation is the safest recovery behavior.
- **[Trade-off] Ensure paths may now create branches during reads** → **Mitigation:** this only happens when the attempt branch is missing; for normal cases it is a no-op.

## Migration Plan

- No database migrations and no config versioning changes.
- Deploy as a standard backend update (restart server).
- Rollback by reverting the change; any created attempt branches/worktrees remain as normal git artifacts and are safe to keep or clean up.

## Open Questions

- None. The design intentionally avoids changing HTTP status mappings; the primary fix is to prevent an avoidable internal error.
