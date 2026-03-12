use sea_orm::entity::prelude::*;

use crate::types::{WorkspaceLifecycleHookFailurePolicy, WorkspaceLifecycleHookRunMode};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "projects")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub dev_script: Option<String>,
    pub dev_script_working_dir: Option<String>,
    pub default_agent_working_dir: Option<String>,
    pub git_no_verify_override: Option<bool>,
    pub scheduler_max_concurrent: i32,
    pub scheduler_max_retries: i32,
    pub default_continuation_turns: i32,
    pub after_prepare_hook_command: Option<String>,
    pub after_prepare_hook_working_dir: Option<String>,
    pub after_prepare_hook_failure_policy: Option<WorkspaceLifecycleHookFailurePolicy>,
    pub after_prepare_hook_run_mode: Option<WorkspaceLifecycleHookRunMode>,
    pub before_cleanup_hook_command: Option<String>,
    pub before_cleanup_hook_working_dir: Option<String>,
    pub before_cleanup_hook_failure_policy: Option<WorkspaceLifecycleHookFailurePolicy>,
    pub remote_project_id: Option<Uuid>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
