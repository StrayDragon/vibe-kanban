use sea_orm_migration::{prelude::*, sea_orm::DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        let sql = match backend {
            DatabaseBackend::Postgres | DatabaseBackend::Sqlite => {
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_task_group_entry_unique \
                 ON tasks (task_group_id) \
                 WHERE task_kind = 'group' AND task_group_id IS NOT NULL;"
            }
            _ => {
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_task_group_entry_unique \
                 ON tasks (task_group_id) \
                 WHERE task_kind = 'group' AND task_group_id IS NOT NULL;"
            }
        };

        manager.get_connection().execute_unprepared(sql).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS idx_tasks_task_group_entry_unique;")
            .await?;
        Ok(())
    }
}
