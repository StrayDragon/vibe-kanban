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
                    .table(McpToolTasks::Table)
                    .col(pk_id_col(manager, McpToolTasks::Id))
                    .col(
                        ColumnDef::new(McpToolTasks::TaskId)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(ColumnDef::new(McpToolTasks::CreatedByClientId).string_len(128))
                    .col(
                        ColumnDef::new(McpToolTasks::ToolName)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(McpToolTasks::ToolArgumentsJson)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(McpToolTasks::Status)
                            .string_len(32)
                            .not_null()
                            .default(Expr::val("working")),
                    )
                    .col(ColumnDef::new(McpToolTasks::StatusMessage).text())
                    .col(ColumnDef::new(McpToolTasks::AttemptId).uuid())
                    .col(ColumnDef::new(McpToolTasks::KanbanTaskId).uuid())
                    .col(ColumnDef::new(McpToolTasks::ProjectId).uuid())
                    .col(
                        ColumnDef::new(McpToolTasks::Resumable)
                            .boolean()
                            .not_null()
                            .default(Expr::val(true)),
                    )
                    .col(ColumnDef::new(McpToolTasks::TtlMs).big_integer())
                    .col(ColumnDef::new(McpToolTasks::PollIntervalMs).big_integer())
                    .col(ColumnDef::new(McpToolTasks::ResultJson).json())
                    .col(ColumnDef::new(McpToolTasks::ErrorJson).json())
                    .col(timestamp_col(McpToolTasks::CreatedAt))
                    .col(
                        ColumnDef::new(McpToolTasks::LastUpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(McpToolTasks::ExpiresAt).timestamp())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_mcp_tool_tasks_task_id")
                    .table(McpToolTasks::Table)
                    .col(McpToolTasks::TaskId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_mcp_tool_tasks_status_updated_at")
                    .table(McpToolTasks::Table)
                    .col(McpToolTasks::Status)
                    .col(McpToolTasks::LastUpdatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_mcp_tool_tasks_attempt_id")
                    .table(McpToolTasks::Table)
                    .col(McpToolTasks::AttemptId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_mcp_tool_tasks_kanban_task_id")
                    .table(McpToolTasks::Table)
                    .col(McpToolTasks::KanbanTaskId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_mcp_tool_tasks_project_id")
                    .table(McpToolTasks::Table)
                    .col(McpToolTasks::ProjectId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_mcp_tool_tasks_expires_at")
                    .table(McpToolTasks::Table)
                    .col(McpToolTasks::ExpiresAt)
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
                    .table(McpToolTasks::Table)
                    .to_owned(),
            )
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

fn timestamp_col<T: Iden>(col: T) -> ColumnDef {
    ColumnDef::new(col)
        .timestamp()
        .not_null()
        .default(Expr::current_timestamp())
        .to_owned()
}

#[derive(Iden)]
enum McpToolTasks {
    Table,
    Id,
    TaskId,
    CreatedByClientId,
    ToolName,
    ToolArgumentsJson,
    Status,
    StatusMessage,
    AttemptId,
    KanbanTaskId,
    ProjectId,
    Resumable,
    TtlMs,
    PollIntervalMs,
    ResultJson,
    ErrorJson,
    CreatedAt,
    LastUpdatedAt,
    ExpiresAt,
}
