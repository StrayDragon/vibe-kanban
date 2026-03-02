use sea_orm_migration::{prelude::*, sea_orm::DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .if_not_exists()
                    .table(Approvals::Table)
                    .col(pk_id_col(manager, Approvals::Id))
                    .col(uuid_col(Approvals::Uuid))
                    .col(ColumnDef::new(Approvals::AttemptId).uuid().not_null())
                    .col(
                        ColumnDef::new(Approvals::ExecutionProcessId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Approvals::ToolCallId)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Approvals::ToolName)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Approvals::ToolInputJson).json().not_null())
                    .col(
                        ColumnDef::new(Approvals::Status)
                            .string_len(32)
                            .not_null()
                            .default(Expr::val("pending")),
                    )
                    .col(ColumnDef::new(Approvals::DeniedReason).text())
                    .col(timestamp_col(Approvals::CreatedAt))
                    .col(ColumnDef::new(Approvals::TimeoutAt).timestamp().not_null())
                    .col(ColumnDef::new(Approvals::RespondedAt).timestamp())
                    .col(ColumnDef::new(Approvals::RespondedByClientId).string_len(128))
                    .col(timestamp_col(Approvals::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_approvals_uuid")
                    .table(Approvals::Table)
                    .col(Approvals::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_approvals_attempt_status_created_at")
                    .table(Approvals::Table)
                    .col(Approvals::AttemptId)
                    .col(Approvals::Status)
                    .col(Approvals::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_approvals_execution_process_id")
                    .table(Approvals::Table)
                    .col(Approvals::ExecutionProcessId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_approvals_execution_process_tool_call")
                    .table(Approvals::Table)
                    .col(Approvals::ExecutionProcessId)
                    .col(Approvals::ToolCallId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Approvals::Table).to_owned())
            .await?;
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

fn uuid_col<T: Iden>(col: T) -> ColumnDef {
    ColumnDef::new(col).uuid().not_null().to_owned()
}

fn timestamp_col<T: Iden>(col: T) -> ColumnDef {
    ColumnDef::new(col)
        .timestamp()
        .not_null()
        .default(Expr::current_timestamp())
        .to_owned()
}

#[derive(Iden)]
enum Approvals {
    Table,
    Id,
    Uuid,
    AttemptId,
    ExecutionProcessId,
    ToolCallId,
    ToolName,
    ToolInputJson,
    Status,
    DeniedReason,
    CreatedAt,
    TimeoutAt,
    RespondedAt,
    RespondedByClientId,
    UpdatedAt,
}
