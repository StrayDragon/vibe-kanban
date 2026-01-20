## ADDED Requirements

### Requirement: Resolve cached image paths for agent input
The system SHALL resolve `.vibe-images/...` references by looking up task-linked images and using their cached file paths when preparing prompts for coding agents.

#### Scenario: Resolve image path from task prompt via cache
- **WHEN** a prompt contains `![alt](.vibe-images/abc.png)` and the task is linked to image record `abc.png`
- **THEN** the resolved path is the image cache file on disk
- **AND** the original prompt text remains intact for the agent

### Requirement: Attach local images for Codex input
The Codex executor SHALL attach resolved `.vibe-images/...` references as `LocalImage` input items in the same order they appear in the prompt.

#### Scenario: Ordered text and image inputs
- **WHEN** a prompt contains text + two `.vibe-images/...` references
- **THEN** Codex receives a sequence of input items that interleaves text and the two local images in the original order
- **AND** any unresolved image path is sent as plain text without a `LocalImage` item

### Requirement: Preserve image rendering after worktree removal
The system SHALL render task images in history even when a task attempt worktree has been removed.

#### Scenario: History image render after attempt removal
- **WHEN** a history entry references `.vibe-images/abc.png` and the attempt worktree no longer exists
- **THEN** the UI resolves image metadata via the task image record and renders the image via `/api/images/{id}/file`

### Requirement: Provide a fake-agent image scenario
The system SHALL include a fake-agent scenario file that emits image-related events for UI validation.

#### Scenario: Run fake-agent scenario
- **WHEN** the fake agent runs with the provided scenario file
- **THEN** the UI receives image-related events referencing `.vibe-images/...` paths for validation
