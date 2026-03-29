use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_event_outbox_published_at")
                    .table(EventOutbox::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_event_outbox_published_at_created_at_id")
                    .table(EventOutbox::Table)
                    .col(EventOutbox::PublishedAt)
                    .col(EventOutbox::CreatedAt)
                    .col(EventOutbox::Id)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_event_outbox_published_at_created_at_id")
                    .table(EventOutbox::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_event_outbox_published_at")
                    .table(EventOutbox::Table)
                    .col(EventOutbox::PublishedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum EventOutbox {
    Table,
    Id,
    CreatedAt,
    PublishedAt,
}
