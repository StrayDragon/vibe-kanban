## Why

Vibe Kanban’s Rust workspace has grown organically and several crates now sit on unclear architectural boundaries. This leads to unnecessary compilation work (small changes recompiling web/runtime stacks), blurred layering (persistence code depending on runtime concerns), and higher maintenance cost when swapping implementations or adding new tooling.

We want a workspace that optimizes for: **fast iteration**, **clear dependency direction**, and **testable/replaceable implementations**.

## What Changes

- Establish an explicit **layered crate model** across the workspace:
  - **Protocol crates**: pure, serializable types shared across boundaries (DB persistence, MCP/HTTP I/O, TS generation). No web/runtime dependencies.
  - **Core crates**: business logic/services. Depends on protocol + db + minimal utilities.
  - **Adapters**: HTTP/MCP/websocket/CLI wiring. Owns framework integrations (Axum/rmcp/etc.).
- Continue the “protocol-first” direction already started by introducing `crates/executors-protocol`:
  - Keep persisted JSON shapes (e.g. `execution_processes.executor_action`) defined in protocol crates and enforce strict decoding after migrations.
- Break up the current `crates/utils` “god crate” into focused crates so that core crates do not inherit heavy dependencies:
  - Move log/event primitives (`LogMsg`, `MsgStore`, entry events) into a **transport-agnostic** crate.
  - Move Axum-specific SSE/WS mappings into adapter-only code (server-side).
  - Split other heavy utilities (git/network/jwt/assets) into dedicated crates only used where needed.
- Modularize `crates/server` and `crates/executors` for maintainability:
  - Split `crates/server/src/mcp/task_server.rs` into smaller modules with clear responsibilities (params parsing, tool handlers, runtime/state, error helpers).
  - Split `crates/executors` into a small “core” plus provider-specific implementations to improve testability and reduce rebuild surface.
- Add **CI guardrails** to prevent boundary regressions (e.g. “db MUST NOT depend on executors runtime”, “protocol crates MUST NOT depend on Axum”).

## Capabilities

### New Capabilities
- `crate-boundaries`: Workspace layering rules and dependency constraints (protocol/core/adapter), including how persisted JSON and TS-generated types are owned.

### Modified Capabilities
- (none)

## Impact

### Goals
- Reduce rebuild/compile time by removing heavy dependencies from widely-used crates (notably `crates/utils` and crates that depend on it transitively).
- Make crate boundaries explicit and enforceable:
  - DB/persistence crates depend only on protocol crates for persisted JSON shapes.
  - Transport/framework code (Axum/rmcp) stays at the edge.
- Improve testability by isolating side-effectful adapters behind small interfaces and keeping shared types in protocol crates.

### Non-goals
- Behavior changes to HTTP/MCP APIs or frontend UX (the refactor is intended to be behavior-preserving).
- Introducing long-term compatibility shims (old module paths and legacy JSON shapes are migrated/updated, not supported indefinitely).

### Risks
- Large mechanical churn (module paths, crate names) can create merge conflicts and slow reviews; mitigation: phase the work and keep each phase verifiable.
- Incorrect assumptions about “who depends on what” can cause cyclic deps; mitigation: define the dependency graph up-front and add CI checks.
- Data/config migration mistakes can brick old persisted state; mitigation: migration-first, strict validation, and “fail fast” on invalid payloads.

### Verification
- `cargo fmt --all`
- `cargo clippy --all --all-targets -- -D warnings`
- `cargo test --workspace`
- `pnpm run generate-types` (after moving protocol-owned TS types)
- Dependency checks (examples):
  - `cargo tree -p db | rg \"\\bexecutors v\"` MUST return nothing
  - `cargo tree -p <protocol-crate> | rg axum` MUST return nothing
