use sea_orm_migration::{prelude::*, sea_orm::DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        if !matches!(backend, DatabaseBackend::Sqlite) {
            return Err(DbErr::Custom(
                "rename_task_groups_to_milestones only supports sqlite".to_string(),
            ));
        }

        // Rename the core table.
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE task_groups RENAME TO milestones;")
            .await?;

        // Rename linkage columns on tasks.
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE tasks RENAME COLUMN task_group_id TO milestone_id;",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE tasks RENAME COLUMN task_group_node_id TO milestone_node_id;",
            )
            .await?;

        // Rename task_kind value for milestone entry tasks.
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE tasks SET task_kind = 'milestone' WHERE task_kind = 'group';",
            )
            .await?;

        // Drop legacy index names so the schema reads cleanly.
        for sql in [
            "DROP INDEX IF EXISTS idx_task_groups_uuid;",
            "DROP INDEX IF EXISTS idx_task_groups_project_id;",
            "DROP INDEX IF EXISTS idx_task_groups_automation_run_next;",
            "DROP INDEX IF EXISTS idx_tasks_task_group_id;",
            "DROP INDEX IF EXISTS idx_tasks_task_group_node_id;",
            "DROP INDEX IF EXISTS idx_tasks_task_group_node_unique;",
            "DROP INDEX IF EXISTS idx_tasks_task_group_entry_unique;",
        ] {
            manager.get_connection().execute_unprepared(sql).await?;
        }

        // Recreate indexes under milestone names.
        for sql in [
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_milestones_uuid ON milestones (uuid);",
            "CREATE INDEX IF NOT EXISTS idx_milestones_project_id ON milestones (project_id);",
            "CREATE INDEX IF NOT EXISTS idx_milestones_automation_run_next ON milestones (automation_mode, run_next_step_requested_at);",
            "CREATE INDEX IF NOT EXISTS idx_tasks_milestone_id ON tasks (milestone_id);",
            "CREATE INDEX IF NOT EXISTS idx_tasks_milestone_node_id ON tasks (milestone_node_id);",
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_milestone_node_unique ON tasks (milestone_id, milestone_node_id);",
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_milestone_entry_unique ON tasks (milestone_id) WHERE task_kind = 'milestone' AND milestone_id IS NOT NULL;",
        ] {
            manager.get_connection().execute_unprepared(sql).await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        if !matches!(backend, DatabaseBackend::Sqlite) {
            return Err(DbErr::Custom(
                "rename_task_groups_to_milestones only supports sqlite".to_string(),
            ));
        }

        // Drop milestone index names.
        for sql in [
            "DROP INDEX IF EXISTS idx_milestones_uuid;",
            "DROP INDEX IF EXISTS idx_milestones_project_id;",
            "DROP INDEX IF EXISTS idx_milestones_automation_run_next;",
            "DROP INDEX IF EXISTS idx_tasks_milestone_id;",
            "DROP INDEX IF EXISTS idx_tasks_milestone_node_id;",
            "DROP INDEX IF EXISTS idx_tasks_milestone_node_unique;",
            "DROP INDEX IF EXISTS idx_tasks_milestone_entry_unique;",
        ] {
            manager.get_connection().execute_unprepared(sql).await?;
        }

        // Restore task_kind value.
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE tasks SET task_kind = 'group' WHERE task_kind = 'milestone';",
            )
            .await?;

        // Restore linkage column names on tasks.
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE tasks RENAME COLUMN milestone_node_id TO task_group_node_id;",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE tasks RENAME COLUMN milestone_id TO task_group_id;",
            )
            .await?;

        // Restore the core table name.
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE milestones RENAME TO task_groups;")
            .await?;

        // Recreate legacy indexes (best-effort).
        for sql in [
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_task_groups_uuid ON task_groups (uuid);",
            "CREATE INDEX IF NOT EXISTS idx_task_groups_project_id ON task_groups (project_id);",
            "CREATE INDEX IF NOT EXISTS idx_task_groups_automation_run_next ON task_groups (automation_mode, run_next_step_requested_at);",
            "CREATE INDEX IF NOT EXISTS idx_tasks_task_group_id ON tasks (task_group_id);",
            "CREATE INDEX IF NOT EXISTS idx_tasks_task_group_node_id ON tasks (task_group_node_id);",
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_task_group_node_unique ON tasks (task_group_id, task_group_node_id);",
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_task_group_entry_unique ON tasks (task_group_id) WHERE task_kind = 'group' AND task_group_id IS NOT NULL;",
        ] {
            manager.get_connection().execute_unprepared(sql).await?;
        }

        Ok(())
    }
}

