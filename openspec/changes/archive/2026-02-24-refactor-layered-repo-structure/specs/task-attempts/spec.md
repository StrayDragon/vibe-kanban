## ADDED Requirements

### Requirement: Stable task-attempts API surface during refactors
The system SHALL preserve existing `/api/task-attempts/*` routes and response JSON shapes while internal modules are reorganized.

#### Scenario: Existing client continues to work
- **WHEN** a client calls an existing task-attempts endpoint (e.g. list attempts)
- **THEN** the HTTP method, path, and response shape match the pre-refactor behavior

### Requirement: DTOs remain discoverable and typed
The system SHALL define task-attempts request/response types in a dedicated DTO module, and types MUST remain available for ts-rs generation.

#### Scenario: Type generation succeeds
- **WHEN** `pnpm run generate-types:check` is executed
- **THEN** the generated `shared/types.ts` is up to date and compilation succeeds
