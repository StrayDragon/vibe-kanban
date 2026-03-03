## Why

Deleting a task can trigger background workspace cleanup while the UI still issues follow-up requests (diff/patch/file, branch status, etc.). Today, those follow-up requests may call server-side “ensure workspace/worktree exists” paths that incorrectly treat a missing `.git/worktrees/` directory as a fatal repository error, resulting in `500 Internal Server Error`.

This is especially disruptive because `.git/worktrees/` is expected to be absent for repositories that have never created a worktree. The server should treat that case as “no worktree metadata yet” and proceed to create/recreate worktrees deterministically.

## What Changes

- Make worktree metadata discovery resilient:
  - Missing `<repo>/.git/worktrees/` is treated as “no worktrees registered” instead of an error.
  - Worktree setup checks fall back to “needs (re)creation” rather than returning an invalid-path error.
- Make workspace “ensure” recreate attempts reliably:
  - Ensure paths have access to each repo’s configured `target_branch`.
  - If an attempt branch is missing, recreate it from `target_branch` before creating the worktree.
- Simplify `git worktree add` helpers to remove ambiguous “create branch” options and centralize branch-creation semantics in `WorktreeManager`.
- Add focused tests that reproduce the missing-metadata failure mode and validate successful first-time worktree creation.

## Capabilities

### New Capabilities
- (none)

### Modified Capabilities
- `workspace-management`: Define requirements for robust worktree creation/ensure behavior when `.git/worktrees/` is missing and when attempt branches need recreation from a configured base branch.

## Impact

### Goals
- Eliminate `500` errors caused by missing `.git/worktrees/` during workspace/worktree ensure flows.
- Ensure task deletion does not leave the UI in a broken state due to transient post-delete refresh requests.
- Keep behavior deterministic and safe: missing worktree metadata implies “no registered worktrees”, not “invalid repository”.

### Non-goals
- Redesigning task deletion UX or adding new frontend flows.
- Introducing new repository layouts or changing how workspace directories are named.
- Broad error-model refactors beyond the worktree/workspace ensure path.

### Risks
- Automatically recreating a missing attempt branch can hide unexpected repo state changes (e.g., someone manually deleted the branch). Mitigation: only recreate when missing and always base it on the configured `target_branch`.
- Changing worktree heuristics could inadvertently mark some edge-case setups as “needs recreation”. Mitigation: keep checks conservative and add targeted tests.

### Verification
- Reproduce the bug by deleting a task and observing post-delete UI requests; confirm no `500` and logs do not include “Failed to read worktree metadata directory”.
- Validate cold-start / ensure flows:
  - First-ever worktree creation in a repo without `.git/worktrees/` succeeds.
  - If an attempt branch is missing, it is recreated from `target_branch` and the worktree is created successfully.
