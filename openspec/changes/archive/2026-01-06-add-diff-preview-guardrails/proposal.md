# Change: Diff preview guardrails for large changes

## Why
Large diffs can consume excessive memory during diff generation and rendering, causing crashes. We need a safe default that avoids loading massive diffs while still letting users override when needed.

## What Changes
- Add configurable diff preview guard presets (Safe, Balanced, Relaxed, Off), defaulting to Balanced.
- Compute lightweight diff summaries and block full diff streaming when thresholds are exceeded unless forced.
- Show summary + warning UI when blocked, with a "Force load" action.
- Ensure stats-only diff paths do not read file contents.

## Impact
- Affected specs: diff-preview-guardrails (new)
- Affected code: crates/services (git, diff_stream, config), crates/server routes, frontend diff panel/hooks, shared types
