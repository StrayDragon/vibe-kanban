## ADDED Requirements
### Requirement: Runtime database selection
The system SHALL use `DATABASE_URL` when present and otherwise connect using the project default SQLite path (`asset_dir()/db.sqlite`).

#### Scenario: DATABASE_URL set
- **WHEN** `DATABASE_URL` is set to a valid SQLite URL
- **THEN** the system connects using that URL

#### Scenario: DATABASE_URL missing
- **WHEN** `DATABASE_URL` is not set
- **THEN** the system connects using the default local SQLite file under the project asset directory

#### Scenario: DATABASE_URL empty
- **WHEN** `DATABASE_URL` is set but empty
- **THEN** startup fails with a clear configuration error and no fallback connection is attempted

### Requirement: SQLite-only backend (current)
The system MUST reject non-SQLite database URLs until additional backends are supported.

#### Scenario: Non-SQLite DATABASE_URL
- **WHEN** `DATABASE_URL` resolves to a non-SQLite backend
- **THEN** startup fails with a clear unsupported-backend error

### Requirement: SQLite connection pragmas
The system MUST enable foreign key enforcement and apply WAL, synchronous NORMAL, and busy timeout pragmas for SQLite connections.

#### Scenario: SQLite connection established
- **WHEN** a SQLite connection is created
- **THEN** foreign key enforcement is enabled and pragmas are applied

### Requirement: Non-destructive startup migrations
The system SHALL run migrations on startup and MUST NOT delete database files during startup.

#### Scenario: Migration metadata missing
- **WHEN** the migration metadata table is absent
- **THEN** migrations run without deleting the database file
