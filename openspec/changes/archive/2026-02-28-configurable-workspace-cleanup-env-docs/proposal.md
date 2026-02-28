# Change: Configurable workspace cleanup + generated env docs

## Why

Local deployments accumulate many workspaces/worktrees over time. The existing periodic cleanup is useful, but its default timing is fixed and the knobs are not documented, making it hard for operators to tune behavior (e.g., shorten TTL for disk pressure).

## What Changes

- Make local workspace expiry cleanup configurable via environment variables:
  - `VK_WORKSPACE_EXPIRED_TTL_SECS`: TTL threshold for "expired" workspaces (default: 72h).
  - `VK_WORKSPACE_CLEANUP_INTERVAL_SECS`: periodic cleanup tick interval (default: 30m).
  - `DISABLE_WORKSPACE_EXPIRED_CLEANUP`: disables TTL-based cleanup (orphan cleanup remains separately controlled).
- Add a generated environment variable reference doc at `docs/env.gen.md`, including CI enforcement via `pnpm run generate-env-docs:check`.
- Link the generated env reference from `docs/operations.md`.

## Capabilities

### New Capabilities
- (none)

### Modified Capabilities
- `workspace-management`: allow operators to tune local workspace expiry cleanup via env vars without changing defaults.
- `install-app`: provide a generated environment variable reference and keep it up to date in CI.

## Impact

- Backend: `crates/local-deployment/src/container.rs`, `crates/db/src/models/workspace.rs`
- Docs/tooling: `scripts/generate-env-docs.js`, `docs/env.gen.md`, `docs/operations.md`
- CI/build: `.github/workflows/test.yml`, `package.json`

## Goals

- Keep default behavior unchanged (72h TTL, 30m interval).
- Provide safe, documented knobs to tune cleanup behavior for local deployments.
- Prevent doc drift by generating `docs/env.gen.md` and enforcing it in CI.

## Non-goals

- Exposing workspace cleanup TTL/interval as a runtime web setting.
- Making cleanup "smarter" than TTL + running-process guards (e.g., checking git dirtiness, branch age, etc.).
- Changing orphan cleanup semantics beyond existing behavior.

## Risks

- Lowering TTL can delete uncommitted worktree changes; mitigate with explicit documentation and conservative minimum clamps.
- Misconfiguration could cause overly aggressive cleanup; mitigate with defaults and clamps.

## Verification

- `pnpm run generate-env-docs:check`
- `cargo test --workspace`
- Optional manual smoke: run local deployment with `VK_WORKSPACE_EXPIRED_TTL_SECS=3600` and confirm eligible workspaces are cleaned on the next tick.

