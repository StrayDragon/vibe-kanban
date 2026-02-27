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
                    .table(IdempotencyKeys::Table)
                    .col(pk_id_col(manager, IdempotencyKeys::Id))
                    .col(uuid_col(IdempotencyKeys::Uuid))
                    .col(
                        ColumnDef::new(IdempotencyKeys::Scope)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IdempotencyKeys::Key)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IdempotencyKeys::RequestHash)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IdempotencyKeys::State)
                            .string_len(32)
                            .not_null()
                            .default(Expr::val("in_progress")),
                    )
                    .col(ColumnDef::new(IdempotencyKeys::ResponseStatus).integer())
                    .col(ColumnDef::new(IdempotencyKeys::ResponseJson).text())
                    .col(timestamp_col(IdempotencyKeys::CreatedAt))
                    .col(timestamp_col(IdempotencyKeys::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_idempotency_keys_uuid")
                    .table(IdempotencyKeys::Table)
                    .col(IdempotencyKeys::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_idempotency_keys_scope_key")
                    .table(IdempotencyKeys::Table)
                    .col(IdempotencyKeys::Scope)
                    .col(IdempotencyKeys::Key)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_idempotency_keys_created_at")
                    .table(IdempotencyKeys::Table)
                    .col(IdempotencyKeys::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(IdempotencyKeys::Table).to_owned())
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
enum IdempotencyKeys {
    Table,
    Id,
    Uuid,
    Scope,
    Key,
    RequestHash,
    State,
    ResponseStatus,
    ResponseJson,
    CreatedAt,
    UpdatedAt,
}
