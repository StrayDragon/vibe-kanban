## ADDED Requirements

### Requirement: Dev-only tooling is excluded from production entry bundle
The frontend SHALL NOT statically import dev-only tooling modules from the production entry path and SHALL NOT render dev-only tooling in production builds.

#### Scenario: Production entry avoids static imports for dev-only modules
- **WHEN** a developer reviews the production entry module (`frontend/src/main.tsx`)
- **THEN** it does not statically import `click-to-react-component`
- **AND** it does not statically import `vibe-kanban-web-companion`

#### Scenario: Dev-only tooling is loaded only in development
- **WHEN** the app runs with `import.meta.env.DEV` set to true
- **THEN** the dev-only tooling modules are loaded via dynamic import and rendered
- **AND** when `import.meta.env.DEV` is false the dev-only tooling is not loaded and not rendered

