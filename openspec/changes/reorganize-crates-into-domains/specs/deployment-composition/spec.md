## ADDED Requirements

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
