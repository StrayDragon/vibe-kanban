# deployment-composition Specification

## Purpose
TBD - created by archiving change refactor-deployment-maintainability. Update Purpose after archive.
## Requirements
### Requirement: Narrow route dependency boundaries
HTTP route handlers SHALL depend on focused service interfaces instead of broad concrete deployment types.

#### Scenario: Route uses minimal service surface
- **WHEN** a route is constructed
- **THEN** its state exposes only the service methods required by that route

### Requirement: Staged deployment initialization
Local deployment initialization SHALL be split into composable stages with explicit responsibilities.

#### Scenario: Initialize deployment in phases
- **WHEN** the server starts local deployment
- **THEN** configuration loading, service construction, and background startup execute through distinct stages

### Requirement: Shared model-loading behavior
Model-loading middleware SHALL use shared helpers for common not-found and database-error handling.

#### Scenario: Missing model in loader
- **WHEN** a loader cannot find a requested model by id
- **THEN** it returns consistent not-found behavior via shared helper logic

### Requirement: Application runtime MUST be the sole backend composition root
The backend MUST define a dedicated application runtime composition crate that owns startup sequencing, capability wiring, and background lifecycle registration for the local server process.

#### Scenario: Runtime boot is centrally composed
- **WHEN** the server process starts
- **THEN** configuration loading, domain construction, background jobs, and shutdown wiring are assembled by the application runtime composition crate

### Requirement: Legacy deployment facades MUST be removed after migration
Once the application runtime composition crate is introduced, legacy broad deployment facades used only as runtime service locators MUST be removed rather than retained as compatibility layers.

#### Scenario: Composition migration completes without compatibility façade
- **WHEN** the runtime migration is finished
- **THEN** the server startup path no longer depends on legacy deployment façade crates for domain access

### Requirement: Domain injection MUST stay narrower than runtime composition
The application runtime MAY construct all domains, but transport adapters SHALL receive only the capability-crate entrypoints needed for their route groups or handler modules.

#### Scenario: Route group receives bounded domain access
- **WHEN** a route group or MCP tool module is initialized
- **THEN** it is wired with only the capability-crate entrypoints required by that group instead of the full runtime composition surface

