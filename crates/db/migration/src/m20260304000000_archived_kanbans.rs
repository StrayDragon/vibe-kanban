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
                    .table(ArchivedKanbans::Table)
                    .col(pk_id_col(manager, ArchivedKanbans::Id))
                    .col(uuid_col(ArchivedKanbans::Uuid))
                    .col(fk_id_col(manager, ArchivedKanbans::ProjectId))
                    .col(ColumnDef::new(ArchivedKanbans::Title).string().not_null())
                    .col(timestamp_col(ArchivedKanbans::CreatedAt))
                    .col(timestamp_col(ArchivedKanbans::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_archived_kanbans_project_id")
                            .from(ArchivedKanbans::Table, ArchivedKanbans::ProjectId)
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
                    .name("idx_archived_kanbans_uuid")
                    .table(ArchivedKanbans::Table)
                    .col(ArchivedKanbans::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_archived_kanbans_project_id")
                    .table(ArchivedKanbans::Table)
                    .col(ArchivedKanbans::ProjectId)
                    .to_owned(),
            )
            .await?;

        if matches!(manager.get_database_backend(), DatabaseBackend::Sqlite) {
            manager
                .alter_table(
                    Table::alter()
                        .table(Tasks::Table)
                        .add_column(fk_id_nullable_col(manager, Tasks::ArchivedKanbanId))
                        .to_owned(),
                )
                .await?;
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(Tasks::Table)
                        .add_column(fk_id_nullable_col(manager, Tasks::ArchivedKanbanId))
                        .add_foreign_key(
                            TableForeignKey::new()
                                .name("fk_tasks_archived_kanban_id")
                                .from_tbl(Tasks::Table)
                                .from_col(Tasks::ArchivedKanbanId)
                                .to_tbl(ArchivedKanbans::Table)
                                .to_col(ArchivedKanbans::Id)
                                .on_delete(ForeignKeyAction::Restrict),
                        )
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_tasks_archived_kanban_id")
                    .table(Tasks::Table)
                    .col(Tasks::ArchivedKanbanId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_tasks_archived_kanban_id")
                    .table(Tasks::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Tasks::Table)
                    .drop_foreign_key(Alias::new("fk_tasks_archived_kanban_id"))
                    .drop_column(Tasks::ArchivedKanbanId)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_archived_kanbans_project_id")
                    .table(ArchivedKanbans::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_archived_kanbans_uuid")
                    .table(ArchivedKanbans::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(ArchivedKanbans::Table).to_owned())
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
enum ArchivedKanbans {
    Table,
    Id,
    Uuid,
    ProjectId,
    Title,
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
    ArchivedKanbanId,
}
