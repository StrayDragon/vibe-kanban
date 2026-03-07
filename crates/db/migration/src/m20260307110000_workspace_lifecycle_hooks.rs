use sea_orm_migration::{prelude::*, sea_orm::DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        add_project_columns(manager).await?;
        add_workspace_columns(manager).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_workspace_columns(manager).await?;
        drop_project_columns(manager).await?;
        Ok(())
    }
}

async fn add_project_columns(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let columns = vec![
        ColumnDef::new(Projects::AfterPrepareHookCommand).text().null().to_owned(),
        ColumnDef::new(Projects::AfterPrepareHookWorkingDir)
            .text()
            .null()
            .to_owned(),
        ColumnDef::new(Projects::AfterPrepareHookFailurePolicy)
            .string_len(32)
            .null()
            .to_owned(),
        ColumnDef::new(Projects::AfterPrepareHookRunMode)
            .string_len(32)
            .null()
            .to_owned(),
        ColumnDef::new(Projects::BeforeCleanupHookCommand)
            .text()
            .null()
            .to_owned(),
        ColumnDef::new(Projects::BeforeCleanupHookWorkingDir)
            .text()
            .null()
            .to_owned(),
        ColumnDef::new(Projects::BeforeCleanupHookFailurePolicy)
            .string_len(32)
            .null()
            .to_owned(),
    ];

    if matches!(manager.get_database_backend(), DatabaseBackend::Sqlite) {
        for column in columns {
            manager
                .alter_table(Table::alter().table(Projects::Table).add_column(column).to_owned())
                .await?;
        }
    } else {
        let mut alter = Table::alter().table(Projects::Table).to_owned();
        for column in columns {
            alter.add_column(column);
        }
        manager.alter_table(alter).await?;
    }

    Ok(())
}

async fn add_workspace_columns(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let columns = vec![
        ColumnDef::new(Workspaces::AfterPrepareHookStatus)
            .string_len(32)
            .null()
            .to_owned(),
        ColumnDef::new(Workspaces::AfterPrepareHookRanAt)
            .timestamp()
            .null()
            .to_owned(),
        ColumnDef::new(Workspaces::AfterPrepareHookErrorSummary)
            .text()
            .null()
            .to_owned(),
        ColumnDef::new(Workspaces::BeforeCleanupHookStatus)
            .string_len(32)
            .null()
            .to_owned(),
        ColumnDef::new(Workspaces::BeforeCleanupHookRanAt)
            .timestamp()
            .null()
            .to_owned(),
        ColumnDef::new(Workspaces::BeforeCleanupHookErrorSummary)
            .text()
            .null()
            .to_owned(),
    ];

    if matches!(manager.get_database_backend(), DatabaseBackend::Sqlite) {
        for column in columns {
            manager
                .alter_table(Table::alter().table(Workspaces::Table).add_column(column).to_owned())
                .await?;
        }
    } else {
        let mut alter = Table::alter().table(Workspaces::Table).to_owned();
        for column in columns {
            alter.add_column(column);
        }
        manager.alter_table(alter).await?;
    }

    Ok(())
}

async fn drop_project_columns(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let columns = vec![
        Projects::AfterPrepareHookCommand,
        Projects::AfterPrepareHookWorkingDir,
        Projects::AfterPrepareHookFailurePolicy,
        Projects::AfterPrepareHookRunMode,
        Projects::BeforeCleanupHookCommand,
        Projects::BeforeCleanupHookWorkingDir,
        Projects::BeforeCleanupHookFailurePolicy,
    ];

    if matches!(manager.get_database_backend(), DatabaseBackend::Sqlite) {
        for column in columns {
            manager
                .alter_table(Table::alter().table(Projects::Table).drop_column(column).to_owned())
                .await?;
        }
    } else {
        let mut alter = Table::alter().table(Projects::Table).to_owned();
        for column in columns {
            alter.drop_column(column);
        }
        manager.alter_table(alter).await?;
    }

    Ok(())
}

async fn drop_workspace_columns(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let columns = vec![
        Workspaces::AfterPrepareHookStatus,
        Workspaces::AfterPrepareHookRanAt,
        Workspaces::AfterPrepareHookErrorSummary,
        Workspaces::BeforeCleanupHookStatus,
        Workspaces::BeforeCleanupHookRanAt,
        Workspaces::BeforeCleanupHookErrorSummary,
    ];

    if matches!(manager.get_database_backend(), DatabaseBackend::Sqlite) {
        for column in columns {
            manager
                .alter_table(Table::alter().table(Workspaces::Table).drop_column(column).to_owned())
                .await?;
        }
    } else {
        let mut alter = Table::alter().table(Workspaces::Table).to_owned();
        for column in columns {
            alter.drop_column(column);
        }
        manager.alter_table(alter).await?;
    }

    Ok(())
}

#[derive(Iden)]
enum Projects {
    Table,
    AfterPrepareHookCommand,
    AfterPrepareHookWorkingDir,
    AfterPrepareHookFailurePolicy,
    AfterPrepareHookRunMode,
    BeforeCleanupHookCommand,
    BeforeCleanupHookWorkingDir,
    BeforeCleanupHookFailurePolicy,
}

#[derive(Iden)]
enum Workspaces {
    Table,
    AfterPrepareHookStatus,
    AfterPrepareHookRanAt,
    AfterPrepareHookErrorSummary,
    BeforeCleanupHookStatus,
    BeforeCleanupHookRanAt,
    BeforeCleanupHookErrorSummary,
}
