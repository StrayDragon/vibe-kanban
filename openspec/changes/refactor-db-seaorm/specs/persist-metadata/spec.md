## ADDED Requirements
### Requirement: SeaORM data access
The system SHALL use SeaORM entities and query APIs for core persistence operations instead of handwritten SQL in application code.

#### Scenario: Fetch tasks by project
- **WHEN** a client requests tasks for a project
- **THEN** the service loads them via SeaORM entity queries and returns the same records as before

### Requirement: Multi-backend readiness
The persistence layer SHALL remain compatible with SQLite and be structured to support PostgreSQL without rewriting business logic.

#### Scenario: SQLite default runtime
- **WHEN** the system starts with a SQLite database URL
- **THEN** it connects and performs CRUD operations using SeaORM without backend-specific code paths

### Requirement: SeaORM migrations as source of truth
The system SHALL define schema changes using SeaORM migrations and keep them aligned with the runtime schema.

#### Scenario: New environment setup
- **WHEN** migrations run on a new SQLite database
- **THEN** the resulting schema matches the existing production schema for all core tables

### Requirement: Dual identifiers
The system SHALL store an auto-increment `id` primary key and a unique `uuid` external identifier for core tables.

#### Scenario: API lookup by uuid
- **WHEN** a client requests a record by uuid
- **THEN** the service resolves it via the uuid unique key and returns the same record

### Requirement: Event stream continuity
The event stream SHALL continue emitting change patches for task, project, workspace, execution process, and scratch records.

#### Scenario: Task update event
- **WHEN** a task title is updated
- **THEN** an update patch is emitted to connected clients
