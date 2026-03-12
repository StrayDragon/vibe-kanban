use sea_orm_migration::{prelude::*, sea_orm::DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add project-level continuation budget (default-off).
        manager
            .alter_table(
                Table::alter()
                    .table(Projects::Table)
                    .add_column(
                        ColumnDef::new(Projects::DefaultContinuationTurns)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        // Add task-level continuation override. NULL = inherit.
        manager
            .alter_table(
                Table::alter()
                    .table(Tasks::Table)
                    .add_column(ColumnDef::new(Tasks::ContinuationTurnsOverride).integer())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .if_not_exists()
                    .table(TaskOrchestrationStates::Table)
                    .col(pk_id_col(manager, TaskOrchestrationStates::Id))
                    .col(
                        ColumnDef::new(TaskOrchestrationStates::TaskId)
                            .big_integer()
                            .not_null(),
                    )
                    // The workspace/attempt these counters apply to. Used to reset budgets on new attempts.
                    .col(ColumnDef::new(TaskOrchestrationStates::AttemptId).uuid())
                    .col(
                        ColumnDef::new(TaskOrchestrationStates::ContinuationTurnsUsed)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(TaskOrchestrationStates::LastVkNextAction).string_len(32))
                    // If VK_NEXT exists but is invalid, store the raw token for diagnostics.
                    .col(
                        ColumnDef::new(TaskOrchestrationStates::LastVkNextInvalidRaw)
                            .string_len(64),
                    )
                    .col(ColumnDef::new(TaskOrchestrationStates::LastVkNextAt).timestamp())
                    .col(
                        ColumnDef::new(TaskOrchestrationStates::LastContinuationStopReasonCode)
                            .string_len(64),
                    )
                    .col(
                        ColumnDef::new(TaskOrchestrationStates::LastContinuationStopReasonDetail)
                            .text(),
                    )
                    .col(
                        ColumnDef::new(TaskOrchestrationStates::LastContinuationStopAt).timestamp(),
                    )
                    .col(
                        ColumnDef::new(TaskOrchestrationStates::LastControlTransferReasonCode)
                            .string_len(64),
                    )
                    .col(ColumnDef::new(TaskOrchestrationStates::LastControlTransferDetail).text())
                    .col(ColumnDef::new(TaskOrchestrationStates::LastControlTransferAt).timestamp())
                    .col(timestamp_col(TaskOrchestrationStates::CreatedAt))
                    .col(timestamp_col(TaskOrchestrationStates::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_task_orchestration_states_task_id")
                            .from(
                                TaskOrchestrationStates::Table,
                                TaskOrchestrationStates::TaskId,
                            )
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
                    .name("idx_task_orchestration_states_task_id")
                    .table(TaskOrchestrationStates::Table)
                    .col(TaskOrchestrationStates::TaskId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("idx_task_orchestration_states_task_id")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(TaskOrchestrationStates::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Tasks::Table)
                    .drop_column(Tasks::ContinuationTurnsOverride)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Projects::Table)
                    .drop_column(Projects::DefaultContinuationTurns)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Projects {
    Table,
    DefaultContinuationTurns,
}

#[derive(Iden)]
enum Tasks {
    Table,
    Id,
    ContinuationTurnsOverride,
}

#[derive(Iden)]
enum TaskOrchestrationStates {
    Table,
    Id,
    TaskId,
    AttemptId,
    ContinuationTurnsUsed,
    LastVkNextAction,
    LastVkNextInvalidRaw,
    LastVkNextAt,
    LastContinuationStopReasonCode,
    LastContinuationStopReasonDetail,
    LastContinuationStopAt,
    LastControlTransferReasonCode,
    LastControlTransferDetail,
    LastControlTransferAt,
    CreatedAt,
    UpdatedAt,
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
