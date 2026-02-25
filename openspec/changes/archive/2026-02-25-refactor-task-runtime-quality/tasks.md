## 1. Implementation
- [x] 1.1 Wrap blocking git CLI invocations used by HTTP paths with `spawn_blocking` or equivalent executor isolation.
- [x] 1.2 Add regression tests to ensure task-attempt operations remain responsive under concurrent requests.
- [x] 1.3 Refactor conversation-history initialization to avoid repeated initial-load resets while streaming.
- [x] 1.4 Decompose `TaskFollowUpSection` into focused hooks/components without changing API behavior.
- [x] 1.5 Add/update frontend tests for history-loading and follow-up interactions.

## 2. Verification
- [x] 2.1 `cargo test -p server task_attempts -- --nocapture` and `cargo test -p server run_git_operation_does_not_block_async_runtime -- --nocapture`
- [x] 2.2 `pnpm -C frontend run test -- UseConversationHistory` and `pnpm -C frontend run check`
- [x] 2.3 `openspec validate refactor-task-runtime-quality --strict`
