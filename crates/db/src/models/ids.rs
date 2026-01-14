use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QuerySelect};
use uuid::Uuid;

use crate::entities::{
    execution_process, image, merge, project, project_repo, repo, session, shared_task, task,
    workspace, workspace_repo,
};

pub async fn project_id_by_uuid<C: ConnectionTrait>(
    db: &C,
    uuid: Uuid,
) -> Result<Option<i64>, DbErr> {
    project::Entity::find()
        .select_only()
        .column(project::Column::Id)
        .filter(project::Column::Uuid.eq(uuid))
        .into_tuple()
        .one(db)
        .await
}

pub async fn project_uuid_by_id<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> Result<Option<Uuid>, DbErr> {
    project::Entity::find()
        .select_only()
        .column(project::Column::Uuid)
        .filter(project::Column::Id.eq(id))
        .into_tuple()
        .one(db)
        .await
}

pub async fn task_id_by_uuid<C: ConnectionTrait>(
    db: &C,
    uuid: Uuid,
) -> Result<Option<i64>, DbErr> {
    task::Entity::find()
        .select_only()
        .column(task::Column::Id)
        .filter(task::Column::Uuid.eq(uuid))
        .into_tuple()
        .one(db)
        .await
}

pub async fn task_uuid_by_id<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> Result<Option<Uuid>, DbErr> {
    task::Entity::find()
        .select_only()
        .column(task::Column::Uuid)
        .filter(task::Column::Id.eq(id))
        .into_tuple()
        .one(db)
        .await
}

pub async fn workspace_id_by_uuid<C: ConnectionTrait>(
    db: &C,
    uuid: Uuid,
) -> Result<Option<i64>, DbErr> {
    workspace::Entity::find()
        .select_only()
        .column(workspace::Column::Id)
        .filter(workspace::Column::Uuid.eq(uuid))
        .into_tuple()
        .one(db)
        .await
}

pub async fn workspace_uuid_by_id<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> Result<Option<Uuid>, DbErr> {
    workspace::Entity::find()
        .select_only()
        .column(workspace::Column::Uuid)
        .filter(workspace::Column::Id.eq(id))
        .into_tuple()
        .one(db)
        .await
}

pub async fn session_id_by_uuid<C: ConnectionTrait>(
    db: &C,
    uuid: Uuid,
) -> Result<Option<i64>, DbErr> {
    session::Entity::find()
        .select_only()
        .column(session::Column::Id)
        .filter(session::Column::Uuid.eq(uuid))
        .into_tuple()
        .one(db)
        .await
}

pub async fn session_uuid_by_id<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> Result<Option<Uuid>, DbErr> {
    session::Entity::find()
        .select_only()
        .column(session::Column::Uuid)
        .filter(session::Column::Id.eq(id))
        .into_tuple()
        .one(db)
        .await
}

pub async fn execution_process_id_by_uuid<C: ConnectionTrait>(
    db: &C,
    uuid: Uuid,
) -> Result<Option<i64>, DbErr> {
    execution_process::Entity::find()
        .select_only()
        .column(execution_process::Column::Id)
        .filter(execution_process::Column::Uuid.eq(uuid))
        .into_tuple()
        .one(db)
        .await
}

pub async fn execution_process_uuid_by_id<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> Result<Option<Uuid>, DbErr> {
    execution_process::Entity::find()
        .select_only()
        .column(execution_process::Column::Uuid)
        .filter(execution_process::Column::Id.eq(id))
        .into_tuple()
        .one(db)
        .await
}

pub async fn repo_id_by_uuid<C: ConnectionTrait>(
    db: &C,
    uuid: Uuid,
) -> Result<Option<i64>, DbErr> {
    repo::Entity::find()
        .select_only()
        .column(repo::Column::Id)
        .filter(repo::Column::Uuid.eq(uuid))
        .into_tuple()
        .one(db)
        .await
}

pub async fn repo_uuid_by_id<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> Result<Option<Uuid>, DbErr> {
    repo::Entity::find()
        .select_only()
        .column(repo::Column::Uuid)
        .filter(repo::Column::Id.eq(id))
        .into_tuple()
        .one(db)
        .await
}

