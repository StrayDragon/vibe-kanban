## ADDED Requirements
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
