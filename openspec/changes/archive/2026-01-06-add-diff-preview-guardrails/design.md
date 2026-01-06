## Context
Diff previews currently fetch full file contents for large changes, which can cause out-of-memory crashes. We need guardrails that block heavy previews by default while preserving a manual override.

## Goals / Non-Goals
- Goals:
  - Prevent OOM by default when diffs exceed thresholds.
  - Provide simple preset-based configuration.
  - Allow manual override with explicit user action.
  - Keep summary available without loading file contents.
- Non-Goals:
  - Arbitrary per-user custom thresholds beyond presets.
  - Full diff pagination or streaming by file chunk (future work).

## Decisions
- Add `diff_preview_guard` config with presets: Safe, Balanced (default), Relaxed, Off.
  - Safe: maxFiles=200, maxLines=10,000, maxBytes=20MB
  - Balanced: maxFiles=500, maxLines=25,000, maxBytes=50MB
  - Relaxed: maxFiles=1000, maxLines=60,000, maxBytes=150MB
  - Off: guard disabled (still respects existing per-file and cumulative byte caps)
- Compute a lightweight diff summary (file count, added/deleted lines, total bytes) without loading full contents into application memory (e.g., rely on numstat + file metadata).
- When thresholds are exceeded and request is not forced, return a blocked guard response and skip full diff generation.
- Add a "force" query param on the diff stream to allow explicit override.

## Risks / Trade-offs
- Additional git CLI work to compute summary; mitigated by avoiding full content reads.
- Forced load can still be expensive; existing per-file (2MB) and cumulative (200MB) caps remain as safety limits.

## Migration Plan
- Introduce config schema v9 with `diff_preview_guard` defaulting to Balanced.
- No data migrations required; config upgrades handled via config versioning.

## Open Questions
- None.
