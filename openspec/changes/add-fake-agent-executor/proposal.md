# Change: Add fake agent executor for deterministic simulation

## Why
Frontend state-sync issues and streaming regressions are difficult to reproduce with real agents. We need a controllable fake executor that behaves like Codex to reliably simulate streams, tool calls, and finishes across environments.

## What Changes
- Add a new fake agent executor that is selectable in executor profiles in all environments.
- Provide a fake-agent binary that emits Codex-compatible event streams (session configured, message deltas, tool events, finished).
- Add configuration for deterministic seeding, timing cadence, and event mix.
- Ensure the fake agent performs no real filesystem/network operations by default.
- Document how to enable and use the fake agent for reproduction and tests.

## Impact
- Affected specs: fake-agent-executor (new)
- Affected code: executors (new executor + binary), executor profiles/defaults, shared types, settings UI copy, dev assets seed
