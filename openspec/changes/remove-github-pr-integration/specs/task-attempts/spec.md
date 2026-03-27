# task-attempts Specification (Delta)

## MODIFIED Requirements

### Requirement: Stable task-attempts API surface during refactors
The system SHALL preserve core `/api/task-attempts/*` routes and response JSON shapes while internal modules are reorganized.

This requirement applies to the local-first attempt lifecycle and observability surfaces (attempt creation/status/logs/changes, local git/worktree operations, and associated DTO shapes). It does not require retaining optional third-party network integration endpoints.

#### Scenario: Core endpoints remain stable
- **WHEN** a client calls an existing core task-attempts endpoint (for example list attempts)
- **THEN** the HTTP method, path, and response shape match the pre-refactor behavior

#### Scenario: Optional PR integration endpoints may be removed
- **WHEN** a client calls a removed remote PR integration endpoint under `/api/task-attempts/*`
- **THEN** the system responds with `404 Not Found` (or `410 Gone`)
- **AND** the system performs no outbound network requests as a side effect of that call

## REMOVED Requirements

### Requirement: GitHub pull request integration endpoints
**Reason**: Reduce network-side effects and credential coupling in the core server; keep a minimal local-first attempt core that is stable under dependency/protocol updates.

**Migration**: Use manual PR workflows outside VK (git + browser/CLI). The UI SHOULD provide copy-friendly information (branch name, compare URL template, suggested commands) rather than calling remote PR APIs.

#### Scenario: PR endpoints are not exposed
- **WHEN** a client calls a GitHub PR integration endpoint (create/attach/list comments)
- **THEN** the response is `404 Not Found` (or `410 Gone`)

