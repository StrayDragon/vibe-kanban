# frontend-architecture-boundaries Specification

## Purpose
TBD - created by archiving change frontend-hardening-phase-2-architecture-boundaries. Update Purpose after archive.
## Requirements
### Requirement: Frontend network access is centralized behind an API boundary
The frontend codebase SHALL centralize all direct network calls (HTTP requests, WebSocket connections, and SSE connections) behind a defined API boundary.

#### Scenario: Lint prevents ad-hoc fetch usage
- **WHEN** a developer runs `pnpm -C frontend run lint`
- **THEN** the lint rules fail if `fetch` is used outside the approved API boundary modules

### Requirement: Query keys and invalidation rules are centralized per domain
The frontend codebase SHALL define query keys and invalidation rules in a single place per domain and reuse them across hooks and event-driven invalidation.

#### Scenario: Guardrails prevent ad-hoc query keys
- **WHEN** a developer runs the project’s frontend lint/guard checks (e.g., `pnpm -C frontend run lint`)
- **THEN** the checks fail if a hook defines an ad-hoc `queryKey` array instead of importing keys from the domain key factory module

### Requirement: User mutations always produce a visible UI state change
For user-triggered mutations that change server state, the UI SHALL present an immediate visible state change and SHALL reconcile to canonical server state via invalidate/resync.

#### Scenario: Create task is immediately visible
- **WHEN** the user creates a task
- **THEN** the task appears in the UI immediately (optimistic/placeholder state acceptable) and later converges to the server state

#### Scenario: Follow-up message is visible after send
- **WHEN** the user sends a follow-up message for an attempt
- **THEN** the UI shows the new follow-up entry in the conversation and reconciles via invalidate/resync even if streams were delayed

### Requirement: Query keys are not inlined outside domain key factories
The frontend codebase SHALL NOT inline React Query `queryKey` arrays in application modules and SHALL instead import query keys from a domain key factory module.

#### Scenario: Lint blocks inline query keys in pages/components
- **WHEN** a developer runs `pnpm -C frontend run lint`
- **THEN** lint fails if any file under `frontend/src/**/*.{ts,tsx}` defines `queryKey: [...]` inline (including invalidation calls)
- **AND** the query key MUST be referenced via an imported key factory (for example `taskAttemptKeys.*`, `projectKeys.*`, etc.)

