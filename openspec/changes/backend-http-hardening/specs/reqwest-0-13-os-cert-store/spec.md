## ADDED Requirements

### Requirement: Outbound HTTPS validation consults the OS trust store

All outbound HTTPS requests made by the backend SHALL validate server
certificates using a verifier that consults the operating system's trust store.

This MUST work in enterprise environments where additional root certificates are
installed into the OS trust store (e.g., corporate proxies).

#### Scenario: Corporate root CA present in OS trust store

- **WHEN** the OS trust store contains an additional root CA required by the
  network
- **THEN** outbound HTTPS requests succeed without requiring per-app custom CA
  configuration

### Requirement: A single `reqwest` major version is used

The backend SHALL avoid mixing multiple major versions of `reqwest` to prevent
inconsistent HTTP/TLS behavior across crates.

#### Scenario: Dependency graph uses `reqwest` 0.13

- **WHEN** the workspace dependency graph is inspected (e.g., via `cargo tree`)
- **THEN** `reqwest` resolves to a single major version (`0.13.x`)

