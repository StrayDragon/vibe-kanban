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
                    .table(TaskGroups::Table)
                    .col(pk_id_col(manager, TaskGroups::Id))
                    .col(uuid_col(TaskGroups::Uuid))
                    .col(fk_id_col(manager, TaskGroups::ProjectId))
                    .col(ColumnDef::new(TaskGroups::Title).string().not_null())
                    .col(ColumnDef::new(TaskGroups::Description).text())
                    .col(
                        ColumnDef::new(TaskGroups::Status)
                            .string_len(32)
                            .not_null()
                            .default(Expr::val("todo")),
                    )
                    .col(ColumnDef::new(TaskGroups::BaselineRef).string().not_null())
                    .col(
                        ColumnDef::new(TaskGroups::SchemaVersion)
                            .integer()
                            .not_null()
                            .default(Expr::val(1)),
                    )
                    .col(ColumnDef::new(TaskGroups::GraphJson).json().not_null())
                    .col(timestamp_col(TaskGroups::CreatedAt))
                    .col(timestamp_col(TaskGroups::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_task_groups_project_id")
                            .from(TaskGroups::Table, TaskGroups::ProjectId)
                            .to(Projects::Table, Projects::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_task_groups_uuid")
                    .table(TaskGroups::Table)
                    .col(TaskGroups::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_task_groups_project_id")
                    .table(TaskGroups::Table)
                    .col(TaskGroups::ProjectId)
                    .to_owned(),
            )
            .await?;

        if matches!(manager.get_database_backend(), DatabaseBackend::Sqlite) {
            manager
                .alter_table(
                    Table::alter()
                        .table(Tasks::Table)
                        .add_column(
                            ColumnDef::new(Tasks::TaskKind)
                                .string_len(32)
                                .not_null()
                                .default(Expr::val("default")),
                        )
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(Tasks::Table)
                        .add_column(fk_id_nullable_col(manager, Tasks::TaskGroupId))
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(Tasks::Table)
                        .add_column(ColumnDef::new(Tasks::TaskGroupNodeId).string())
                        .to_owned(),
                )
                .await?;
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(Tasks::Table)
                        .add_column(
                            ColumnDef::new(Tasks::TaskKind)
                                .string_len(32)
                                .not_null()
                                .default(Expr::val("default")),
                        )
                        .add_column(fk_id_nullable_col(manager, Tasks::TaskGroupId))
                        .add_column(ColumnDef::new(Tasks::TaskGroupNodeId).string())
                        .add_foreign_key(
                            TableForeignKey::new()
                                .name("fk_tasks_task_group_id")
                                .from_tbl(Tasks::Table)
                                .from_col(Tasks::TaskGroupId)
                                .to_tbl(TaskGroups::Table)
                                .to_col(TaskGroups::Id)
                                .on_delete(ForeignKeyAction::SetNull),
                        )
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_tasks_task_group_id")
                    .table(Tasks::Table)
                    .col(Tasks::TaskGroupId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_tasks_task_group_node_id")
                    .table(Tasks::Table)
                    .col(Tasks::TaskGroupNodeId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_tasks_task_group_node_unique")
                    .table(Tasks::Table)
                    .col(Tasks::TaskGroupId)
                    .col(Tasks::TaskGroupNodeId)
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
                    .name("idx_tasks_task_group_node_unique")
                    .table(Tasks::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_tasks_task_group_node_id")
                    .table(Tasks::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_tasks_task_group_id")
                    .table(Tasks::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Tasks::Table)
                    .drop_foreign_key(Alias::new("fk_tasks_task_group_id"))
                    .drop_column(Tasks::TaskGroupNodeId)
                    .drop_column(Tasks::TaskGroupId)
                    .drop_column(Tasks::TaskKind)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_task_groups_project_id")
                    .table(TaskGroups::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_task_groups_uuid")
                    .table(TaskGroups::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(TaskGroups::Table).to_owned())
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

fn fk_id_col<T: Iden>(manager: &SchemaManager, col: T) -> ColumnDef {
    let mut col = ColumnDef::new(col);
    match manager.get_database_backend() {
        DatabaseBackend::Sqlite => {
            col.integer();
        }
        _ => {
            col.big_integer();
        }
    }
    col.not_null().to_owned()
}

fn fk_id_nullable_col<T: Iden>(manager: &SchemaManager, col: T) -> ColumnDef {
    let mut col = ColumnDef::new(col);
    match manager.get_database_backend() {
        DatabaseBackend::Sqlite => {
            col.integer();
        }
        _ => {
            col.big_integer();
        }
    }
    col.to_owned()
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
enum TaskGroups {
    Table,
    Id,
    Uuid,
    ProjectId,
    Title,
    Description,
    Status,
    BaselineRef,
    SchemaVersion,
    GraphJson,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Projects {
    Table,
    Id,
}

#[derive(Iden)]
enum Tasks {
    Table,
    TaskKind,
    TaskGroupId,
    TaskGroupNodeId,
}
