use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(Projects::Table)
                    .col(pk_id_col(manager, Projects::Id))
                    .col(uuid_col(Projects::Uuid))
                    .col(ColumnDef::new(Projects::Name).string().not_null())
                    .col(ColumnDef::new(Projects::DevScript).string())
                    .col(ColumnDef::new(Projects::DevScriptWorkingDir).string())
                    .col(ColumnDef::new(Projects::DefaultAgentWorkingDir).string())
                    .col(uuid_nullable_col(Projects::RemoteProjectId))
                    .col(timestamp_col(Projects::CreatedAt))
                    .col(timestamp_col(Projects::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_projects_uuid")
                    .table(Projects::Table)
                    .col(Projects::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_projects_remote_project_id")
                    .table(Projects::Table)
                    .col(Projects::RemoteProjectId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(SharedTasks::Table)
                    .col(pk_id_col(manager, SharedTasks::Id))
                    .col(uuid_col(SharedTasks::Uuid))
                    .col(uuid_col(SharedTasks::RemoteProjectId))
                    .col(ColumnDef::new(SharedTasks::Title).string().not_null())
                    .col(ColumnDef::new(SharedTasks::Description).text())
                    .col(
                        ColumnDef::new(SharedTasks::Status)
                            .string_len(32)
                            .not_null()
                            .default(Expr::val("todo")),
                    )
                    .col(uuid_nullable_col(SharedTasks::AssigneeUserId))
                    .col(ColumnDef::new(SharedTasks::AssigneeFirstName).string())
                    .col(ColumnDef::new(SharedTasks::AssigneeLastName).string())
                    .col(ColumnDef::new(SharedTasks::AssigneeUsername).string())
                    .col(
                        ColumnDef::new(SharedTasks::Version)
                            .integer()
                            .not_null()
                            .default(Expr::val(1)),
                    )
                    .col(ColumnDef::new(SharedTasks::LastEventSeq).integer())
                    .col(timestamp_col(SharedTasks::CreatedAt))
                    .col(timestamp_col(SharedTasks::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_shared_tasks_uuid")
                    .table(SharedTasks::Table)
                    .col(SharedTasks::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_shared_tasks_remote_project_id")
                    .table(SharedTasks::Table)
                    .col(SharedTasks::RemoteProjectId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_shared_tasks_status")
                    .table(SharedTasks::Table)
                    .col(SharedTasks::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(SharedActivityCursors::Table)
                    .col(pk_id_col(manager, SharedActivityCursors::Id))
                    .col(uuid_col(SharedActivityCursors::Uuid))
                    .col(uuid_col(SharedActivityCursors::RemoteProjectId))
                    .col(
                        ColumnDef::new(SharedActivityCursors::LastSeq)
                            .integer()
                            .not_null()
                            .default(Expr::val(0)),
                    )
                    .col(timestamp_col(SharedActivityCursors::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_shared_activity_cursors_uuid")
                    .table(SharedActivityCursors::Table)
                    .col(SharedActivityCursors::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_shared_activity_cursors_remote_project_id")
                    .table(SharedActivityCursors::Table)
                    .col(SharedActivityCursors::RemoteProjectId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(Tasks::Table)
                    .col(pk_id_col(manager, Tasks::Id))
                    .col(uuid_col(Tasks::Uuid))
                    .col(fk_id_col(manager, Tasks::ProjectId))
                    .col(ColumnDef::new(Tasks::Title).string().not_null())
                    .col(ColumnDef::new(Tasks::Description).text())
                    .col(
                        ColumnDef::new(Tasks::Status)
                            .string_len(32)
                            .not_null()
                            .default(Expr::val("todo")),
                    )
                    .col(fk_id_nullable_col(manager, Tasks::ParentWorkspaceId))
                    .col(fk_id_nullable_col(manager, Tasks::SharedTaskId))
                    .col(timestamp_col(Tasks::CreatedAt))
                    .col(timestamp_col(Tasks::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tasks_project_id")
                            .from(Tasks::Table, Tasks::ProjectId)
                            .to(Projects::Table, Projects::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tasks_shared_task_id")
                            .from(Tasks::Table, Tasks::SharedTaskId)
                            .to(SharedTasks::Table, SharedTasks::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_tasks_uuid")
                    .table(Tasks::Table)
                    .col(Tasks::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_tasks_project_id")
                    .table(Tasks::Table)
                    .col(Tasks::ProjectId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_tasks_status")
                    .table(Tasks::Table)
                    .col(Tasks::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_tasks_parent_workspace_id")
                    .table(Tasks::Table)
                    .col(Tasks::ParentWorkspaceId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(Repos::Table)
                    .col(pk_id_col(manager, Repos::Id))
                    .col(uuid_col(Repos::Uuid))
                    .col(ColumnDef::new(Repos::Path).string().not_null())
                    .col(ColumnDef::new(Repos::Name).string().not_null())
                    .col(ColumnDef::new(Repos::DisplayName).string().not_null())
                    .col(timestamp_col(Repos::CreatedAt))
                    .col(timestamp_col(Repos::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_repos_uuid")
                    .table(Repos::Table)
                    .col(Repos::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_repos_path")
                    .table(Repos::Table)
                    .col(Repos::Path)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(ProjectRepos::Table)
                    .col(pk_id_col(manager, ProjectRepos::Id))
                    .col(uuid_col(ProjectRepos::Uuid))
                    .col(fk_id_col(manager, ProjectRepos::ProjectId))
                    .col(fk_id_col(manager, ProjectRepos::RepoId))
                    .col(ColumnDef::new(ProjectRepos::SetupScript).text())
                    .col(ColumnDef::new(ProjectRepos::CleanupScript).text())
                    .col(ColumnDef::new(ProjectRepos::CopyFiles).text())
                    .col(
                        ColumnDef::new(ProjectRepos::ParallelSetupScript)
                            .boolean()
                            .not_null()
                            .default(Expr::val(false)),
                    )
                    .col(timestamp_col(ProjectRepos::CreatedAt))
                    .col(timestamp_col(ProjectRepos::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_project_repos_project_id")
                            .from(ProjectRepos::Table, ProjectRepos::ProjectId)
                            .to(Projects::Table, Projects::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_project_repos_repo_id")
                            .from(ProjectRepos::Table, ProjectRepos::RepoId)
                            .to(Repos::Table, Repos::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_project_repos_uuid")
                    .table(ProjectRepos::Table)
                    .col(ProjectRepos::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_project_repos_project_id")
                    .table(ProjectRepos::Table)
                    .col(ProjectRepos::ProjectId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_project_repos_repo_id")
                    .table(ProjectRepos::Table)
                    .col(ProjectRepos::RepoId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_project_repos_unique")
                    .table(ProjectRepos::Table)
                    .col(ProjectRepos::ProjectId)
                    .col(ProjectRepos::RepoId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(Images::Table)
                    .col(pk_id_col(manager, Images::Id))
                    .col(uuid_col(Images::Uuid))
                    .col(ColumnDef::new(Images::FilePath).string().not_null())
                    .col(ColumnDef::new(Images::OriginalName).string().not_null())
                    .col(ColumnDef::new(Images::MimeType).string())
                    .col(ColumnDef::new(Images::SizeBytes).big_integer().not_null())
                    .col(ColumnDef::new(Images::Hash).string().not_null())
                    .col(timestamp_col(Images::CreatedAt))
                    .col(timestamp_col(Images::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_images_uuid")
                    .table(Images::Table)
                    .col(Images::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_images_file_path")
                    .table(Images::Table)
                    .col(Images::FilePath)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_images_hash")
                    .table(Images::Table)
                    .col(Images::Hash)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(TaskImages::Table)
                    .col(pk_id_col(manager, TaskImages::Id))
                    .col(uuid_col(TaskImages::Uuid))
                    .col(fk_id_col(manager, TaskImages::TaskId))
                    .col(fk_id_col(manager, TaskImages::ImageId))
                    .col(timestamp_col(TaskImages::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_task_images_task_id")
                            .from(TaskImages::Table, TaskImages::TaskId)
                            .to(Tasks::Table, Tasks::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_task_images_image_id")
                            .from(TaskImages::Table, TaskImages::ImageId)
                            .to(Images::Table, Images::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_task_images_uuid")
                    .table(TaskImages::Table)
                    .col(TaskImages::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_task_images_task_id")
                    .table(TaskImages::Table)
                    .col(TaskImages::TaskId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_task_images_image_id")
                    .table(TaskImages::Table)
                    .col(TaskImages::ImageId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_task_images_unique")
                    .table(TaskImages::Table)
                    .col(TaskImages::TaskId)
                    .col(TaskImages::ImageId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(Tags::Table)
                    .col(pk_id_col(manager, Tags::Id))
                    .col(uuid_col(Tags::Uuid))
                    .col(ColumnDef::new(Tags::TagName).string().not_null())
                    .col(ColumnDef::new(Tags::Content).text().not_null())
                    .col(timestamp_col(Tags::CreatedAt))
                    .col(timestamp_col(Tags::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_tags_uuid")
                    .table(Tags::Table)
                    .col(Tags::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_tags_tag_name")
                    .table(Tags::Table)
                    .col(Tags::TagName)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(Workspaces::Table)
                    .col(pk_id_col(manager, Workspaces::Id))
                    .col(uuid_col(Workspaces::Uuid))
                    .col(fk_id_col(manager, Workspaces::TaskId))
                    .col(ColumnDef::new(Workspaces::ContainerRef).string())
                    .col(ColumnDef::new(Workspaces::Branch).string().not_null())
                    .col(ColumnDef::new(Workspaces::AgentWorkingDir).string())
                    .col(ColumnDef::new(Workspaces::SetupCompletedAt).timestamp())
                    .col(timestamp_col(Workspaces::CreatedAt))
                    .col(timestamp_col(Workspaces::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_workspaces_task_id")
                            .from(Workspaces::Table, Workspaces::TaskId)
                            .to(Tasks::Table, Tasks::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_workspaces_uuid")
                    .table(Workspaces::Table)
                    .col(Workspaces::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_workspaces_task_id")
                    .table(Workspaces::Table)
                    .col(Workspaces::TaskId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_workspaces_container_ref")
                    .table(Workspaces::Table)
                    .col(Workspaces::ContainerRef)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(WorkspaceRepos::Table)
                    .col(pk_id_col(manager, WorkspaceRepos::Id))
                    .col(uuid_col(WorkspaceRepos::Uuid))
                    .col(fk_id_col(manager, WorkspaceRepos::WorkspaceId))
                    .col(fk_id_col(manager, WorkspaceRepos::RepoId))
                    .col(ColumnDef::new(WorkspaceRepos::TargetBranch).string().not_null())
                    .col(timestamp_col(WorkspaceRepos::CreatedAt))
                    .col(timestamp_col(WorkspaceRepos::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_workspace_repos_workspace_id")
                            .from(WorkspaceRepos::Table, WorkspaceRepos::WorkspaceId)
                            .to(Workspaces::Table, Workspaces::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_workspace_repos_repo_id")
                            .from(WorkspaceRepos::Table, WorkspaceRepos::RepoId)
                            .to(Repos::Table, Repos::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_workspace_repos_uuid")
                    .table(WorkspaceRepos::Table)
                    .col(WorkspaceRepos::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_workspace_repos_workspace_id")
                    .table(WorkspaceRepos::Table)
                    .col(WorkspaceRepos::WorkspaceId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_workspace_repos_repo_id")
                    .table(WorkspaceRepos::Table)
                    .col(WorkspaceRepos::RepoId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_workspace_repos_unique")
                    .table(WorkspaceRepos::Table)
                    .col(WorkspaceRepos::WorkspaceId)
                    .col(WorkspaceRepos::RepoId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(Merges::Table)
                    .col(pk_id_col(manager, Merges::Id))
                    .col(uuid_col(Merges::Uuid))
                    .col(fk_id_col(manager, Merges::WorkspaceId))
                    .col(fk_id_col(manager, Merges::RepoId))
                    .col(ColumnDef::new(Merges::MergeType).string_len(16).not_null())
                    .col(ColumnDef::new(Merges::MergeCommit).string())
                    .col(ColumnDef::new(Merges::TargetBranchName).string().not_null())
                    .col(ColumnDef::new(Merges::PrNumber).big_integer())
                    .col(ColumnDef::new(Merges::PrUrl).string())
                    .col(ColumnDef::new(Merges::PrStatus).string_len(16))
                    .col(ColumnDef::new(Merges::PrMergedAt).timestamp())
                    .col(ColumnDef::new(Merges::PrMergeCommitSha).string())
                    .col(timestamp_col(Merges::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_merges_workspace_id")
                            .from(Merges::Table, Merges::WorkspaceId)
                            .to(Workspaces::Table, Workspaces::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_merges_repo_id")
                            .from(Merges::Table, Merges::RepoId)
                            .to(Repos::Table, Repos::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_merges_uuid")
                    .table(Merges::Table)
                    .col(Merges::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_merges_workspace_id")
                    .table(Merges::Table)
                    .col(Merges::WorkspaceId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_merges_repo_id")
                    .table(Merges::Table)
                    .col(Merges::RepoId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_merges_open_pr")
                    .table(Merges::Table)
                    .col(Merges::MergeType)
                    .col(Merges::PrStatus)
                    .col(Merges::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(Sessions::Table)
                    .col(pk_id_col(manager, Sessions::Id))
                    .col(uuid_col(Sessions::Uuid))
                    .col(fk_id_col(manager, Sessions::WorkspaceId))
                    .col(ColumnDef::new(Sessions::Executor).string())
                    .col(timestamp_col(Sessions::CreatedAt))
                    .col(timestamp_col(Sessions::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sessions_workspace_id")
                            .from(Sessions::Table, Sessions::WorkspaceId)
                            .to(Workspaces::Table, Workspaces::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_sessions_uuid")
                    .table(Sessions::Table)
                    .col(Sessions::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_sessions_workspace_id")
                    .table(Sessions::Table)
                    .col(Sessions::WorkspaceId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_sessions_created_at")
                    .table(Sessions::Table)
                    .col(Sessions::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(ExecutionProcesses::Table)
                    .col(pk_id_col(manager, ExecutionProcesses::Id))
                    .col(uuid_col(ExecutionProcesses::Uuid))
                    .col(fk_id_col(manager, ExecutionProcesses::SessionId))
                    .col(
                        ColumnDef::new(ExecutionProcesses::RunReason)
                            .string_len(32)
                            .not_null()
                            .default(Expr::val("setupscript")),
                    )
                    .col(
                        ColumnDef::new(ExecutionProcesses::ExecutorAction)
                            .json()
                            .not_null()
                            .default(Expr::val("{}")),
                    )
                    .col(
                        ColumnDef::new(ExecutionProcesses::Status)
                            .string_len(32)
                            .not_null()
                            .default(Expr::val("running")),
                    )
                    .col(ColumnDef::new(ExecutionProcesses::ExitCode).integer())
                    .col(
                        ColumnDef::new(ExecutionProcesses::Dropped)
                            .boolean()
                            .not_null()
                            .default(Expr::val(false)),
                    )
                    .col(timestamp_col(ExecutionProcesses::StartedAt))
                    .col(ColumnDef::new(ExecutionProcesses::CompletedAt).timestamp())
                    .col(timestamp_col(ExecutionProcesses::CreatedAt))
                    .col(timestamp_col(ExecutionProcesses::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_execution_processes_session_id")
                            .from(ExecutionProcesses::Table, ExecutionProcesses::SessionId)
                            .to(Sessions::Table, Sessions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_execution_processes_uuid")
                    .table(ExecutionProcesses::Table)
                    .col(ExecutionProcesses::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_execution_processes_session_id")
                    .table(ExecutionProcesses::Table)
                    .col(ExecutionProcesses::SessionId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_execution_processes_status")
                    .table(ExecutionProcesses::Table)
                    .col(ExecutionProcesses::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_execution_processes_run_reason")
                    .table(ExecutionProcesses::Table)
                    .col(ExecutionProcesses::RunReason)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_execution_processes_session_created_at")
                    .table(ExecutionProcesses::Table)
                    .col(ExecutionProcesses::SessionId)
                    .col(ExecutionProcesses::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(ExecutionProcessRepoStates::Table)
                    .col(pk_id_col(manager, ExecutionProcessRepoStates::Id))
                    .col(uuid_col(ExecutionProcessRepoStates::Uuid))
                    .col(fk_id_col(manager, ExecutionProcessRepoStates::ExecutionProcessId))
                    .col(fk_id_col(manager, ExecutionProcessRepoStates::RepoId))
                    .col(ColumnDef::new(ExecutionProcessRepoStates::BeforeHeadCommit).string())
                    .col(ColumnDef::new(ExecutionProcessRepoStates::AfterHeadCommit).string())
                    .col(ColumnDef::new(ExecutionProcessRepoStates::MergeCommit).string())
                    .col(timestamp_col(ExecutionProcessRepoStates::CreatedAt))
                    .col(timestamp_col(ExecutionProcessRepoStates::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_execution_process_repo_states_process_id")
                            .from(
                                ExecutionProcessRepoStates::Table,
                                ExecutionProcessRepoStates::ExecutionProcessId,
                            )
                            .to(ExecutionProcesses::Table, ExecutionProcesses::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_execution_process_repo_states_repo_id")
                            .from(ExecutionProcessRepoStates::Table, ExecutionProcessRepoStates::RepoId)
                            .to(Repos::Table, Repos::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_execution_process_repo_states_uuid")
                    .table(ExecutionProcessRepoStates::Table)
                    .col(ExecutionProcessRepoStates::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_execution_process_repo_states_process_id")
                    .table(ExecutionProcessRepoStates::Table)
                    .col(ExecutionProcessRepoStates::ExecutionProcessId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_execution_process_repo_states_repo_id")
                    .table(ExecutionProcessRepoStates::Table)
                    .col(ExecutionProcessRepoStates::RepoId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_execution_process_repo_states_unique")
                    .table(ExecutionProcessRepoStates::Table)
                    .col(ExecutionProcessRepoStates::ExecutionProcessId)
                    .col(ExecutionProcessRepoStates::RepoId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(CodingAgentTurns::Table)
                    .col(pk_id_col(manager, CodingAgentTurns::Id))
                    .col(uuid_col(CodingAgentTurns::Uuid))
                    .col(fk_id_col(manager, CodingAgentTurns::ExecutionProcessId))
                    .col(ColumnDef::new(CodingAgentTurns::AgentSessionId).string())
                    .col(ColumnDef::new(CodingAgentTurns::Prompt).text())
                    .col(ColumnDef::new(CodingAgentTurns::Summary).text())
                    .col(timestamp_col(CodingAgentTurns::CreatedAt))
                    .col(timestamp_col(CodingAgentTurns::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_coding_agent_turns_execution_process_id")
                            .from(CodingAgentTurns::Table, CodingAgentTurns::ExecutionProcessId)
                            .to(ExecutionProcesses::Table, ExecutionProcesses::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_coding_agent_turns_uuid")
                    .table(CodingAgentTurns::Table)
                    .col(CodingAgentTurns::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_coding_agent_turns_execution_process_id")
                    .table(CodingAgentTurns::Table)
                    .col(CodingAgentTurns::ExecutionProcessId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_coding_agent_turns_agent_session_id")
                    .table(CodingAgentTurns::Table)
                    .col(CodingAgentTurns::AgentSessionId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(TaskAttemptActivities::Table)
                    .col(pk_id_col(manager, TaskAttemptActivities::Id))
                    .col(uuid_col(TaskAttemptActivities::Uuid))
                    .col(fk_id_col(manager, TaskAttemptActivities::ExecutionProcessId))
                    .col(ColumnDef::new(TaskAttemptActivities::Status).string_len(32).not_null())
                    .col(ColumnDef::new(TaskAttemptActivities::Note).text())
                    .col(timestamp_col(TaskAttemptActivities::CreatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_task_attempt_activities_execution_process_id")
                            .from(
                                TaskAttemptActivities::Table,
                                TaskAttemptActivities::ExecutionProcessId,
                            )
                            .to(ExecutionProcesses::Table, ExecutionProcesses::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_task_attempt_activities_uuid")
                    .table(TaskAttemptActivities::Table)
                    .col(TaskAttemptActivities::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_task_attempt_activities_execution_process_id")
                    .table(TaskAttemptActivities::Table)
                    .col(TaskAttemptActivities::ExecutionProcessId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_task_attempt_activities_created_at")
                    .table(TaskAttemptActivities::Table)
                    .col(TaskAttemptActivities::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(Drafts::Table)
                    .col(pk_id_col(manager, Drafts::Id))
                    .col(uuid_col(Drafts::Uuid))
                    .col(fk_id_col(manager, Drafts::SessionId))
                    .col(ColumnDef::new(Drafts::DraftType).string_len(32).not_null())
                    .col(fk_id_nullable_col(manager, Drafts::RetryProcessId))
                    .col(
                        ColumnDef::new(Drafts::Prompt)
                            .text()
                            .not_null()
                            .default(Expr::val("")),
                    )
                    .col(
                        ColumnDef::new(Drafts::Queued)
                            .boolean()
                            .not_null()
                            .default(Expr::val(false)),
                    )
                    .col(
                        ColumnDef::new(Drafts::Sending)
                            .boolean()
                            .not_null()
                            .default(Expr::val(false)),
                    )
                    .col(
                        ColumnDef::new(Drafts::Version)
                            .integer()
                            .not_null()
                            .default(Expr::val(0)),
                    )
                    .col(ColumnDef::new(Drafts::Variant).string())
                    .col(ColumnDef::new(Drafts::ImageIds).json())
                    .col(timestamp_col(Drafts::CreatedAt))
                    .col(timestamp_col(Drafts::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_drafts_session_id")
                            .from(Drafts::Table, Drafts::SessionId)
                            .to(Sessions::Table, Sessions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_drafts_retry_process_id")
                            .from(Drafts::Table, Drafts::RetryProcessId)
                            .to(ExecutionProcesses::Table, ExecutionProcesses::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_drafts_uuid")
                    .table(Drafts::Table)
                    .col(Drafts::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_drafts_session_id")
                    .table(Drafts::Table)
                    .col(Drafts::SessionId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_drafts_draft_type")
                    .table(Drafts::Table)
                    .col(Drafts::DraftType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_drafts_unique")
                    .table(Drafts::Table)
                    .col(Drafts::SessionId)
                    .col(Drafts::DraftType)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_drafts_queued_sending")
                    .table(Drafts::Table)
                    .col(Drafts::Queued)
                    .col(Drafts::Sending)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(ExecutionProcessLogEntries::Table)
                    .col(pk_id_col(manager, ExecutionProcessLogEntries::Id))
                    .col(uuid_col(ExecutionProcessLogEntries::Uuid))
                    .col(fk_id_col(manager, ExecutionProcessLogEntries::ExecutionProcessId))
                    .col(
                        ColumnDef::new(ExecutionProcessLogEntries::Channel)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExecutionProcessLogEntries::EntryIndex)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExecutionProcessLogEntries::EntryJson)
                            .json()
                            .not_null(),
                    )
                    .col(timestamp_col(ExecutionProcessLogEntries::CreatedAt))
                    .col(timestamp_col(ExecutionProcessLogEntries::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_log_entries_execution_process_id")
                            .from(
                                ExecutionProcessLogEntries::Table,
                                ExecutionProcessLogEntries::ExecutionProcessId,
                            )
                            .to(ExecutionProcesses::Table, ExecutionProcesses::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_execution_process_log_entries_uuid")
                    .table(ExecutionProcessLogEntries::Table)
                    .col(ExecutionProcessLogEntries::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_execution_process_log_entries_unique")
                    .table(ExecutionProcessLogEntries::Table)
                    .col(ExecutionProcessLogEntries::ExecutionProcessId)
                    .col(ExecutionProcessLogEntries::Channel)
                    .col(ExecutionProcessLogEntries::EntryIndex)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_execution_process_log_entries_exec_channel_index")
                    .table(ExecutionProcessLogEntries::Table)
                    .col(ExecutionProcessLogEntries::ExecutionProcessId)
                    .col(ExecutionProcessLogEntries::Channel)
                    .col(ExecutionProcessLogEntries::EntryIndex)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(ExecutionProcessLogs::Table)
                    .col(pk_id_col(manager, ExecutionProcessLogs::Id))
                    .col(uuid_col(ExecutionProcessLogs::Uuid))
                    .col(fk_id_col(manager, ExecutionProcessLogs::ExecutionProcessId))
                    .col(ColumnDef::new(ExecutionProcessLogs::Logs).text().not_null())
                    .col(ColumnDef::new(ExecutionProcessLogs::ByteSize).big_integer().not_null())
                    .col(
                        ColumnDef::new(ExecutionProcessLogs::InsertedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_execution_process_logs_execution_id")
                            .from(ExecutionProcessLogs::Table, ExecutionProcessLogs::ExecutionProcessId)
                            .to(ExecutionProcesses::Table, ExecutionProcesses::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_execution_process_logs_uuid")
                    .table(ExecutionProcessLogs::Table)
                    .col(ExecutionProcessLogs::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_execution_process_logs_execution_id")
                    .table(ExecutionProcessLogs::Table)
                    .col(ExecutionProcessLogs::ExecutionProcessId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_execution_process_logs_inserted_at")
                    .table(ExecutionProcessLogs::Table)
                    .col(ExecutionProcessLogs::InsertedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(Scratch::Table)
                    .col(pk_id_col(manager, Scratch::Id))
                    .col(uuid_col(Scratch::Uuid))
                    .col(fk_id_col(manager, Scratch::SessionId))
                    .col(ColumnDef::new(Scratch::ScratchType).string_len(64).not_null())
                    .col(ColumnDef::new(Scratch::Payload).json().not_null())
                    .col(timestamp_col(Scratch::CreatedAt))
                    .col(timestamp_col(Scratch::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_scratch_session_id")
                            .from(Scratch::Table, Scratch::SessionId)
                            .to(Sessions::Table, Sessions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_scratch_uuid")
                    .table(Scratch::Table)
                    .col(Scratch::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_scratch_session_id")
                    .table(Scratch::Table)
                    .col(Scratch::SessionId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_scratch_session_type")
                    .table(Scratch::Table)
                    .col(Scratch::SessionId)
                    .col(Scratch::ScratchType)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create().if_not_exists()
                    .table(EventOutbox::Table)
                    .col(pk_id_col(manager, EventOutbox::Id))
                    .col(uuid_col(EventOutbox::Uuid))
                    .col(ColumnDef::new(EventOutbox::EventType).string_len(64).not_null())
                    .col(ColumnDef::new(EventOutbox::EntityType).string_len(64).not_null())
                    .col(ColumnDef::new(EventOutbox::EntityUuid).uuid().not_null())
                    .col(ColumnDef::new(EventOutbox::Payload).json().not_null())
                    .col(timestamp_col(EventOutbox::CreatedAt))
                    .col(ColumnDef::new(EventOutbox::PublishedAt).timestamp())
                    .col(
                        ColumnDef::new(EventOutbox::Attempts)
                            .integer()
                            .not_null()
                            .default(Expr::val(0)),
                    )
                    .col(ColumnDef::new(EventOutbox::LastError).text())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_event_outbox_uuid")
                    .table(EventOutbox::Table)
                    .col(EventOutbox::Uuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_event_outbox_published_at")
                    .table(EventOutbox::Table)
                    .col(EventOutbox::PublishedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create().if_not_exists()
                    .name("idx_event_outbox_entity_uuid")
                    .table(EventOutbox::Table)
                    .col(EventOutbox::EntityUuid)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(EventOutbox::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Scratch::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Drafts::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(TaskAttemptActivities::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CodingAgentTurns::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ExecutionProcessRepoStates::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ExecutionProcessLogs::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ExecutionProcessLogEntries::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ExecutionProcesses::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Merges::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(WorkspaceRepos::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Sessions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Workspaces::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(TaskImages::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Images::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ProjectRepos::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Repos::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Tags::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Tasks::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SharedActivityCursors::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SharedTasks::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Projects::Table).to_owned())
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

fn fk_id_col<T: Iden>(manager: &SchemaManager, col: T) -> ColumnDef {
    let mut col = ColumnDef::new(col);
    match manager.get_database_backend() {
        DatabaseBackend::Sqlite => {
            col.integer();
        }
        _ => {
            col.big_integer();
        }
    }
    col.not_null().to_owned()
}

fn fk_id_nullable_col<T: Iden>(manager: &SchemaManager, col: T) -> ColumnDef {
    let mut col = ColumnDef::new(col);
    match manager.get_database_backend() {
        DatabaseBackend::Sqlite => {
            col.integer();
        }
        _ => {
            col.big_integer();
        }
    }
    col.to_owned()
}

fn uuid_col<T: Iden>(col: T) -> ColumnDef {
    ColumnDef::new(col).uuid().not_null().to_owned()
}

fn uuid_nullable_col<T: Iden>(col: T) -> ColumnDef {
    ColumnDef::new(col).uuid().to_owned()
}

fn timestamp_col<T: Iden>(col: T) -> ColumnDef {
    ColumnDef::new(col)
        .timestamp()
        .not_null()
        .default(Expr::current_timestamp())
        .to_owned()
}

#[derive(Iden)]
enum Projects {
    Table,
    Id,
    Uuid,
    Name,
    DevScript,
    DevScriptWorkingDir,
    DefaultAgentWorkingDir,
    RemoteProjectId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum SharedTasks {
    Table,
    Id,
    Uuid,
    RemoteProjectId,
    Title,
    Description,
    Status,
    AssigneeUserId,
    AssigneeFirstName,
    AssigneeLastName,
    AssigneeUsername,
    Version,
    LastEventSeq,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum SharedActivityCursors {
    Table,
    Id,
    Uuid,
    RemoteProjectId,
    LastSeq,
    UpdatedAt,
}

#[derive(Iden)]
enum Tasks {
    Table,
    Id,
    Uuid,
    ProjectId,
    Title,
    Description,
    Status,
    ParentWorkspaceId,
    SharedTaskId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Repos {
    Table,
    Id,
    Uuid,
    Path,
    Name,
    DisplayName,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum ProjectRepos {
    Table,
    Id,
    Uuid,
    ProjectId,
    RepoId,
    SetupScript,
    CleanupScript,
    CopyFiles,
    ParallelSetupScript,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Workspaces {
    Table,
    Id,
    Uuid,
    TaskId,
    ContainerRef,
    Branch,
    AgentWorkingDir,
    SetupCompletedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum WorkspaceRepos {
    Table,
    Id,
    Uuid,
    WorkspaceId,
    RepoId,
    TargetBranch,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Sessions {
    Table,
    Id,
    Uuid,
    WorkspaceId,
    Executor,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum ExecutionProcesses {
    Table,
    Id,
    Uuid,
    SessionId,
    RunReason,
    ExecutorAction,
    Status,
    ExitCode,
    Dropped,
    StartedAt,
    CompletedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum ExecutionProcessRepoStates {
    Table,
    Id,
    Uuid,
    ExecutionProcessId,
    RepoId,
    BeforeHeadCommit,
    AfterHeadCommit,
    MergeCommit,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum CodingAgentTurns {
    Table,
    Id,
    Uuid,
    ExecutionProcessId,
    AgentSessionId,
    Prompt,
    Summary,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum TaskAttemptActivities {
    Table,
    Id,
    Uuid,
    ExecutionProcessId,
    Status,
    Note,
    CreatedAt,
}

#[derive(Iden)]
enum Drafts {
    Table,
    Id,
    Uuid,
    SessionId,
    DraftType,
    RetryProcessId,
    Prompt,
    Queued,
    Sending,
    Version,
    Variant,
    ImageIds,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum ExecutionProcessLogEntries {
    Table,
    Id,
    Uuid,
    ExecutionProcessId,
    Channel,
    EntryIndex,
    EntryJson,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum ExecutionProcessLogs {
    Table,
    Id,
    Uuid,
    ExecutionProcessId,
    Logs,
    ByteSize,
    InsertedAt,
}

#[derive(Iden)]
enum Merges {
    Table,
    Id,
    Uuid,
    WorkspaceId,
    RepoId,
    MergeType,
    MergeCommit,
    TargetBranchName,
    PrNumber,
    PrUrl,
    PrStatus,
    PrMergedAt,
    PrMergeCommitSha,
    CreatedAt,
}

#[derive(Iden)]
enum Images {
    Table,
    Id,
    Uuid,
    FilePath,
    OriginalName,
    MimeType,
    SizeBytes,
    Hash,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum TaskImages {
    Table,
    Id,
    Uuid,
    TaskId,
    ImageId,
    CreatedAt,
}

#[derive(Iden)]
enum Tags {
    Table,
    Id,
    Uuid,
    TagName,
    Content,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Scratch {
    Table,
    Id,
    Uuid,
    SessionId,
    ScratchType,
    Payload,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum EventOutbox {
    Table,
    Id,
    Uuid,
    EventType,
    EntityType,
    EntityUuid,
    Payload,
    CreatedAt,
    PublishedAt,
    Attempts,
    LastError,
}
