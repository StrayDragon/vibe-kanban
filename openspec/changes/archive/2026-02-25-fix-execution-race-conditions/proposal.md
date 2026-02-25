# Change: Fix execution race conditions in completion and repo updates

## Why
Two race patterns cause inconsistent outcomes: concurrent exit-monitor/manual-stop finalization, and TOCTOU behavior when adding repositories and updating default working directory.

## What Changes
- Introduce single-owner completion finalization for each execution process.
- Ensure manual stop and exit monitor share one completion state machine.
- Make repository-add transition logic atomic so default working-directory updates reflect committed inserts only.

## Impact
- Affected specs: `execution-race-safety`
- Affected code: `crates/local-deployment/src/container.rs`, `crates/services/src/services/project.rs`, related persistence layers/tests
- Out of scope: changing task orchestration product behavior beyond race safety.
