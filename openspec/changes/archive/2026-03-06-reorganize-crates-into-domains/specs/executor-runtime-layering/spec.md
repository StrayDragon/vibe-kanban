## ADDED Requirements

### Requirement: Executor registry MUST be separated from provider implementations
The executor workspace MUST provide a registry/facade layer that selects and exposes executors without embedding provider implementation logic directly in the registry crate.

#### Scenario: Executor selection goes through registry
- **WHEN** runtime code resolves an executor for a task attempt or profile
- **THEN** it does so through the executor registry/facade crate rather than by importing provider implementation crates directly

### Requirement: Provider crates MUST depend only on executor foundations
Executor provider crates MUST depend only on executor protocol/core crates, log foundations, and low-level utilities. They MUST NOT depend on transport adapter crates, runtime composition crates, or backend capability crates.

#### Scenario: Provider crate remains runtime-agnostic
- **WHEN** a provider crate dependency graph is audited
- **THEN** the graph contains executor foundations and shared low-level crates but no dependency path to `server`, `app-runtime`, or capability crates

### Requirement: Shared executor behavior MUST live below the registry layer
Behavior reused across multiple executor providers, including normalization, command assembly, environment shaping, and shared retry/runtime helpers, MUST live in executor foundation crates below the registry layer.

#### Scenario: Shared provider behavior is implemented once
- **WHEN** multiple providers need the same runtime helper behavior
- **THEN** that behavior is implemented in executor foundation crates instead of duplicated across provider crates or embedded in the registry crate
