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
                        .table(Tasks::Table)
                        .add_column(ColumnDef::new(Tasks::OriginTaskId).big_integer())
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(Tasks::Table)
                        .add_column(
                            ColumnDef::new(Tasks::CreatedByKind)
                                .string_len(32)
                                .not_null()
                                .default(Expr::val("human_ui")),
                        )
                        .to_owned(),
                )
                .await?;
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(Tasks::Table)
                        .add_column(ColumnDef::new(Tasks::OriginTaskId).big_integer())
                        .add_column(
                            ColumnDef::new(Tasks::CreatedByKind)
                                .string_len(32)
                                .not_null()
                                .default(Expr::val("human_ui")),
                        )
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_tasks_origin_task_id")
                    .table(Tasks::Table)
                    .col(Tasks::OriginTaskId)
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
                    .name("idx_tasks_origin_task_id")
                    .to_owned(),
            )
            .await?;

        if matches!(manager.get_database_backend(), DatabaseBackend::Sqlite) {
            manager
                .alter_table(
                    Table::alter()
                        .table(Tasks::Table)
                        .drop_column(Tasks::OriginTaskId)
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(Tasks::Table)
                        .drop_column(Tasks::CreatedByKind)
                        .to_owned(),
                )
                .await?;
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(Tasks::Table)
                        .drop_column(Tasks::OriginTaskId)
                        .drop_column(Tasks::CreatedByKind)
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }
}

#[derive(Iden)]
enum Tasks {
    Table,
    OriginTaskId,
    CreatedByKind,
}
