# frontend-performance-guardrails Specification

## Purpose
TBD - created by archiving change frontend-optimization-phase-5-performance-and-query-keys. Update Purpose after archive.
## Requirements
### Requirement: Route-level code splitting prevents a single large entry chunk
The frontend router SHALL lazy-load non-root route modules to avoid bundling all pages into a single entry chunk.

#### Scenario: Production build emits code-split chunks
- **WHEN** a developer runs `pnpm -C frontend run build`
- **THEN** the output contains code-split JavaScript chunks (more than one JS bundle under `frontend/dist/assets/`)

