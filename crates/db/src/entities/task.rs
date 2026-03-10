use sea_orm::entity::prelude::*;

use crate::types::{TaskCreatedByKind, TaskKind, TaskStatus};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "tasks")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub task_kind: TaskKind,
    pub milestone_id: Option<i64>,
    pub milestone_node_id: Option<String>,
    pub parent_workspace_id: Option<i64>,
    pub origin_task_id: Option<i64>,
    pub created_by_kind: TaskCreatedByKind,
    pub shared_task_id: Option<i64>,
    pub archived_kanban_id: Option<i64>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
