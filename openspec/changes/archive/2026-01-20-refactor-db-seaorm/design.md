## Context
The current DB layer is SQLx + SQLite, with many handwritten SQL queries, SQLite preupdate/update hooks, and rowid-based lookups. The team intends to move to PostgreSQL, but raw SQL and SQLite-specific behavior make the migration risky.

## Goals / Non-Goals
- Goals:
  - Use SeaORM for async, typed data access across SQLite and PostgreSQL.
  - Establish a clean, cross-database schema with consistent identifiers.
  - Keep API behavior stable while swapping the persistence layer.
  - Establish SeaORM migrations as the canonical schema definition.
- Non-Goals:
  - Preserving existing SQLite data or migration history.
  - Full PostgreSQL rollout in this change.
  - Redesigning API contracts or domain models beyond identifier access patterns.
  - Replacing the event stream design beyond what is required for SeaORM compatibility.

## Current Data Layer Inventory
- Core modules: `crates/db/src/models/*.rs` provide SQLx-backed CRUD for projects, tasks, workspaces, sessions, execution processes/logs, repos, images, tags, scratch, merges, and drafts.
- SQLite-specific dependencies:
  - Preupdate/update hooks in `crates/services/src/services/events.rs`.
  - Rowid lookups in `crates/db/src/models/task.rs`, `crates/db/src/models/project.rs`, `crates/db/src/models/workspace.rs`, `crates/db/src/models/execution_process.rs`, `crates/db/src/models/scratch.rs`.
  - Migrations with `PRAGMA`, generated columns, `json_extract`, and SQLite-only trigger definitions.
- Complex SQL hotspots that will need SeaORM/SeaQuery equivalents:
  - `Task::find_by_project_id_with_attempt_status` (correlated subqueries + joins).
  - `ExecutionProcess::list_missing_before_context` (multi-join + subquery).
  - `ExecutionProcessLogEntries` search/update paths (dynamic query builder).
  - `WorkspaceRepo::update_target_branch_for_children_of_workspace` (nested query + update).
- Tables defined in migrations but not yet represented as modules in `crates/db/src/models`:
  - `drafts`, `shared_tasks`, `shared_activity_cursors`, `task_attempt_activities`.
  - Legacy or renamed tables still present in migration history: `task_attempts`, `executor_sessions`, `task_templates`.
- Scratch usage detail: API routes use session UUID as the scratch key (`/scratch/{scratch_type}/{session_id}`), so the new schema should model scratch as belonging to a session.

## Mapping Strategy
- Code First: hand-author SeaORM entities and migrations to define the new baseline schema.
- Use `id` as the primary key (auto-increment) and `uuid` as a unique external identifier.
- All FK relations point to `id` columns; APIs continue to accept uuid for lookups.
- UUIDs are generated in application code by default; PostgreSQL may use `gen_random_uuid()` where desired.
- SeaORM migrations should use `ColumnType::Uuid` for uuid columns where supported.
- Map enums to `DeriveActiveEnum` with string values matching current CHECK constraints (lowercase).
- Use SeaORM `Json`/`JsonValue` for JSON columns; target SQLite TEXT + JSON1 and PostgreSQL JSONB.
- Move `updated_at` maintenance to application/SeaORM hooks; avoid DB triggers where possible.

## Entity Mapping Priority
- P0 (core lifecycle): `projects`, `tasks`, `workspaces`, `sessions`, `execution_processes`, `execution_process_log_entries`, `execution_process_logs` (legacy), `scratch`.
- P1 (repo + assets): `repos`, `project_repos`, `workspace_repos`, `execution_process_repo_states`, `merges`, `images`, `task_images`, `tags`.
- P2 (auxiliary): `coding_agent_turns`, `task_attempt_activities`, `drafts`, `shared_tasks`, `shared_activity_cursors`.

## DDL to SeaORM Examples
### Example: Tasks (new schema with id PK + uuid UK)
New schema (SQLite and PostgreSQL):
```sql
CREATE TABLE tasks (
    id INTEGER PRIMARY KEY, -- SQLite rowid alias
    uuid BLOB NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'todo',
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);
```

SeaORM Entity (new baseline):
```rust
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "tasks")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}
```

### Example: Scratch (new schema with unique constraint)
New schema (SQLite and PostgreSQL):
```sql
CREATE TABLE scratch (
    id           INTEGER PRIMARY KEY,
    uuid         BLOB NOT NULL UNIQUE,
    session_id   INTEGER NOT NULL,
    scratch_type TEXT NOT NULL,
    payload      TEXT NOT NULL,
    created_at   TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    UNIQUE (session_id, scratch_type)
);
```

SeaORM Entity (unique constraint + FK):
```rust
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "scratch")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub uuid: Uuid,
    pub session_id: i64,
    pub scratch_type: String,
    pub payload: String,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}
```

