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
                    .table(AttemptControlLeases::Table)
                    .col(pk_id_col(manager, AttemptControlLeases::Id))
                    .col(uuid_col(AttemptControlLeases::Uuid))
                    .col(
                        ColumnDef::new(AttemptControlLeases::AttemptId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AttemptControlLeases::ControlToken)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AttemptControlLeases::ClaimedByClientId)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AttemptControlLeases::ExpiresAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(timestamp_col(AttemptControlLeases::CreatedAt))
                    .col(timestamp_col(AttemptControlLeases::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_attempt_control_leases_uuid")
                    .table(AttemptControlLeases::Table)
                    .col(AttemptControlLeases::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_attempt_control_leases_attempt_id")
                    .table(AttemptControlLeases::Table)
                    .col(AttemptControlLeases::AttemptId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_attempt_control_leases_expires_at")
                    .table(AttemptControlLeases::Table)
                    .col(AttemptControlLeases::ExpiresAt)
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
                    .table(AttemptControlLeases::Table)
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
enum AttemptControlLeases {
    Table,
    Id,
    Uuid,
    AttemptId,
    ControlToken,
    ClaimedByClientId,
    ExpiresAt,
    CreatedAt,
    UpdatedAt,
}
