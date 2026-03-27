# crate-boundaries Specification (Delta)

## ADDED Requirements

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

