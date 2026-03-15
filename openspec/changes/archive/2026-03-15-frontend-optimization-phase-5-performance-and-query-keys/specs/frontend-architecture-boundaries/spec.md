## ADDED Requirements

### Requirement: Query keys are not inlined outside domain key factories
The frontend codebase SHALL NOT inline React Query `queryKey` arrays in application modules and SHALL instead import query keys from a domain key factory module.

#### Scenario: Lint blocks inline query keys in pages/components
- **WHEN** a developer runs `pnpm -C frontend run lint`
- **THEN** lint fails if any file under `frontend/src/**/*.{ts,tsx}` defines `queryKey: [...]` inline (including invalidation calls)
- **AND** the query key MUST be referenced via an imported key factory (for example `taskAttemptKeys.*`, `projectKeys.*`, etc.)