### Example: Virtual/Generated Column (execution_processes)
SQLite currently uses a generated column for indexing JSON content:
```sql
ALTER TABLE execution_processes
ADD COLUMN executor_action_type TEXT
  GENERATED ALWAYS AS (json_extract(executor_action, '$.type')) VIRTUAL;
CREATE INDEX idx_execution_processes_task_attempt_type_created
ON execution_processes (task_attempt_id, executor_action_type, created_at DESC);
```

SeaORM migration approach:
- SQLite: keep the generated column (custom SQL via `Statement::from_string`).
- PostgreSQL: replace with a functional index on JSONB, e.g.
  `CREATE INDEX ... ON execution_processes ((executor_action->>'type'), created_at DESC);`

### Example: Execution Process Log Entries (event-ish data)
SQLite DDL (current):
```sql
CREATE TABLE execution_process_log_entries (
    execution_id      BLOB NOT NULL,
    channel           TEXT NOT NULL
                     CHECK (channel IN ('raw', 'normalized')),
    entry_index       INTEGER NOT NULL,
    entry_json        TEXT NOT NULL,
    created_at        TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at        TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);
CREATE UNIQUE INDEX idx_execution_process_log_entries_unique
    ON execution_process_log_entries (execution_id, channel, entry_index);
```

SeaORM Entity (composite unique index, enum channel):
```rust
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "execution_process_log_entries")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub execution_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub channel: LogChannel,
    #[sea_orm(primary_key, auto_increment = false)]
    pub entry_index: i32,
    pub entry_json: Json,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum LogChannel {
    #[sea_orm(string_value = "raw")]
    Raw,
    #[sea_orm(string_value = "normalized")]
    Normalized,
}
```

## P0 Baseline Schema (id PK + uuid UK)
### projects
- Columns: `id` (PK, autoincrement), `uuid` (UK), `name`, `dev_script`, `dev_script_working_dir`, `default_agent_working_dir`, `remote_project_id` (uuid), `created_at`, `updated_at`
- Indexes: `projects_uuid_unique`, `projects_remote_project_id_unique`

### tasks
- Columns: `id` (PK), `uuid` (UK), `project_id` (FK -> projects.id), `title`, `description`, `status`, `parent_workspace_id` (nullable), `shared_task_id` (nullable), `created_at`, `updated_at`
- Indexes: `tasks_project_id`, `tasks_status`, `tasks_parent_workspace_id`
- Note: `parent_workspace_id` is nullable and intentionally not enforced as FK to avoid circular dependency with workspaces.

### workspaces
- Columns: `id` (PK), `uuid` (UK), `task_id` (FK -> tasks.id), `container_ref`, `branch`, `agent_working_dir`, `setup_completed_at`, `created_at`, `updated_at`
- Indexes: `workspaces_task_id`, `workspaces_container_ref`

### sessions
- Columns: `id` (PK), `uuid` (UK), `workspace_id` (FK -> workspaces.id), `executor`, `created_at`, `updated_at`
- Indexes: `sessions_workspace_id`, `sessions_created_at`

### execution_processes
- Columns: `id` (PK), `uuid` (UK), `session_id` (FK -> sessions.id), `run_reason`, `executor_action` (JSON), `status`, `exit_code`, `dropped`, `started_at`, `completed_at`, `created_at`, `updated_at`
- Indexes: `execution_processes_session_id`, `execution_processes_status`, `execution_processes_run_reason`, `execution_processes_session_created_at`

### execution_process_log_entries
- Columns: `id` (PK), `uuid` (UK), `execution_process_id` (FK -> execution_processes.id), `channel`, `entry_index`, `entry_json` (JSON), `created_at`, `updated_at`
- Indexes: `log_entries_unique (execution_process_id, channel, entry_index)`, `log_entries_exec_channel_index (execution_process_id, channel, entry_index DESC)`

### execution_process_logs (legacy)
- Columns: `id` (PK), `uuid` (UK), `execution_process_id` (FK -> execution_processes.id), `logs` (JSONL), `byte_size`, `inserted_at`
- Indexes: `execution_process_logs_execution_id`, `execution_process_logs_inserted_at`

### scratch
- Columns: `id` (PK), `uuid` (UK), `session_id` (FK -> sessions.id), `scratch_type`, `payload` (JSON), `created_at`, `updated_at`
- Indexes: `scratch_session_id`, `scratch_unique (session_id, scratch_type)`

### event_outbox
- Columns: `id` (PK), `uuid` (UK), `event_type`, `entity_type`, `entity_uuid`, `payload` (JSON), `created_at`, `published_at`, `attempts`, `last_error`
- Indexes: `event_outbox_published_at`, `event_outbox_entity_uuid`

## Identifier Strategy
- SQLite: use `INTEGER PRIMARY KEY` to get auto-increment and rowid compatibility.
- PostgreSQL: use `BIGINT GENERATED BY DEFAULT AS IDENTITY`.
- UUIDs remain stable external identifiers and are always unique.
- Service layer will provide `find_by_uuid` helpers and resolve `uuid -> id` for FK writes.
- Scratch continues to be addressed by session UUID + scratch_type at the API layer.

