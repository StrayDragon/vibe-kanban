use sea_orm_migration::prelude::*;

mod m20250101000000_baseline;
mod m20250215000000_task_groups;
mod m20250220000000_task_group_entry_unique;
mod m20260227000000_idempotency_keys;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250101000000_baseline::Migration),
            Box::new(m20250215000000_task_groups::Migration),
            Box::new(m20250220000000_task_group_entry_unique::Migration),
            Box::new(m20260227000000_idempotency_keys::Migration),
        ]
    }
}
