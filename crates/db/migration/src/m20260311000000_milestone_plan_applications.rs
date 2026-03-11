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
                    .table(MilestonePlanApplications::Table)
                    .col(pk_id_col(manager, MilestonePlanApplications::Id))
                    .col(uuid_col(MilestonePlanApplications::Uuid))
                    .col(fk_id_col(manager, MilestonePlanApplications::MilestoneId))
                    .col(
                        ColumnDef::new(MilestonePlanApplications::SchemaVersion)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MilestonePlanApplications::PlanJson)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MilestonePlanApplications::AppliedByKind)
                            .string_len(32)
                            .not_null()
                            .default(Expr::val("human_ui")),
                    )
                    .col(ColumnDef::new(MilestonePlanApplications::IdempotencyKey).string())
                    .col(timestamp_col(MilestonePlanApplications::CreatedAt))
                    .col(timestamp_col(MilestonePlanApplications::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_milestone_plan_applications_milestone_id")
                            .from(
                                MilestonePlanApplications::Table,
                                MilestonePlanApplications::MilestoneId,
                            )
                            .to(Milestones::Table, Milestones::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Reasonable lookup patterns: latest applied plan per milestone.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_milestone_plan_applications_milestone_id_created_at")
                    .table(MilestonePlanApplications::Table)
                    .col(MilestonePlanApplications::MilestoneId)
                    .col(MilestonePlanApplications::CreatedAt)
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
                    .name("idx_milestone_plan_applications_milestone_id_created_at")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(MilestonePlanApplications::Table)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum MilestonePlanApplications {
    Table,
    Id,
    Uuid,
    MilestoneId,
    SchemaVersion,
    PlanJson,
    AppliedByKind,
    IdempotencyKey,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Milestones {
    Table,
    Id,
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
