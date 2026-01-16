# Tasks

## 1. Specs
- [x] Add agent-image-input spec delta covering workspace image resolution, Codex LocalImage input, and test scenario support.

## 2. Backend / Executors
- [x] Implement prompt image parsing that extracts `.vibe-images/...` references and resolves them via task-linked image records to cache-path absolute files.
- [x] Extend Codex app-server client to send structured `InputItem` lists.
- [x] Update Codex executor to send ordered text/image items, falling back to text-only when image paths are invalid or unlinked.
- [x] Add tests for parsing, ordering, and cache-path resolution edge cases.
- [x] Add history rendering fallback to task-level image metadata when worktree copies are missing.

## 3. Fake Agent / Docs
- [x] Add a fake-agent scenario file that emits image-related events for UI testing.
- [x] Document how to run the fake-agent scenario to validate image handling.
