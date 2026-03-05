## Context

The current Rust workspace (`crates/`) has multiple crates with unclear layering and broad dependency footprints. The most visible issues are:

- `crates/utils` is depended on by most crates and currently contains both “core” helpers and transport/framework code (e.g. Axum SSE/WS types in `crates/utils/src/log_msg.rs` and `crates/utils/src/msg_store.rs`).
- Several crates inherit heavy dependencies transitively from `utils` (Axum, reqwest, git2, jwt, etc.), increasing rebuild time and coupling unrelated components.
- `crates/server/src/mcp/task_server.rs` is very large and mixes concerns (env parsing, tool handler logic, runtime state, validation, error construction), making it harder to maintain.
- `crates/executors` contains many provider implementations plus shared runtime logic, creating a large rebuild surface and making provider-level testing/replacement harder.

This change proposes an explicit architecture and a staged refactor that preserves behavior while improving compile times, boundaries, and testability.

## Goals / Non-Goals

**Goals:**
- Define and enforce a workspace layering model: protocol → core → adapters.
- Ensure protocol/persistence-facing JSON shapes are owned by protocol crates (strict decoding after migrations).
- Remove Axum/rmcp/web dependencies from crates that are not transport adapters.
- Reduce rebuild surface by splitting large crates (`utils`, `executors`, `server`) into focused crates/modules.
- Add guardrails so the architecture stays stable as the workspace evolves.

**Non-Goals:**
- Changing HTTP/MCP external behavior (schemas, routes, tool semantics) as part of the refactor.
- Maintaining long-term compatibility shims for old module paths or legacy JSON shapes.
- Reworking the overall product architecture (this is crate organization + dependency direction).

## Decisions

### 1) Workspace layering model

We adopt the following layers and dependency direction:

- **Protocol crates**
  - Contain serializable types that cross boundaries: DB persistence, MCP/HTTP payloads, TS generation.
  - MUST avoid transport/framework dependencies (no Axum/rmcp).
  - Examples: `executors-protocol` (already introduced).
- **Core crates**
  - Contain business logic and services.
  - Depend on protocol crates and persistence crates (`db`), plus lightweight utilities.
  - SHOULD be transport-agnostic.
- **Adapter crates**
  - Own the boundary to frameworks and runtime environments (HTTP, SSE/WS, MCP, CLI).
  - Depend on core + protocol crates.
  - Examples: `server` (Axum), `mcp_task_server` (rmcp).

This model is enforced via CI checks (see Decision 6).

### 2) Protocol-first persistence + strictness

Persisted JSON fields and stable wire types are defined in protocol crates and validated strictly.

- Example baseline: `db` depends on `executors-protocol` (not on `executors` runtime) and persisted `executor_action` is a strict protocol type after DB migration.
- No “best-effort fallback” parsing in runtime code; migrations update old data and decoding becomes strict.

### 3) Split `utils` by transport boundaries first (Logs / SSE / WS)

The highest leverage boundary fix is to remove Axum types from `utils` because `utils` sits high in the dependency graph.

Plan:
- Create `crates/logs-protocol` for `LogMsg` and other transport-agnostic log message enums/structs.
- Create `crates/logs-store` for `MsgStore` and history/stream logic.
- Create `crates/logs-axum` (adapter) to provide:
  - `LogMsg` → `axum::response::sse::Event`
  - `LogMsg` → `axum::extract::ws::Message`
  - helper stream adapters for SSE

This preserves behavior while keeping Axum out of protocol/core crates.

Alternative considered: keep adapters in `server` only. Rejected because multiple crates currently need the mapping (e.g. `deployment`), and a dedicated adapter crate keeps ownership clear and avoids re-implementations.

### 4) Further decomposition of `utils`

After logs are split, continue extracting heavy domains so crates only pay for what they use:

- `utils-core`: small, broadly-used helpers (text/path/version/tokio utilities).
- `utils-git`: git2-based operations and helpers.
- `utils-net`: reqwest-based HTTP helpers (if needed).
- `utils-assets`: `rust-embed` assets and asset path helpers.
- `utils-jwt`: jsonwebtoken wrapper logic.

All call sites are updated in one pass; no compatibility re-exports are kept long-term.

### 5) Modularize `executors`

Split `executors` into:

- `executors-core`: shared runtime logic (profiles, command building, env overrides, normalization utilities, dispatch wiring).
- Provider crates: `executor-codex`, `executor-claude`, `executor-cursor`, etc.
- A small facade crate (can remain `executors`) that wires enabled providers and exposes the registry used by `services/server`.

Provider crates are optional dependencies behind features so that builds can compile a subset of providers when desired (compile-time wins; better replacement/testing).

### 6) Decompose `server` MCP implementation

`crates/server/src/mcp/task_server.rs` is split into focused modules (or a sub-crate) to reduce file size and clarify responsibilities:

- `mcp/params`: parsing + invalid params classification
- `mcp/errors`: standard error helpers
- `mcp/runtime`: task runtime + persistence hooks
- `mcp/tools/*`: tool handler implementations grouped by domain

No behavior change is intended; this is purely structural.

### 7) CI guardrails for boundaries

Add explicit checks to fail fast when boundaries regress, for example:

- `db` MUST NOT depend on executors runtime crates.
- Protocol crates MUST NOT depend on Axum.
- “Core” crates MUST NOT depend on Axum unless explicitly designated as adapters.

These checks can be implemented as a small script that runs `cargo tree` for selected roots and asserts absence/presence of patterns.

## Risks / Trade-offs

- **Large mechanical churn** → mitigate by phasing, keeping each phase green (`fmt/clippy/test`) and landing sequential PRs.
- **Dependency cycles during crate splitting** → mitigate by defining the target graph up-front and using `cargo tree` frequently during refactors.
- **Hidden transport coupling** (types accidentally pull in Axum) → mitigate by enforcing “no Axum in protocol crates” in CI and code review.
- **Migration mistakes** for persisted JSON/config → mitigate by “migration-first + strict decode” and adding focused migration tests where possible.

## Migration Plan

Phase 0 (baseline, already implemented in the repo):
- Introduce `executors-protocol` and migrate persisted `executor_action` JSON to strict protocol types.

Phase 1 (logs boundary):
- Introduce `logs-protocol`, `logs-store`, `logs-axum`.
- Move `LogMsg` / `MsgStore` out of `utils` into the new crates.
- Update `server/deployment/services/executors` call sites.
- Remove `axum` dependency from `utils` and from crates that no longer need it transitively.

Phase 2 (utils decomposition):
- Introduce `utils-core`, `utils-git`, `utils-jwt`, `utils-assets`, `utils-net` as needed.
- Move modules out of `utils` and update all imports.
- Delete leftover `utils` modules or convert `utils` into a thin crate that only hosts narrowly-scoped core helpers (no heavy deps).

Phase 3 (executors modularization):
- Create `executors-core` and provider crates.
- Move shared logic and provider implementations.
- Add facade wiring + feature flags.
- Update dependency graph so crates can compile a subset of providers.

Phase 4 (server MCP decomposition):
- Split `task_server.rs` into modules/sub-crate without semantic changes.
- Ensure `cargo test --workspace` and MCP tool schemas remain stable.

Phase 5 (guardrails):
- Add CI dependency checks and documentation for the layering model.

## Open Questions

- Do we want a single “umbrella” crate for protocols (e.g. `protocol`) or keep per-domain protocol crates (`executors-protocol`, `logs-protocol`, …)?
- Which executor providers should be enabled by default in CI builds vs local builds?
- Should `deployment` remain an Axum-using adapter crate, or should SSE mapping live strictly in `server` + adapter crates?
