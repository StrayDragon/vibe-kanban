use sea_orm_migration::{prelude::*, sea_orm::DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Hard cut: remove legacy automation toggles. Automation becomes milestone-scoped.
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
                    .table(Tasks::Table)
                    .drop_column(Tasks::AutomationMode)
                    .to_owned(),
            )
            .await?;

        // Milestone metadata lives on task_groups (TaskGroup-as-Milestone).
        if matches!(manager.get_database_backend(), DatabaseBackend::Sqlite) {
            manager
                .alter_table(
                    Table::alter()
                        .table(TaskGroups::Table)
                        .add_column(ColumnDef::new(TaskGroups::Objective).text())
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(TaskGroups::Table)
                        .add_column(ColumnDef::new(TaskGroups::DefinitionOfDone).text())
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(TaskGroups::Table)
                        .add_column(ColumnDef::new(TaskGroups::DefaultExecutorProfileId).json())
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(TaskGroups::Table)
                        .add_column(
                            ColumnDef::new(TaskGroups::AutomationMode)
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
                        .table(TaskGroups::Table)
                        .add_column(ColumnDef::new(TaskGroups::RunNextStepRequestedAt).timestamp())
                        .to_owned(),
                )
                .await?;
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(TaskGroups::Table)
                        .add_column(ColumnDef::new(TaskGroups::Objective).text())
                        .add_column(ColumnDef::new(TaskGroups::DefinitionOfDone).text())
                        .add_column(ColumnDef::new(TaskGroups::DefaultExecutorProfileId).json())
                        .add_column(
                            ColumnDef::new(TaskGroups::AutomationMode)
                                .string_len(32)
                                .not_null()
                                .default(Expr::val("manual")),
                        )
                        .add_column(ColumnDef::new(TaskGroups::RunNextStepRequestedAt).timestamp())
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_task_groups_automation_run_next")
                    .table(TaskGroups::Table)
                    .col(TaskGroups::AutomationMode)
                    .col(TaskGroups::RunNextStepRequestedAt)
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
                    .name("idx_task_groups_automation_run_next")
                    .table(TaskGroups::Table)
                    .to_owned(),
            )
            .await?;

        // Revert milestone columns.
        if matches!(manager.get_database_backend(), DatabaseBackend::Sqlite) {
            manager
                .alter_table(
                    Table::alter()
                        .table(TaskGroups::Table)
                        .drop_column(TaskGroups::RunNextStepRequestedAt)
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(TaskGroups::Table)
                        .drop_column(TaskGroups::AutomationMode)
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(TaskGroups::Table)
                        .drop_column(TaskGroups::DefaultExecutorProfileId)
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(TaskGroups::Table)
                        .drop_column(TaskGroups::DefinitionOfDone)
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(TaskGroups::Table)
                        .drop_column(TaskGroups::Objective)
                        .to_owned(),
                )
                .await?;
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(TaskGroups::Table)
                        .drop_column(TaskGroups::RunNextStepRequestedAt)
                        .drop_column(TaskGroups::AutomationMode)
                        .drop_column(TaskGroups::DefaultExecutorProfileId)
                        .drop_column(TaskGroups::DefinitionOfDone)
                        .drop_column(TaskGroups::Objective)
                        .to_owned(),
                )
                .await?;
        }

        // Restore removed legacy columns with their previous defaults.
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
                    .table(Tasks::Table)
                    .add_column(
                        ColumnDef::new(Tasks::AutomationMode)
                            .string_len(32)
                            .not_null()
                            .default(Expr::val("inherit")),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Projects {
    Table,
    ExecutionMode,
}

#[derive(Iden)]
enum Tasks {
    Table,
    AutomationMode,
}

#[derive(Iden)]
enum TaskGroups {
    Table,
    Objective,
    DefinitionOfDone,
    DefaultExecutorProfileId,
    AutomationMode,
    RunNextStepRequestedAt,
}
