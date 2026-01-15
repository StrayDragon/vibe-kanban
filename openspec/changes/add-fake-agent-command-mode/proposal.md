# Change: Add command-mode for fake agent simulations

## Why
Fake agent currently emits random or scripted events but cannot be driven by ad-hoc command text, which makes targeted automated tests harder to build and maintain.

## What Changes
- Add a command-mode that activates when the prompt starts with a short prefix (default `help` or `?`), without requiring a slash.
- Provide a small command set for common tool/event sequences (exec, apply_patch, mcp, web_search, warnings/errors, reasoning, message, sleep).
- Allow emitting arbitrary codex `EventMsg` JSON (and raw JSON-RPC notifications) so new event types can be tested without code changes.
- Preserve existing behavior when the prompt does not match the prefix, and keep `scenario_path` highest priority.

## Impact
- Affected specs: fake-agent-executor
- Affected code: `crates/executors/src/executors/fake_agent.rs`