pub async fn project_repo_id_by_uuid<C: ConnectionTrait>(
    db: &C,
    uuid: Uuid,
) -> Result<Option<i64>, DbErr> {
    project_repo::Entity::find()
        .select_only()
        .column(project_repo::Column::Id)
        .filter(project_repo::Column::Uuid.eq(uuid))
        .into_tuple()
        .one(db)
        .await
}

pub async fn workspace_repo_id_by_uuid<C: ConnectionTrait>(
    db: &C,
    uuid: Uuid,
) -> Result<Option<i64>, DbErr> {
    workspace_repo::Entity::find()
        .select_only()
        .column(workspace_repo::Column::Id)
        .filter(workspace_repo::Column::Uuid.eq(uuid))
        .into_tuple()
        .one(db)
        .await
}

pub async fn image_id_by_uuid<C: ConnectionTrait>(
    db: &C,
    uuid: Uuid,
) -> Result<Option<i64>, DbErr> {
    image::Entity::find()
        .select_only()
        .column(image::Column::Id)
        .filter(image::Column::Uuid.eq(uuid))
        .into_tuple()
        .one(db)
        .await
}

pub async fn image_uuid_by_id<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> Result<Option<Uuid>, DbErr> {
    image::Entity::find()
        .select_only()
        .column(image::Column::Uuid)
        .filter(image::Column::Id.eq(id))
        .into_tuple()
        .one(db)
        .await
}

pub async fn shared_task_id_by_uuid<C: ConnectionTrait>(
    db: &C,
    uuid: Uuid,
) -> Result<Option<i64>, DbErr> {
    shared_task::Entity::find()
        .select_only()
        .column(shared_task::Column::Id)
        .filter(shared_task::Column::Uuid.eq(uuid))
        .into_tuple()
        .one(db)
        .await
}

pub async fn shared_task_uuid_by_id<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> Result<Option<Uuid>, DbErr> {
    shared_task::Entity::find()
        .select_only()
        .column(shared_task::Column::Uuid)
        .filter(shared_task::Column::Id.eq(id))
        .into_tuple()
        .one(db)
        .await
}

pub async fn merge_id_by_uuid<C: ConnectionTrait>(
    db: &C,
    uuid: Uuid,
) -> Result<Option<i64>, DbErr> {
    merge::Entity::find()
        .select_only()
        .column(merge::Column::Id)
        .filter(merge::Column::Uuid.eq(uuid))
        .into_tuple()
        .one(db)
        .await
}

pub async fn merge_uuid_by_id<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> Result<Option<Uuid>, DbErr> {
    merge::Entity::find()
        .select_only()
        .column(merge::Column::Uuid)
        .filter(merge::Column::Id.eq(id))
        .into_tuple()
        .one(db)
        .await
}

#[cfg(test)]
mod tests {
    use sea_orm::Database;
    use sea_orm_migration::MigratorTrait;

    use crate::models::{
        project::{CreateProject, Project},
        task::{CreateTask, Task},
    };

    use super::*;

    async fn setup_db() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db_migration::Migrator::up(&db, None).await.unwrap();
        db
    }

    #[tokio::test]
    async fn ids_roundtrip_and_uuid_resolution() {
        let db = setup_db().await;

        let project_id = Uuid::new_v4();
        let project = Project::create(
            &db,
            &CreateProject {
                name: "Test project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();
        assert_eq!(project.id, project_id);

        let project_row_id = project_id_by_uuid(&db, project_id)
            .await
            .unwrap()
            .expect("project row id");
        assert_eq!(
            project_uuid_by_id(&db, project_row_id).await.unwrap(),
            Some(project_id)
        );

        let task_id = Uuid::new_v4();
        let task = Task::create(
            &db,
            &CreateTask::from_title_description(
                project_id,
                "Test task".to_string(),
                None,
            ),
            task_id,
        )
        .await
        .unwrap();
        assert_eq!(task.id, task_id);
        assert_eq!(task.project_id, project_id);

        let task_row_id = task_id_by_uuid(&db, task_id)
            .await
            .unwrap()
            .expect("task row id");
        assert_eq!(
            task_uuid_by_id(&db, task_row_id).await.unwrap(),
            Some(task_id)
        );

        let tasks = Task::find_by_project_id_with_attempt_status(&db, project_id)
            .await
            .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, task_id);
    }
}
