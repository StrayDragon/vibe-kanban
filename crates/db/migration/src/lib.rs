use sea_orm_migration::prelude::*;

mod m20250101000000_baseline;
mod m20250215000000_task_groups;
mod m20250220000000_task_group_entry_unique;
mod m20260227000000_idempotency_keys;
mod m20260302000000_project_git_no_verify_override;
mod m20260302000001_approvals;
mod m20260302000002_attempt_control_leases;
mod m20260303000000_mcp_tool_tasks;
mod m20260304000000_archived_kanbans;
mod m20260304000001_executor_protocol_strict;
mod m20260307000000_auto_orchestrator;
mod m20260307010000_task_automation_override;
mod m20260307020000_task_lineage_source;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250101000000_baseline::Migration),
            Box::new(m20250215000000_task_groups::Migration),
            Box::new(m20250220000000_task_group_entry_unique::Migration),
            Box::new(m20260227000000_idempotency_keys::Migration),
            Box::new(m20260302000000_project_git_no_verify_override::Migration),
            Box::new(m20260302000001_approvals::Migration),
            Box::new(m20260302000002_attempt_control_leases::Migration),
            Box::new(m20260303000000_mcp_tool_tasks::Migration),
            Box::new(m20260304000000_archived_kanbans::Migration),
            Box::new(m20260304000001_executor_protocol_strict::Migration),
            Box::new(m20260307000000_auto_orchestrator::Migration),
            Box::new(m20260307010000_task_automation_override::Migration),
            Box::new(m20260307020000_task_lineage_source::Migration),
        ]
    }
}
