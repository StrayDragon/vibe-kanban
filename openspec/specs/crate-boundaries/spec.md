# crate-boundaries Specification

## Purpose
TBD - created by archiving change rust-workspace-boundary-refactor. Update Purpose after archive.
## Requirements
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

### Requirement: Capability crates MUST own a single backend domain
The Rust workspace MUST organize business logic into capability crates with stable ownership boundaries. A capability crate MUST own one backend domain and MUST NOT grow into a catch-all application crate.

#### Scenario: Domain ownership remains explicit
- **WHEN** a new backend feature is added
- **THEN** the implementation is assigned to one existing domain crate or a new focused capability crate instead of a broad shared-services crate

### Requirement: Transport adapters MUST depend on focused capability-crate entrypoints
Transport adapter crates such as `server` MUST depend on `app-runtime` and focused capability-crate entrypoints instead of broad service-locator crates that expose unrelated subsystems.

#### Scenario: Route state uses focused dependencies
- **WHEN** a route or MCP handler is wired
- **THEN** its state is assembled from `app-runtime` and the specific capability entrypoints it requires without receiving a catch-all runtime object with unrelated service getters

### Requirement: Blocking git and filesystem work MUST stay behind capability-owned async boundaries
Blocking git, worktree, and filesystem traversal operations MUST be implemented inside capability-owned infrastructure boundaries and exposed to the rest of the workspace through async-safe interfaces.

#### Scenario: Async call path avoids raw blocking git access
- **WHEN** transport or orchestration code needs repository or filesystem work
- **THEN** it calls an async-safe domain API rather than invoking raw `git2` or blocking filesystem operations directly

### Requirement: Broad service-locator crates MUST NOT remain in the runtime path
The runtime path MUST NOT keep broad crates whose primary role is to expose many unrelated subsystems through getters or re-exports. Composition crates MAY wire multiple domains together, but they MUST NOT become long-lived service-locator APIs.

#### Scenario: Workspace audit rejects broad runtime façades
- **WHEN** the crate dependency graph and public entrypoints are reviewed
- **THEN** there is no broad runtime crate on the main server path whose public API is dominated by unrelated subsystem getters or catch-all re-exports

### Requirement: Deprecated migration tooling is not part of the core server binary
The system SHALL provide deprecated/one-off legacy migration commands via a dedicated migration CLI binary and SHALL NOT expose those commands as subcommands of the main `server` runtime binary.

#### Scenario: Server help does not include legacy migration commands
- **WHEN** an operator inspects the `server` CLI help output
- **THEN** the `server` CLI does not expose a `legacy` (or equivalent) migration command group
- **AND** operators are directed to use the dedicated migration CLI instead

#### Scenario: Server runtime does not depend on migration implementation modules
- **WHEN** the `server` binary is built for production use
- **THEN** the dependency graph for the runtime server does not include legacy migration implementation modules

### Requirement: Migration tooling is an explicit, out-of-band operator action
The migration CLI SHALL be invoked explicitly by an operator and SHALL NOT be triggered via HTTP APIs or runtime background jobs.

#### Scenario: No HTTP endpoint triggers migrations
- **WHEN** a client calls any HTTP endpoint on the server
- **THEN** no legacy migration code path is executed as a side effect of that call