## Schema Baseline Strategy
- Create a new SeaORM migration baseline for SQLite and PostgreSQL.
- Data migration is explicitly out of scope; existing SQLite DBs will be reset.
- Remove legacy SQLx migrations from active use; archive for reference only.

## Event Dispatcher (Service-Driven Observer)
### Goals
- Replace SQLite preupdate/update hooks and rowid-based fetches.
- Ensure events are emitted only after successful commits.
- Support retries without database-specific locking behavior.

### Interfaces
- `DomainEvent`: typed events derived from ORM models (TaskUpdated, ProjectUpdated, WorkspaceUpdated, ExecutionProcessUpdated, ScratchUpdated).
- `EventDispatcher`: collects and flushes domain events after transaction success.
- `EventPublisher` trait: abstracts delivery (in-memory broadcast, SSE, or queued delivery).

```rust
pub trait EventPublisher {
    fn publish(&self, event: DomainEvent) -> futures::future::BoxFuture<'static, Result<(), EventError>>;
}

pub struct EventDispatcher<P> {
    publisher: P,
}

impl<P: EventPublisher> EventDispatcher<P> {
    pub async fn enqueue(
        txn: &DatabaseTransaction,
        event: &DomainEvent,
    ) -> Result<(), DbErr> {
        // insert into event_outbox within txn
    }

    pub async fn flush_pending(&self, db: &DatabaseConnection) -> Result<(), EventError> {
        // load unpublished events, publish, mark published
    }
}
```

### UUID -> ID Resolution Pattern
- Service layer accepts uuid inputs and resolves foreign keys before writes.
- Each entity module provides a `find_id_by_uuid` helper used by services.
- Write paths should return both `id` and `uuid` to avoid re-querying within a transaction.

```rust
pub async fn resolve_project_id(
    db: &DatabaseConnection,
    project_uuid: Uuid,
) -> Result<i64, DbErr> {
    Project::find()
        .filter(project::Column::Uuid.eq(project_uuid))
        .select_only()
        .column(project::Column::Id)
        .into_tuple()
        .one(db)
        .await?
        .ok_or(DbErr::RecordNotFound("project not found".into()))
}
```

### Transaction Flow (post-commit)
1. Service layer opens `DatabaseTransaction`.
2. Service performs SeaORM writes, building `DomainEvent` values from the resulting models.
3. Events are inserted into an `event_outbox` table inside the same transaction (for durability).
4. Transaction commits.
5. Dispatcher loads pending outbox entries and publishes them; marks entries as delivered.

### Reliability and Idempotency
- Use an outbox table to provide at-least-once delivery and crash recovery.
- Publish operations must be idempotent by event id.
- Retries are handled by the dispatcher worker, not by SQLite-specific busy logic.

### Implications
- Remove reliance on rowid in events; use explicit ids from service operations.
- Bulk updates must collect affected ids before writing or re-query after update.
- Deprecate `crates/db/src/retry.rs` in favor of backend-appropriate transaction retry (if needed).

## Decisions
- Decision: Use SeaORM (async, multi-backend) as the ORM for core persistence.
  - Alternatives considered: SeaQuery+SQLx (incremental but leaves manual mapping), Diesel (sync-first), staying on SQLx.
- Decision: Introduce a SeaORM migration crate with a new baseline schema (no legacy data retention).
  - Rationale: SeaORM migrations are backend-aware and align with ORM entities.
- Decision: Use auto-increment `id` primary keys with unique `uuid` external identifiers.
- Decision: Replace SQLite preupdate/update hooks with a service-layer `EventDispatcher` using an outbox table for post-commit delivery.
- Decision: Deprecate `crates/db/src/retry.rs` and rely on backend-appropriate transaction/retry handling.

## Risks / Trade-offs
- SeaORM entities must match existing SQLite schema exactly, which can be tedious and error-prone.
- SQLite-only hooks (preupdate/update, rowid) must be replaced to keep event streams consistent.
- Generated columns and trigger-based `updated_at` require backend-specific handling in migrations.
- Large-scale refactor will touch many modules and could introduce regressions in query behavior.

## Migration Plan
1. Define the new baseline schema with id/uuid strategy across core tables.
2. Stand up a SeaORM migration crate that creates the new schema for SQLite and PostgreSQL.
3. Port model modules from SQLx to SeaORM, replacing SQLx queries with entity APIs.
4. Replace DBService connection logic with SeaORM Database/ConnectOptions.
5. Implement EventDispatcher + outbox and remove rowid reliance.
6. Remove SQLx-specific retry behavior and SQLite-only hooks/triggers.
7. Reset local databases on startup or during migration to ensure a clean baseline.

## Open Questions
- How will change notifications be implemented for PostgreSQL (triggers + notify vs. app-side change table)?
- Should we generate entities from the current SQLite schema (sea-orm-cli) or hand-author them for clarity?
- How should JSON fields and enums be modeled for a future PostgreSQL backend?
- Which SeaORM feature flags are required to support UUID across SQLite/PostgreSQL in this workspace?
- Should we adjust `.cargo/config.toml` to use per-worktree `target/` during dependency changes?
