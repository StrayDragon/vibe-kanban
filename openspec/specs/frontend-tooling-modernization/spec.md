# frontend-tooling-modernization Specification

## Purpose
TBD - created by archiving change frontend-hardening-phase-4-modernize-tooling. Update Purpose after archive.
## Requirements
### Requirement: Frontend toolchain upgrades preserve core verification commands
After toolchain modernization, the repository SHALL continue to support the standard frontend verification commands and they SHALL succeed on a clean checkout.

#### Scenario: Typecheck succeeds
- **WHEN** a developer runs `pnpm -C frontend run check`
- **THEN** TypeScript compilation completes without errors

#### Scenario: Lint succeeds
- **WHEN** a developer runs `pnpm -C frontend run lint`
- **THEN** lint completes with 0 errors

#### Scenario: Build succeeds
- **WHEN** a developer runs `pnpm -C frontend run build`
- **THEN** the production build completes successfully

#### Scenario: End-to-end smoke succeeds
- **WHEN** a developer runs `pnpm run e2e:just-run`
- **THEN** the e2e suite completes successfully

### Requirement: Security baseline is not regressed by modernization
Toolchain modernization SHALL NOT reintroduce HIGH or MODERATE production dependency vulnerabilities in the frontend.

#### Scenario: Audit remains clean for HIGH/MODERATE
- **WHEN** a developer runs `pnpm -C frontend audit --prod`
- **THEN** the report contains 0 HIGH findings and 0 MODERATE findings

