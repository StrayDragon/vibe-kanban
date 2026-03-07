use sea_orm_migration::{prelude::*, sea_orm::DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if matches!(manager.get_database_backend(), DatabaseBackend::Sqlite) {
            manager
                .alter_table(
                    Table::alter()
                        .table(Projects::Table)
                        .add_column(
                            ColumnDef::new(Projects::ExecutionMode)
                                .string_len(32)
                                .not_null()
                                .default(Expr::val("manual")),
                        )
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(Projects::Table)
                        .add_column(
                            ColumnDef::new(Projects::SchedulerMaxConcurrent)
                                .integer()
                                .not_null()
                                .default(1),
                        )
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(Projects::Table)
                        .add_column(
                            ColumnDef::new(Projects::SchedulerMaxRetries)
                                .integer()
                                .not_null()
                                .default(3),
                        )
                        .to_owned(),
                )
                .await?;
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(Projects::Table)
                        .add_column(
                            ColumnDef::new(Projects::ExecutionMode)
                                .string_len(32)
                                .not_null()
                                .default(Expr::val("manual")),
                        )
                        .add_column(
                            ColumnDef::new(Projects::SchedulerMaxConcurrent)
                                .integer()
                                .not_null()
                                .default(1),
                        )
                        .add_column(
                            ColumnDef::new(Projects::SchedulerMaxRetries)
                                .integer()
                                .not_null()
                                .default(3),
                        )
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_table(
                Table::create()
                    .if_not_exists()
                    .table(TaskDispatchStates::Table)
                    .col(pk_id_col(manager, TaskDispatchStates::Id))
                    .col(
                        ColumnDef::new(TaskDispatchStates::TaskId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskDispatchStates::Controller)
                            .string_len(32)
                            .not_null()
                            .default(Expr::val("scheduler")),
                    )
                    .col(
                        ColumnDef::new(TaskDispatchStates::Status)
                            .string_len(32)
                            .not_null()
                            .default(Expr::val("idle")),
                    )
                    .col(
                        ColumnDef::new(TaskDispatchStates::RetryCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(TaskDispatchStates::MaxRetries)
                            .integer()
                            .not_null()
                            .default(3),
                    )
                    .col(ColumnDef::new(TaskDispatchStates::LastError).text())
                    .col(ColumnDef::new(TaskDispatchStates::BlockedReason).text())
                    .col(ColumnDef::new(TaskDispatchStates::NextRetryAt).timestamp())
                    .col(ColumnDef::new(TaskDispatchStates::ClaimExpiresAt).timestamp())
                    .col(timestamp_col(TaskDispatchStates::CreatedAt))
                    .col(timestamp_col(TaskDispatchStates::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_task_dispatch_states_task_id")
                            .from(TaskDispatchStates::Table, TaskDispatchStates::TaskId)
                            .to(Tasks::Table, Tasks::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_task_dispatch_states_task_id")
                    .table(TaskDispatchStates::Table)
                    .col(TaskDispatchStates::TaskId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_task_dispatch_states_status_next_retry")
                    .table(TaskDispatchStates::Table)
                    .col(TaskDispatchStates::Status)
                    .col(TaskDispatchStates::NextRetryAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(TaskDispatchStates::Table)
                    .to_owned(),
            )
            .await?;

        if matches!(manager.get_database_backend(), DatabaseBackend::Sqlite) {
            manager
                .alter_table(
                    Table::alter()
                        .table(Projects::Table)
                        .drop_column(Projects::ExecutionMode)
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(Projects::Table)
                        .drop_column(Projects::SchedulerMaxConcurrent)
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(Projects::Table)
                        .drop_column(Projects::SchedulerMaxRetries)
                        .to_owned(),
                )
                .await?;
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(Projects::Table)
                        .drop_column(Projects::ExecutionMode)
                        .drop_column(Projects::SchedulerMaxConcurrent)
                        .drop_column(Projects::SchedulerMaxRetries)
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }
}

fn pk_id_col<T: Iden>(manager: &SchemaManager, col: T) -> ColumnDef {
    let mut col = ColumnDef::new(col);
    match manager.get_database_backend() {
        DatabaseBackend::Sqlite => {
            col.integer();
        }
        _ => {
            col.big_integer();
        }
    }
    col.not_null().auto_increment().primary_key().to_owned()
}

fn timestamp_col<T: Iden>(col: T) -> ColumnDef {
    ColumnDef::new(col)
        .timestamp()
        .not_null()
        .default(Expr::current_timestamp())
        .to_owned()
}

#[derive(Iden)]
enum Projects {
    Table,
    ExecutionMode,
    SchedulerMaxConcurrent,
    SchedulerMaxRetries,
}

#[derive(Iden)]
enum TaskDispatchStates {
    Table,
    Id,
    TaskId,
    Controller,
    Status,
    RetryCount,
    MaxRetries,
    LastError,
    BlockedReason,
    NextRetryAt,
    ClaimExpiresAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Tasks {
    Table,
    Id,
}
