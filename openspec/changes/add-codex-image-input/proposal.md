# Change: Add Codex image input and cached image resolution

## Why
Uploaded images are embedded as markdown but Codex receives only text, and `.vibe-images/` references break when worktrees are deleted or the agent runs from repo subdirs, so images frequently go missing.

## What Changes
- Resolve `.vibe-images/...` references by looking up task-linked images in the cache directory and attaching the absolute file paths to Codex.
- Keep prompt text order while inserting Codex `LocalImage` items for any resolved images.
- Fall back to task-level image metadata for history rendering when worktrees are removed.
- Provide a fake-agent scenario and guidance to exercise the image flow in UI/testing.

## Impact
- Affected specs: agent-image-input (new)
- Affected code: `crates/executors/src/executors/codex/`, `crates/executors/src/actions/`, `crates/services/src/services/image.rs`, `frontend/src/hooks/useImageMetadata.ts`, `docs/`
