## Context

VK runs Codex via the Codex “app-server” JSON-RPC interface (`codex app-server`) and decodes events/requests using pinned Rust protocol crates (`codex-protocol`, `codex-app-server-protocol`). In practice, `codex-cli` can update rapidly (pnpm global upgrades or fallback mechanisms like `npx @latest`), which can outpace VK’s pinned protocol and cause:

- Decode failures (unknown enum variants / new item shapes)
- Missing/ignored signals (new notifications or request variants)
- Unpredictable follow-up/resync behavior and difficult debugging

At the same time, Codex supports “Dynamic Tools”: the client can register tool schemas on `thread/start`, and the app-server can request tool execution via `item/tool/call`. This provides an ergonomic way for Codex to call VK-native capabilities with strict schemas.

Constraints:
- Strategy: “track latest” (no long-term support for old Codex versions).
- Local-only execution environment.
- Preferred failure mode: **disable Codex executor** with actionable guidance when incompatible (do not attempt to run anyway).

## Goals / Non-Goals

**Goals:**
- Make protocol drift detectable and actionable before a run starts (fail-fast).
- Reduce incidence of mid-run breakage caused by `codex-cli` being ahead of VK.
- Provide a minimal, high-signal Dynamic Tools surface that differentiates VK for Codex users.

**Non-Goals:**
- Implement other experimental Codex features (collaboration modes, review APIs) in this change.
- Guarantee compatibility with older Codex CLIs (only latest is supported).
- Build remote execution or multi-host orchestration for Codex (local-first only).

## Decisions

### 1) Compatibility check uses protocol/schema fingerprinting (not version strings)

**Decision:** Treat compatibility as “protocol schema match” and compute a stable fingerprint from the locally installed `codex-cli` app-server protocol schema bundle. Compare against VK’s expected fingerprint generated from VK’s pinned protocol crates.

**Rationale:** Version strings are insufficient (can be missing, ambiguous, or not directly mapped to Rust crate revs). Schema fingerprints directly reflect the JSON surface VK must decode.

**Mechanism (conceptual):**
- Runtime: run `codex app-server generate-json-schema --out <tmp>` and hash the produced schema bundle (or the v2 bundle file) into `runtime_protocol_fingerprint`.
- Build-time: generate and embed `expected_protocol_fingerprint` from VK’s pinned `codex-app-server-protocol` crate schema output.
- If mismatch: mark Codex as incompatible and surface remediation copy.

**Alternatives considered:**
- Maintain a mapping table `codex-cli semver -> protocol rev` (high churn, fragile).
- Parse `codex --version` only (does not guarantee protocol stability).
- Best-effort decode with broad `serde_json::Value` (still risks semantic mismatch and hidden failures).

### 2) Executor is disabled on incompatibility (hard gate)

**Decision:** When incompatibility is detected, VK disables the Codex executor in settings/selection and blocks spawn attempts with a clear error message.

**Rationale:** This matches the desired user experience (“don’t run incompatible Codex”) and prevents mid-run failures that are harder to recover from.

**Remediation copy should include:**
- Detected `codex-cli` version (if available)
- Fingerprint mismatch indication
- Two recommended fixes:
  1) Upgrade VK
  2) Align/pin `codex-cli` to a compatible version (advanced)

### 3) Compatibility check is cached and revalidated on change

**Decision:** Cache compatibility status keyed by the resolved Codex command identity (path + reported version/source) and re-run the fingerprint check only when those inputs change, or on explicit user refresh.

**Rationale:** Avoid repeated schema generation while keeping results accurate after upgrades.

### 4) Dynamic Tools are implemented as a VK-owned registry with strict schemas

**Decision:** Introduce a small Dynamic Tool registry for Codex threads, and pass tool specs via `ThreadStartParams.dynamic_tools`. Handle tool execution requests via server request `item/tool/call`.

**Rationale:** Dynamic Tools provide a “first-class” integration path without requiring users to configure external MCP servers. Strict JSON schemas keep tool calls reliable and testable.

**Tool scope (Phase 2, suggested minimal set):**
- Read-only observability (no approvals required beyond standard access rules):
  - `vk.get_attempt_status`
  - `vk.tail_attempt_logs`
  - `vk.get_attempt_changes`
- Optional (later) gated mutations (require explicit approvals):
  - `vk.create_follow_up` / `vk.add_task_comment` / `vk.update_task_status`

**Alternatives considered:**
- “Just use MCP”: workable, but requires external server wiring/config and does not integrate as seamlessly into Codex’s native tool call path.
- Mega-tool (one tool that does everything): rejected due to safety/UX and schema complexity.

### 5) Tool execution uses VK approval + auditing conventions

**Decision:** Treat each Dynamic Tool call as a tool invocation for logging/auditing. For mutating tools, require explicit approval via VK’s existing approvals system and record an approval decision in logs.

**Rationale:** Keeps user trust and consistency with existing `bash`/`edit` approvals flows.

## Risks / Trade-offs

- **Schema generation overhead** → Cache fingerprints and only re-check on version/path changes; allow manual refresh.
- **False negatives due to experimental fields** → Fingerprint should focus on stable “v2 schema bundle” outputs, but may still change frequently; acceptable under “track latest”.
- **Tool surface creep** → Keep Phase 2 tool list small; require a spec + scenario for each new tool.
- **Security boundary ambiguity (read vs write)** → Start read-only; gate mutations behind approvals and clear UI messaging.

## Migration Plan

- Phase 1:
  - Add compatibility fingerprinting and a UI surface to report compatibility.
  - Enforce “incompatible ⇒ disabled” behavior.
- Phase 2:
  - Add Dynamic Tool registry, registration at `thread/start`, and request handling for `item/tool/call`.
  - Add normalization/log UX for Dynamic Tool activity.

Rollback:
- If Dynamic Tools introduce instability, ship a config flag to disable dynamic tool registration while keeping Phase 1 gating.

## Open Questions

- Fingerprint definition: hash a single bundle file vs hashing the whole output directory (stability vs completeness).
- Tool naming conventions: `vk.<name>` vs `vk_<name>` to avoid collisions and stay code-mode safe.
- How to represent tool outputs (Markdown-first vs JSON-in-text) given the response supports only text/image content items.
