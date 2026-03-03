## 1. Worktree metadata handling

- [x] 1.1 Update `WorktreeManager::find_worktree_git_internal_name()` to treat missing `<repo>/.git/worktrees/` as `Ok(None)` (NotFound is not an error); verify with a new unit test in `crates/services`.
- [x] 1.2 Update `WorktreeManager::is_worktree_properly_set_up()` so “no internal worktree name found” returns `Ok(false)` (needs creation) instead of failing; verify by exercising the ensure path in a unit/integration test.

## 2. Ensure workspace using target branches

- [x] 2.1 Change `WorkspaceManager::ensure_workspace_exists()` to accept repo inputs that include `target_branch` (e.g. `RepoWorkspaceInput`) and call `WorktreeManager::create_worktree(..., create_branch=true)` for each repo; verify via `cargo check -p services`.
- [x] 2.2 Make `WorktreeManager::create_worktree()` branch creation idempotent (only create the attempt branch if it does not already exist); verify with a unit test that calls it twice without error.
- [x] 2.3 Update local deployment `ensure_container_exists()` to build `RepoWorkspaceInput { repo, target_branch }` from `workspace_repo` rows and call the updated `WorkspaceManager::ensure_workspace_exists()`; verify via `cargo check -p local-deployment`.

## 3. Centralize branch creation; simplify git worktree helpers

- [x] 3.1 Remove the `create_branch` flag from `GitCli::worktree_add()` / `GitService::add_worktree()` and update all call sites to always add worktrees for an existing branch; verify via `cargo check -p services`.
- [x] 3.2 Ensure branch creation happens only in `WorktreeManager::create_worktree()` (and only when missing); verify by grepping for any remaining “create branch via worktree add” logic.

## 4. Tests and end-to-end verification

- [x] 4.1 Add a focused `crates/services` test that creates a new git repo with no `.git/worktrees/` directory and asserts first-time worktree creation succeeds; verify via `cargo test -p services`.
- [x] 4.2 Add a test that simulates a missing attempt branch and asserts ensure recreates it from `target_branch` before worktree creation (or validates the branch-creation behavior directly); verify via `cargo test -p services`.
- [x] 4.3 Manual smoke test: run the app, create a task+attempt, delete the task in the UI, and confirm no `500` occurs and server logs do not include “Failed to read worktree metadata directory”; verify via `pnpm run dev` (or equivalent `just run ...`).
