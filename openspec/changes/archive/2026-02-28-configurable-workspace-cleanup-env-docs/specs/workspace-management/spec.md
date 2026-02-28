## ADDED Requirements

### Requirement: Configurable expired workspace cleanup (local deployment)
The system SHALL periodically clean up expired workspaces that have no running processes, and SHALL allow operators to tune the expiry TTL and cleanup interval via environment variables for local deployments.

#### Scenario: Defaults are used when env vars are unset
- **WHEN** `VK_WORKSPACE_EXPIRED_TTL_SECS` and `VK_WORKSPACE_CLEANUP_INTERVAL_SECS` are unset
- **THEN** the system uses the built-in defaults for the expiry TTL and cleanup interval

#### Scenario: TTL can be reduced by operators
- **WHEN** `VK_WORKSPACE_EXPIRED_TTL_SECS` is set to a lower value
- **THEN** workspaces older than that TTL and without running processes are eligible for cleanup

#### Scenario: TTL-based cleanup can be disabled
- **WHEN** `DISABLE_WORKSPACE_EXPIRED_CLEANUP` is set
- **THEN** the system SHALL NOT perform TTL-based expired workspace cleanup

