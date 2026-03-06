## ADDED Requirements

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
