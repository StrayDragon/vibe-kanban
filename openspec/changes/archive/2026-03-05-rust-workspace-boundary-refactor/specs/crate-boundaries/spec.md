# crate-boundaries Specification (Incremental)

## ADDED Requirements

### Requirement: Protocol crates MUST be transport/framework independent
Protocol crates (shared type crates used for persistence, MCP/HTTP payloads, and TypeScript generation) MUST NOT depend on transport or web frameworks.

#### Scenario: Protocol crates do not depend on Axum
- **WHEN** the workspace dependency graph is audited in CI
- **THEN** protocol crates have no dependency path to Axum, rmcp, or other transport frameworks

### Requirement: Persistence crates MUST not depend on executor runtime crates
Crates responsible for persistence (e.g. `db`, `db-migration`) MUST NOT depend on runtime executor/provider implementations. Persisted JSON fields MUST be defined and validated via protocol crates.

#### Scenario: db has no dependency path to executors runtime
- **WHEN** the workspace dependency graph is audited in CI
- **THEN** the `db` crate dependency graph contains `executors-protocol` but does not contain `executors` (runtime)

### Requirement: Transport adapters MUST own framework-specific message conversion
Any conversion between transport/framework types (e.g. SSE/WS message types) and internal protocol/core types MUST live in adapter-layer crates. Protocol/core crates MUST expose transport-agnostic types only.

#### Scenario: Log messages remain usable across transports
- **WHEN** log messages are emitted from core/runtime code
- **THEN** the same log message type can be used by both SSE and WebSocket adapters without requiring core crates to import transport/framework types

### Requirement: Core crates MUST not depend on web frameworks
Core crates (business logic, services, persistence) MUST NOT depend on web frameworks (Axum) or MCP server frameworks (rmcp). Only adapter crates may depend on these frameworks.

#### Scenario: services remains framework-agnostic
- **WHEN** the workspace dependency graph is audited in CI
- **THEN** core crates (including `services` and `db`) have no dependency path to Axum or rmcp

### Requirement: Legacy persisted/config shapes MUST be migrated, not supported indefinitely
When persisted/config shapes change (field names, enum variants, nested structures), the system MUST provide migrations that upgrade legacy data to the current strict protocol schema. Runtime parsing MUST be strict after migration.

#### Scenario: Legacy executor profile identifiers are upgraded
- **WHEN** the system encounters legacy executor profile identifiers in persisted JSON/config
- **THEN** migration code upgrades them to the current strict identifiers and strict decoding succeeds afterward

### Requirement: TypeScript generation MUST use protocol-owned types
Shared TypeScript types generated from Rust MUST be derived from protocol-owned Rust types and MUST NOT require importing adapter-layer crates.

#### Scenario: Types generation stays protocol-first
- **WHEN** `pnpm run generate-types` is executed
- **THEN** type generation succeeds using protocol-owned Rust types as the source of truth

### Requirement: CI MUST enforce crate boundary regressions
The CI pipeline MUST include checks that fail the build when boundary rules are violated (e.g. persistence depending on runtime, protocol crates depending on web frameworks).

#### Scenario: Boundary regression fails CI
- **WHEN** a new dependency violates the workspace layering rules
- **THEN** CI fails with an actionable error describing the violated boundary rule
