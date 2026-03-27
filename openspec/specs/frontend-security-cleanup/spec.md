# frontend-security-cleanup Specification

## Purpose
TBD - created by archiving change frontend-hardening-phase-1-security-cleanup. Update Purpose after archive.
## Requirements
### Requirement: Frontend production dependencies have no HIGH/MODERATE advisories
The repository SHALL ensure that the frontend production dependency graph does not contain HIGH or MODERATE known vulnerabilities when evaluated with the project’s supported audit command.

#### Scenario: Audit passes without HIGH/MODERATE findings
- **WHEN** a developer runs `pnpm -C frontend audit --prod`
- **THEN** the report contains 0 HIGH findings and 0 MODERATE findings

### Requirement: Unused frontend dependencies and files are removed
The repository SHALL remove unused frontend dependencies and unused source files that are not referenced by the build/runtime.

#### Scenario: Static analysis reports no unused dependencies/files
- **WHEN** a developer runs the project’s unused-code check (e.g., Knip)
- **THEN** the report does not include unused frontend dependencies or unused frontend source files targeted by this change

### Requirement: Frontend typecheck and build remain healthy after cleanup
The repository SHALL keep the frontend TypeScript typecheck and production build passing after dependency upgrades and dead-code pruning.

#### Scenario: Typecheck succeeds
- **WHEN** a developer runs `pnpm -C frontend run check`
- **THEN** TypeScript compilation completes without errors

#### Scenario: Production build succeeds
- **WHEN** a developer runs `pnpm -C frontend run build`
- **THEN** the build completes successfully without unresolved imports or bundler errors

### Requirement: User-uploaded images must not allow SVG execution
The system SHALL reject SVG uploads (for example `.svg` or `image/svg+xml`) for user-uploaded images that are later served from the same origin.

#### Scenario: SVG upload is rejected
- **WHEN** a client uploads an image with SVG format
- **THEN** the system rejects the upload with a 4xx error

