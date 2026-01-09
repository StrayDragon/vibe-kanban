## Context
We need a reproducible executor that simulates Codex-like streaming behavior to diagnose UI state-sync issues without relying on external agents or network dependencies. The executor must be usable in any environment and integrate with the existing execution-process + log streaming pipeline.

## Goals / Non-Goals
- Goals:
  - Provide a Fake executor that can be selected via executor profiles.
  - Emit Codex-compatible JSONL events so existing log normalization and UI code paths are exercised.
  - Support deterministic runs with a seed and controlled cadence.
  - Avoid any real filesystem/network side effects by default.
- Non-Goals:
  - Implement a full Codex app-server protocol.
  - Execute real tools or shell commands.
  - Change UI appearance or layout.

## Decisions
- Decision: Add a new executor variant (FAKE_AGENT) with a dedicated config struct.
  - Rationale: Keeps integration consistent with existing executor selection and settings UI.
- Decision: Implement a small Rust binary (fake-agent) that emits JSONL notifications shaped like Codex `codex/event` messages.
  - Rationale: Allows reuse of Codex log normalization and fits the existing SpawnedChild model.
- Decision: Resolve the fake-agent binary path by default to a sibling binary of the running server, with an env override (e.g., `VIBE_FAKE_AGENT_PATH`).
  - Rationale: Ensures availability in all environments while allowing manual overrides.
- Decision: Provide a deterministic PRNG seed and configurable timing/event mix in the fake agent config.
  - Rationale: Supports reproducible test cases and controlled stress scenarios.

## Risks / Trade-offs
- Risk: Packaging the fake-agent binary across platforms.
  - Mitigation: Use sibling-binary resolution and validate on build; allow override path env.
- Risk: Divergence from real Codex behavior.
  - Mitigation: Use codex-protocol types to build events and keep a minimal supported subset (session configured, message delta, tool events, finished).

## Migration Plan
- Add new executor variant and default profile entry.
- Ship fake-agent binary with builds.
- Document usage and dev/test setup steps.

## Open Questions
- Should the fake agent be visible in settings by default, or gated behind a config flag?
- Which simulated tool events are most important for reproducing UI issues (e.g., bash, apply_patch, mcp)?
- Should scripted scenarios be supported via a JSONL input file in addition to random generation?
