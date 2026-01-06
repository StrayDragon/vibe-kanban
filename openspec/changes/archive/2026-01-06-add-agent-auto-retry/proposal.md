# Change: Agent auto-retry configuration

## Why
Transient coding agent failures (e.g., temporary rate limiting or load) currently require manual retries. This slows users down and creates friction during recovery.

## What Changes
- Add per-executor configuration for recoverable error patterns (regex), retry delay (seconds), and max retry attempts.
- Automatically retry failed coding-agent runs when error output matches configured patterns.
- Emit a system tip in the conversation when an auto-retry is scheduled/executed.

## Impact
- Affected specs: agent-auto-retry (new)
- Affected code: executor config schema + profiles, execution process completion handling, retry scheduling, frontend conversation UI styling, settings UI schema validation
