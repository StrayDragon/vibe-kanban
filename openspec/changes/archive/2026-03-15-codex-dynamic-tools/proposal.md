## Why

VK’s Codex integration is sensitive to protocol drift: the local `codex-cli` can update faster than VK’s pinned `codex-*-protocol` crates, causing decode failures (unknown event/item variants), missing signals, and follow-up/resync instability. This is hard to diagnose and erodes trust.

At the same time, Codex “Dynamic Tools” provide a unique opportunity to make VK feel first-class for Codex users by letting Codex call VK-native actions (attempt/task introspection, lightweight automation) with strict schemas.

## What Changes

- **Phase 1 — Protocol compatibility gate (fail-fast):**
  - Add a deterministic compatibility check between the locally installed `codex-cli` and VK’s pinned Codex app-server protocol.
  - If incompatible, **disable the Codex executor** and show an actionable message (what is incompatible, how to fix: upgrade VK / align `codex-cli`).
  - Improve “soft compatibility” handling so unknown/new fields do not break core flows whenever possible.

- **Phase 2 — Codex Dynamic Tools support (value-add):**
  - Allow VK to register a curated set of Dynamic Tools on `thread/start` and handle `DynamicToolCall` server requests.
  - Implement a small tool set that highlights VK’s strengths (task/attempt observability, safe automation hooks) with strict JSON schemas and auditable outputs.
  - Keep tools minimal and composable (avoid mega-tools); prefer read-only tools first, then gated mutations.

## Capabilities

### New Capabilities
- `codex-protocol-compat-gate`: Detect Codex app-server protocol incompatibility with local `codex-cli` and disable the executor with actionable remediation.
- `codex-dynamic-tools`: Expose VK-native Dynamic Tools to Codex and execute DynamicToolCall requests safely with strict schemas and clear UX.

### Modified Capabilities
- (none)

## Impact

- Backend/Rust:
  - `crates/executor-codex/`: add compatibility fingerprinting + dynamic tool dispatch.
  - `crates/executors-core/`: integrate compatibility status into existing agent command/version visibility where appropriate.
  - `crates/server/` (optional): expose compatibility status via existing “system info / preflight” surfaces for UI consumption.
- Frontend:
  - Agent settings: surface Codex compatibility status (compatible / incompatible with remediation copy).
  - (Phase 2) Tooling UX: present Dynamic Tool activity in logs with consistent normalization and guardrails.
- Operational:
  - Local-only; no remote executor assumptions. The system should not silently fall back to running an incompatible Codex.

## Goals

- Make Codex executor failures predictable and actionable (fail-fast, not mid-run).
- Reduce maintenance burden from rapid Codex CLI evolution by using machine-checkable compatibility signals.
- Deliver an “aha” feature for Codex users via Dynamic Tools that feels uniquely VK.

## Non-goals

- Implementing other experimental Codex features (e.g., collaboration modes, review APIs) in this change (they may be evaluated later).
- Supporting older Codex versions long-term (strategy: “track latest”).
- Shipping remote/daemonized Codex execution (explicitly local-first).

## Risks

- Compatibility checks that rely on version strings can be brittle; prefer protocol/schema fingerprints.
- Dynamic Tools increase the surface area for approvals/permissions; must start with read-only tools and keep mutations explicitly gated.
- Over-scoping tools can create UX and safety debt; enforce a small, composable tool set.

## Verification

- Phase 1: unit/integration tests that simulate mismatched protocol schema → Codex executor is disabled with a clear message.
- Phase 2: e2e-style scenario exercising a Dynamic Tool call end-to-end (tool registered → called → output rendered and logged).
