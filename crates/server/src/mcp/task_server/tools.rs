use rmcp::{tool, tool_router};

use super::*;

pub(super) fn build_tool_router() -> ToolRouter<TaskServer> {
    TaskServer::tool_router()
}

#[tool_router]
impl TaskServer {
    #[tool(
        description = r#"Use when: Quick environment preflight for external orchestrators.
Required: (none)
Optional: binaries[]
Next: list_projects / list_executors
Avoid: Using this as a health check for long-running processes."#,
        output_schema = tool_output_schema::<CliDependencyPreflightResponse>(),
        annotations(read_only_hint = true)
    )]
    async fn cli_dependency_preflight(
        &self,
        Parameters(CliDependencyPreflightRequest { binaries }): Parameters<
            CliDependencyPreflightRequest,
        >,
    ) -> Result<CallToolResult, ErrorData> {
        let default_bins = vec![
            "git".to_string(),
            "node".to_string(),
            "pnpm".to_string(),
            "cargo".to_string(),
            "docker".to_string(),
            "gh".to_string(),
        ];
        let bins = binaries.unwrap_or(default_bins);

        let mut checks = Vec::with_capacity(bins.len());
        for name in bins {
            let name_trim = name.trim().to_string();
            if name_trim.is_empty() {
                continue;
            }
            let output = std::process::Command::new(&name_trim)
                .arg("--version")
                .output();

            match output {
                Ok(out) if out.status.success() => {
                    let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    checks.push(CliDependencyCheck {
                        name: name_trim,
                        ok: true,
                        version: Some(version).filter(|v| !v.is_empty()),
                        error: None,
                    });
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    checks.push(CliDependencyCheck {
                        name: name_trim,
                        ok: false,
                        version: None,
                        error: Some(stderr).filter(|v| !v.is_empty()),
                    });
                }
                Err(err) => {
                    checks.push(CliDependencyCheck {
                        name: name_trim,
                        ok: false,
                        version: None,
                        error: Some(err.to_string()),
                    });
                }
            }
        }

        let all_ok = checks.iter().all(|c| c.ok);
        Self::success(&CliDependencyPreflightResponse { all_ok, checks })
    }

    #[tool(
        description = r#"Use when: Discover project_id values.
Required: (none)
Optional: (none)
Next: list_tasks, list_repos
Avoid: Guessing UUIDs."#,
        output_schema = tool_output_schema::<ListProjectsResponse>(),
        annotations(read_only_hint = true)
    )]
    async fn list_projects(&self) -> Result<Json<ListProjectsResponse>, ErrorData> {
        let projects = Project::find_all(&self.deployment.db().pool)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to list projects",
                    Some(json!({ "error": e.to_string() })),
                )
            })?;
        let summaries = projects
            .into_iter()
            .map(ProjectSummary::from_project)
            .collect::<Vec<_>>();
        Ok(Json(ListProjectsResponse {
            count: summaries.len(),
            projects: summaries,
        }))
    }

    #[tool(
        description = r#"Use when: Get repo_id + names for a project.
Required: project_id
Optional: (none)
Next: start_attempt
Avoid: Passing a task_id/attempt_id instead of project_id."#,
        output_schema = tool_output_schema::<ListReposResponse>(),
        annotations(read_only_hint = true)
    )]
    async fn list_repos(
        &self,
        Parameters(ListReposRequest { project_id }): Parameters<ListReposRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let repos = ProjectRepo::find_repos_for_project(&self.deployment.db().pool, project_id)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to list repos",
                    Some(json!({ "error": e.to_string(), "project_id": project_id })),
                )
            })?;
        let summaries = repos
            .into_iter()
            .map(|r| McpRepoSummary {
                id: r.id.to_string(),
                name: r.name,
            })
            .collect::<Vec<_>>();
        Self::success(&ListReposResponse {
            count: summaries.len(),
            repos: summaries,
            project_id: project_id.to_string(),
        })
    }

    #[tool(
        description = r#"Use when: Discover valid executor ids + variants for start_attempt.
Required: (none)
Optional: (none)
Next: start_attempt
Avoid: Guessing executor names; passing DEFAULT as a variant (omit variant instead)."#,
        output_schema = tool_output_schema::<ListExecutorsResponse>(),
        annotations(read_only_hint = true)
    )]
    async fn list_executors(&self) -> Result<CallToolResult, ErrorData> {
        let configs = executors::profile::ExecutorConfigs::get_cached();
        let mut executors = Vec::with_capacity(configs.executors.len());

        for (executor, config) in &configs.executors {
            let mut variants: Vec<String> = config
                .variant_names()
                .into_iter()
                .map(|name| name.to_string())
                .collect();
            variants.sort();

            let supports_mcp = config
                .get_default()
                .map(|a| a.supports_mcp())
                .unwrap_or(false);

            executors.push(McpExecutorSummary {
                executor: executor.to_string(),
                variants,
                supports_mcp,
                default_variant: None,
            });
        }

        executors.sort_by(|a, b| a.executor.cmp(&b.executor));

        Self::success(&ListExecutorsResponse {
            count: executors.len(),
            executors,
        })
    }

    #[tool(
        description = r#"Use when: List tasks in a project (includes latest attempt/session summary fields).
Required: project_id
Optional: status, limit
Next: get_task, start_attempt, list_task_attempts
Avoid: Using this as an attempt/session listing (use list_task_attempts)."#,
        output_schema = tool_output_schema::<ListTasksResponse>(),
        annotations(read_only_hint = true)
    )]
    async fn list_tasks(
        &self,
        Parameters(ListTasksRequest {
            project_id,
            status,
            limit,
        }): Parameters<ListTasksRequest>,
    ) -> Result<Json<ListTasksResponse>, ErrorData> {
        let status_filter = if let Some(ref status_str) = status {
            let trimmed = status_str.trim();
            if trimmed.is_empty() {
                None
            } else {
                match TaskStatus::from_str(trimmed) {
                    Ok(s) => Some(s),
                    Err(_) => {
                        let mut details = serde_json::Map::new();
                        details.insert("tool".to_string(), json!("list_tasks"));
                        details.insert("path".to_string(), json!("status"));
                        details.insert("value".to_string(), json!(trimmed));
                        details.insert(
                            "valid_values".to_string(),
                            json!(["todo", "inprogress", "inreview", "done", "cancelled"]),
                        );
                        details.insert("next_tools".to_string(), json!([]));
                        details.insert(
                            "example_args".to_string(),
                            json!({ "project_id": project_id, "status": "todo" }),
                        );

                        return Err(ErrorData::invalid_params(
                            "Invalid status filter",
                            Some(crate::mcp::params::invalid_params_payload(
                                "invalid_argument",
                                "Valid values: todo, inprogress, inreview, done, cancelled."
                                    .to_string(),
                                details,
                            )),
                        ));
                    }
                }
            }
        } else {
            None
        };

        let all_tasks: Vec<TaskWithAttemptStatus> =
            Task::find_by_project_id_with_attempt_status(&self.deployment.db().pool, project_id)
                .await
                .map_err(|e| {
                    ErrorData::internal_error(
                        "Failed to list tasks",
                        Some(json!({ "error": e.to_string(), "project_id": project_id })),
                    )
                })?;

        let task_limit = limit.unwrap_or(50).max(0) as usize;
        let filtered = all_tasks.into_iter().filter(|t| {
            if let Some(ref want) = status_filter {
                &t.status == want
            } else {
                true
            }
        });
        let limited: Vec<TaskWithAttemptStatus> = filtered.take(task_limit).collect();

        let task_ids: Vec<Uuid> = limited.iter().map(|task| task.id).collect();
        let summaries = self.task_attempt_summaries(task_ids).await.map_err(|e| {
            ErrorData::internal_error(
                "Failed to compute attempt summaries",
                Some(json!({ "error": e.to_string() })),
            )
        })?;

        let mut task_summaries = Vec::with_capacity(limited.len());
        for task in limited {
            let attempt_summary = summaries.get(&task.id).cloned().unwrap_or_default();
            task_summaries.push(TaskSummary::from_task_with_status(task, attempt_summary));
        }

        Ok(Json(ListTasksResponse {
            count: task_summaries.len(),
            tasks: task_summaries,
            project_id: project_id.to_string(),
        }))
    }

    #[tool(
        description = r#"Use when: List archived kanban batches for a project.
Required: project_id
Optional: (none)
Next: archive_project_kanban / restore_archived_kanban
Avoid: Guessing project_id (use list_projects)."#,
        output_schema = tool_output_schema::<ListArchivedKanbansResponse>(),
        annotations(read_only_hint = true)
    )]
    async fn list_archived_kanbans(
        &self,
        Parameters(ListArchivedKanbansRequest { project_id }): Parameters<
            ListArchivedKanbansRequest,
        >,
    ) -> Result<Json<ListArchivedKanbansResponse>, ErrorData> {
        let pool = &self.deployment.db().pool;
        let project = Project::find_by_id(pool, project_id).await.map_err(|e| {
            ErrorData::internal_error(
                "Failed to load project",
                Some(json!({ "error": e.to_string(), "project_id": project_id })),
            )
        })?;
        if project.is_none() {
            return Err(ErrorData::invalid_params(
                "Project not found",
                Some(json!({ "project_id": project_id })),
            ));
        }

        let archives = ArchivedKanban::list_by_project_with_task_counts(pool, project_id)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to list archived kanbans",
                    Some(json!({ "error": e.to_string(), "project_id": project_id })),
                )
            })?;

        let archived_kanbans: Vec<McpArchivedKanban> = archives
            .into_iter()
            .map(McpArchivedKanban::from_model)
            .collect();

        Ok(Json(ListArchivedKanbansResponse {
            project_id: project_id.to_string(),
            count: archived_kanbans.len(),
            archived_kanbans,
        }))
    }

    #[tool(
        description = r#"Use when: Archive a project's kanban by moving tasks with selected statuses into a new archived kanban batch.
Required: project_id, statuses
Optional: title
Next: list_archived_kanbans / restore_archived_kanban
Avoid: Archiving tasks with running execution processes."#,
        output_schema = tool_output_schema::<ArchiveProjectKanbanResponse>(),
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false
        ),
        execution(task_support = "optional")
    )]
    async fn archive_project_kanban(
        &self,
        Parameters(ArchiveProjectKanbanRequest {
            project_id,
            statuses,
            title,
        }): Parameters<ArchiveProjectKanbanRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let pool = &self.deployment.db().pool;
        let project = Project::find_by_id(pool, project_id)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to load project",
                    Some(json!({ "error": e.to_string(), "project_id": project_id })),
                )
            })?
            .ok_or_else(|| {
                ErrorData::invalid_params(
                    "Project not found",
                    Some(json!({ "project_id": project_id })),
                )
            })?;

        let mut parsed_statuses = Vec::new();
        for raw in statuses {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            let normalized = trimmed.to_lowercase();
            match TaskStatus::from_str(&normalized) {
                Ok(status) => parsed_statuses.push(status),
                Err(_) => {
                    return Self::err_with(
                        "Invalid task status.",
                        Some(json!({
                            "tool": "archive_project_kanban",
                            "path": "statuses",
                            "value": trimmed,
                            "valid_values": ["todo", "inprogress", "inreview", "done", "cancelled"],
                        })),
                        Some(
                            "Valid values: todo, inprogress, inreview, done, cancelled."
                                .to_string(),
                        ),
                        Some("invalid_argument"),
                        Some(false),
                    );
                }
            }
        }

        if parsed_statuses.is_empty() {
            return Self::err_with(
                "At least one status is required.",
                Some(json!({ "tool": "archive_project_kanban", "path": "statuses" })),
                Some("Provide one or more statuses (e.g. done, cancelled).".to_string()),
                Some("missing_required"),
                Some(false),
            );
        }

        let req = crate::routes::archived_kanbans::ArchiveProjectKanbanRequest {
            statuses: parsed_statuses,
            title,
        };

        let ResponseJson(response) = match crate::routes::archived_kanbans::archive_project_kanban(
            axum::Extension(project),
            axum::extract::State(self.deployment.clone()),
            axum::Json(req),
        )
        .await
        {
            Ok(ok) => ok,
            Err(err) => {
                return Self::tool_error_from_api_error(
                    "archive_project_kanban",
                    err,
                    json!({ "tool": "archive_project_kanban", "project_id": project_id }),
                );
            }
        };

        let data = response.into_data().ok_or_else(|| {
            ErrorData::internal_error(
                "Archive response missing data",
                Some(json!({ "project_id": project_id })),
            )
        })?;

        Self::success(&ArchiveProjectKanbanResponse {
            archived_kanban: McpArchivedKanban::from_model(data.archived_kanban),
            moved_task_count: data.moved_task_count,
        })
    }

    #[tool(
        description = r#"Use when: Restore tasks from an archived kanban back to the active set.
Required: archive_id
Optional: restore_all, statuses
Next: list_tasks / list_archived_kanbans
Avoid: Providing statuses together with restore_all=true."#,
        output_schema = tool_output_schema::<RestoreArchivedKanbanResponse>(),
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true
        ),
        execution(task_support = "optional")
    )]
    async fn restore_archived_kanban(
        &self,
        Parameters(RestoreArchivedKanbanRequest {
            archive_id,
            restore_all,
            statuses,
        }): Parameters<RestoreArchivedKanbanRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let restore_all = restore_all.unwrap_or(false);

        let mut parsed_statuses = Vec::new();
        if let Some(statuses) = statuses {
            for raw in statuses {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let normalized = trimmed.to_lowercase();
                match TaskStatus::from_str(&normalized) {
                    Ok(status) => parsed_statuses.push(status),
                    Err(_) => {
                        return Self::err_with(
                            "Invalid task status.",
                            Some(json!({
                                "tool": "restore_archived_kanban",
                                "path": "statuses",
                                "value": trimmed,
                                "valid_values": ["todo", "inprogress", "inreview", "done", "cancelled"],
                            })),
                            Some(
                                "Valid values: todo, inprogress, inreview, done, cancelled."
                                    .to_string(),
                            ),
                            Some("invalid_argument"),
                            Some(false),
                        );
                    }
                }
            }
        }

        if restore_all && !parsed_statuses.is_empty() {
            return Self::err_with(
                "Do not provide statuses when restore_all=true.",
                Some(json!({ "tool": "restore_archived_kanban" })),
                Some(
                    "Either set restore_all=true, or provide statuses for a partial restore."
                        .to_string(),
                ),
                Some("invalid_argument"),
                Some(false),
            );
        }

        if !restore_all && parsed_statuses.is_empty() {
            return Self::err_with(
                "At least one status is required when restore_all=false.",
                Some(json!({ "tool": "restore_archived_kanban", "path": "statuses" })),
                Some("Provide one or more statuses, or set restore_all=true.".to_string()),
                Some("missing_required"),
                Some(false),
            );
        }

        let req = crate::routes::archived_kanbans::RestoreArchivedKanbanRequest {
            restore_all: Some(restore_all),
            statuses: if restore_all {
                None
            } else {
                Some(parsed_statuses)
            },
        };

        let ResponseJson(response) = match crate::routes::archived_kanbans::restore_archived_kanban(
            axum::extract::State(self.deployment.clone()),
            axum::extract::Path(archive_id),
            axum::Json(req),
        )
        .await
        {
            Ok(ok) => ok,
            Err(err) => {
                return Self::tool_error_from_api_error(
                    "restore_archived_kanban",
                    err,
                    json!({ "tool": "restore_archived_kanban", "archive_id": archive_id }),
                );
            }
        };

        let data = response.into_data().ok_or_else(|| {
            ErrorData::internal_error(
                "Restore response missing data",
                Some(json!({ "archive_id": archive_id })),
            )
        })?;

        Self::success(&RestoreArchivedKanbanResponse {
            archive_id: archive_id.to_string(),
            restored_task_count: data.restored_task_count,
        })
    }

    #[tool(
        description = r#"Use when: Fetch full task details (title/description/status).
Required: task_id
Optional: (none)
Next: update_task, start_attempt
Avoid: Expecting attempt/session info here (use list_tasks/list_task_attempts)."#,
        output_schema = tool_output_schema::<GetTaskResponse>(),
        annotations(read_only_hint = true)
    )]
    async fn get_task(
        &self,
        Parameters(GetTaskRequest { task_id }): Parameters<GetTaskRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let task = Task::find_by_id(&self.deployment.db().pool, task_id)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to load task",
                    Some(json!({ "error": e.to_string() })),
                )
            })?
            .ok_or_else(|| {
                ErrorData::invalid_params("Task not found", Some(json!({ "task_id": task_id })))
            })?;
        Self::success(&GetTaskResponse {
            task: McpTask::from_task(task),
        })
    }

    #[tool(
        description = r#"Use when: Create a new task/ticket in a project.
Required: project_id, title
Optional: description, request_id
Next: start_attempt
Avoid: Empty title; guessing project_id (use list_projects)."#,
        output_schema = tool_output_schema::<CreateTaskResponse>(),
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true
        )
    )]
    async fn create_task(
        &self,
        Parameters(CreateTaskRequest {
            project_id,
            title,
            description,
            request_id,
        }): Parameters<CreateTaskRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let title = title.trim();
        if title.is_empty() {
            return Self::err_with(
                "Title must not be empty.",
                None,
                Some("Provide a task title.".to_string()),
                Some("missing_required"),
                None,
            );
        }
        let title = title.to_string();

        let expanded_description = match description {
            Some(desc) => Some(self.expand_tags(&desc).await),
            None => None,
        };

        let payload = CreateTask::from_title_description(project_id, title, expanded_description);
        let request_hash = Self::request_hash(&payload)?;
        let key = Self::stable_tool_idempotency_key(request_id);

        let task_id = match self
            .idempotent("create_task", key, request_hash, || async {
                let id = Uuid::new_v4();
                Task::create(&self.deployment.db().pool, &payload, id)
                    .await
                    .map_err(|e| {
                        ErrorData::internal_error(
                            "Failed to create task",
                            Some(json!({ "error": e.to_string(), "project_id": project_id })),
                        )
                    })?;
                Ok(CreateTaskResponse {
                    task_id: id.to_string(),
                })
            })
            .await
        {
            Ok(task_id) => task_id,
            Err(ToolOrRpcError::Tool(tool_error)) => return Ok(tool_error),
            Err(ToolOrRpcError::Rpc(err)) => return Err(err),
        };

        Self::success(&task_id)
    }

    #[tool(
        description = r#"Use when: Update a task's title/description/status.
Required: task_id
Optional: title, description, status
Next: get_task, start_attempt
Avoid: Calling this just to set status=inprogress (start_attempt already does that)."#,
        output_schema = tool_output_schema::<UpdateTaskResponse>(),
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true
        )
    )]
    async fn update_task(
        &self,
        Parameters(UpdateTaskRequest {
            task_id,
            title,
            description,
            status,
        }): Parameters<UpdateTaskRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let pool = &self.deployment.db().pool;
        let existing = Task::find_by_id(pool, task_id)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to load task",
                    Some(json!({ "error": e.to_string(), "task_id": task_id })),
                )
            })?
            .ok_or_else(|| {
                ErrorData::invalid_params("Task not found", Some(json!({ "task_id": task_id })))
            })?;

        if let Some(archive_id) = existing.archived_kanban_id {
            return Self::err_with(
                "Task is archived. Restore it before editing.",
                Some(json!({
                    "tool": "update_task",
                    "task_id": task_id,
                    "archived_kanban_id": archive_id,
                })),
                Some("Restore the archived kanban batch first, then retry.".to_string()),
                Some(MCP_CODE_BLOCKED_GUARDRAILS),
                Some(false),
            );
        }

        let status = status.and_then(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
        let status = if let Some(status) = status {
            Some(TaskStatus::from_str(&status).map_err(|_| {
                let mut details = serde_json::Map::new();
                details.insert("tool".to_string(), json!("update_task"));
                details.insert("path".to_string(), json!("status"));
                details.insert("value".to_string(), json!(status));
                details.insert(
                    "valid_values".to_string(),
                    json!(["todo", "inprogress", "inreview", "done", "cancelled"]),
                );
                details.insert("next_tools".to_string(), json!([]));
                details.insert(
                    "example_args".to_string(),
                    json!({ "task_id": task_id, "status": "todo" }),
                );

                ErrorData::invalid_params(
                    "Invalid task status",
                    Some(crate::mcp::params::invalid_params_payload(
                        "invalid_argument",
                        "Valid values: todo, inprogress, inreview, done, cancelled.".to_string(),
                        details,
                    )),
                )
            })?)
        } else {
            None
        };

        let title = title.and_then(|t| {
            let trimmed = t.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        let description = description.map(|d| d.trim().to_string());
        let parent_workspace_id = existing.parent_workspace_id;

        Task::update(
            pool,
            existing.id,
            existing.project_id,
            title.unwrap_or(existing.title),
            description.or(existing.description),
            status.unwrap_or(existing.status),
            parent_workspace_id,
        )
        .await
        .map_err(|e| {
            ErrorData::internal_error(
                "Failed to update task",
                Some(json!({ "error": e.to_string() })),
            )
        })?;

        Self::success(&UpdateTaskResponse {
            task_id: task_id.to_string(),
        })
    }

    #[tool(
        description = r#"Use when: Permanently delete a task/ticket.
Required: task_id
Optional: (none)
Next: list_tasks
Avoid: Deleting the wrong task (confirm with get_task first)."#,
        output_schema = tool_output_schema::<DeleteTaskResponse>(),
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true
        )
    )]
    async fn delete_task(
        &self,
        Parameters(DeleteTaskRequest { task_id }): Parameters<DeleteTaskRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let pool = &self.deployment.db().pool;
        if let Some(task) = Task::find_by_id(pool, task_id).await.map_err(|e| {
            ErrorData::internal_error(
                "Failed to load task",
                Some(json!({ "error": e.to_string(), "task_id": task_id })),
            )
        })? && let Some(archive_id) = task.archived_kanban_id
        {
            return Self::err_with(
                "Task is archived. Delete its archive to remove it.",
                Some(json!({
                    "tool": "delete_task",
                    "task_id": task_id,
                    "archived_kanban_id": archive_id,
                })),
                Some(
                    "Delete the archived kanban batch instead, or restore then delete.".to_string(),
                ),
                Some(MCP_CODE_BLOCKED_GUARDRAILS),
                Some(false),
            );
        }

        let rows = Task::delete(pool, task_id).await.map_err(|e| {
            ErrorData::internal_error(
                "Failed to delete task",
                Some(json!({ "error": e.to_string() })),
            )
        })?;
        let deleted_task_id = if rows > 0 {
            Some(task_id.to_string())
        } else {
            None
        };
        Self::success(&DeleteTaskResponse { deleted_task_id })
    }

    #[tool(
        description = r#"Use when: List attempts for a task (workspace history).
Required: task_id
Optional: (none)
Next: tail_attempt_feed, send_follow_up, stop_attempt
Avoid: Assuming a task always has an attempt."#,
        output_schema = tool_output_schema::<ListTaskAttemptsResponse>(),
        annotations(read_only_hint = true)
    )]
    async fn list_task_attempts(
        &self,
        Parameters(ListTaskAttemptsRequest { task_id }): Parameters<ListTaskAttemptsRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let pool = &self.deployment.db().pool;
        let workspaces = Workspace::fetch_all(pool, Some(task_id))
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to list workspaces",
                    Some(json!({ "error": e.to_string(), "task_id": task_id })),
                )
            })?;
        let workspace_ids: Vec<Uuid> = workspaces.iter().map(|w| w.id).collect();
        let sessions_by_workspace = Session::find_latest_by_workspace_ids(pool, &workspace_ids)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to list sessions",
                    Some(json!({ "error": e.to_string(), "task_id": task_id })),
                )
            })?;

        let mut attempts = Vec::with_capacity(workspaces.len());
        for ws in &workspaces {
            let session = sessions_by_workspace.get(&ws.id);
            attempts.push(AttemptSummary {
                attempt_id: ws.id.to_string(),
                workspace_branch: ws.branch.clone(),
                created_at: ws.created_at.to_rfc3339(),
                updated_at: ws.updated_at.to_rfc3339(),
                latest_session_id: session.map(|s| s.id.to_string()),
                latest_session_executor: session.and_then(|s| s.executor.clone()),
            });
        }

        let latest_attempt_id = workspaces.first().map(|w| w.id.to_string());
        let latest_session_id = latest_attempt_id
            .as_ref()
            .and_then(|attempt_id| Uuid::parse_str(attempt_id).ok())
            .and_then(|id| sessions_by_workspace.get(&id))
            .map(|s| s.id.to_string());

        Self::success(&ListTaskAttemptsResponse {
            task_id: task_id.to_string(),
            count: attempts.len(),
            attempts,
            latest_attempt_id,
            latest_session_id,
        })
    }

    #[tool(
        description = r#"Use when: Create a new attempt/workspace for a task and start the executor.
Required: task_id, executor, repos
Optional: variant, request_id, prompt
Next: tail_attempt_feed, send_follow_up, claim_attempt_control
Avoid: Empty repos; guessing executor (use list_executors)."#,
        output_schema = tool_output_schema::<StartAttemptResponse>(),
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true
        ),
        execution(task_support = "optional")
    )]
    async fn start_attempt(
        &self,
        Parameters(StartAttemptRequest {
            task_id,
            executor,
            variant,
            repos,
            request_id,
            prompt,
        }): Parameters<StartAttemptRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let pool = &self.deployment.db().pool;
        let task = Task::find_by_id(pool, task_id)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to load task",
                    Some(json!({ "error": e.to_string(), "task_id": task_id })),
                )
            })?
            .ok_or_else(|| {
                ErrorData::invalid_params(
                    "Task not found",
                    Some(json!({
                        "code": "not_found",
                        "retryable": false,
                        "hint": "Call list_tasks to get a valid task_id.",
                        "task_id": task_id,
                    })),
                )
            })?;

        if let Some(archive_id) = task.archived_kanban_id {
            return Self::err_with(
                "Task is archived. Restore it before starting an attempt.",
                Some(json!({
                    "tool": "start_attempt",
                    "task_id": task_id,
                    "archived_kanban_id": archive_id,
                })),
                Some("Use restore_archived_kanban (or the HTTP restore endpoint) to move it back to the active kanban.".to_string()),
                Some(MCP_CODE_BLOCKED_GUARDRAILS),
                Some(false),
            );
        }

        if repos.is_empty() {
            return Self::err_with(
                "At least one repository must be specified.",
                None,
                Some("Call list_repos to get repo_id and provide target_branch.".to_string()),
                Some("missing_required"),
                None,
            );
        }

        let executor_trimmed = executor.trim();
        if executor_trimmed.is_empty() {
            return Self::err_with(
                "Executor must not be empty.",
                None,
                Some("Provide a supported executor (e.g., CLAUDE_CODE).".to_string()),
                Some("missing_required"),
                None,
            );
        }

        let normalized_executor = executor_trimmed.replace('-', "_").to_ascii_uppercase();
        let base_executor = match BaseCodingAgent::from_str(&normalized_executor) {
            Ok(exec) => exec,
            Err(_) => {
                return Self::err_with(
                    format!("Unknown executor '{executor_trimmed}'."),
                    Some(json!({ "value": executor_trimmed })),
                    Some(
                        "Call list_executors to see valid executor names and variants.".to_string(),
                    ),
                    Some("invalid_argument"),
                    None,
                );
            }
        };

        let variant = variant.and_then(|v| {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        let executor_profile_id = ExecutorProfileId {
            executor: base_executor,
            variant,
        };

        #[derive(Serialize)]
        struct RepoSpecForHash {
            repo_id: Uuid,
            target_branch: String,
        }

        let mut repo_specs_for_hash = Vec::with_capacity(repos.len());
        let mut workspace_repos = Vec::with_capacity(repos.len());
        for (index, repo) in repos.into_iter().enumerate() {
            let target_branch = repo.target_branch.trim();
            if target_branch.is_empty() {
                return Self::err_with(
                    "Target branch must not be empty.",
                    Some(json!({
                        "field": format!("repos[{index}].target_branch")
                    })),
                    Some("Provide a branch name like `main` or `master`.".to_string()),
                    Some("invalid_argument"),
                    None,
                );
            }
            repo_specs_for_hash.push(RepoSpecForHash {
                repo_id: repo.repo_id,
                target_branch: target_branch.to_string(),
            });
            workspace_repos.push(CreateWorkspaceRepo {
                repo_id: repo.repo_id,
                target_branch: target_branch.to_string(),
            });
        }

        #[derive(Serialize)]
        struct StartAttemptIdempotencyPayload<'a> {
            task_id: Uuid,
            executor: &'a str,
            variant: &'a Option<String>,
            repos: &'a [RepoSpecForHash],
            prompt: &'a Option<String>,
        }

        let payload_hash = Self::request_hash(&StartAttemptIdempotencyPayload {
            task_id,
            executor: executor_trimmed,
            variant: &executor_profile_id.variant,
            repos: &repo_specs_for_hash,
            prompt: &prompt,
        })?;
        let key = Self::stable_tool_idempotency_key(request_id);

        let response = match self
            .idempotent("start_attempt", key, payload_hash, || async {
                let pool = &self.deployment.db().pool;
                let task = Task::find_by_id(pool, task_id)
                    .await
                    .map_err(|e| {
                        ErrorData::internal_error(
                            "Failed to load task",
                            Some(json!({ "error": e.to_string(), "task_id": task_id })),
                        )
                    })?
                    .ok_or_else(|| {
                        ErrorData::invalid_params(
                            "Task not found",
                            Some(json!({
                                "code": "not_found",
                                "retryable": false,
                                "hint": "Call list_tasks to get a valid task_id.",
                                "task_id": task_id,
                            })),
                        )
                    })?;

                if let Some(archive_id) = task.archived_kanban_id {
                    return Err(ErrorData::invalid_params(
                        "Task is archived. Restore it before starting an attempt.",
                        Some(json!({ "task_id": task_id, "archived_kanban_id": archive_id })),
                    ));
                }

                let project = Project::find_by_id(pool, task.project_id)
                    .await
                    .map_err(|e| {
                        ErrorData::internal_error(
                            "Failed to load project",
                            Some(json!({
                                "error": e.to_string(),
                                "project_id": task.project_id,
                                "task_id": task_id,
                            })),
                        )
                    })?
                    .ok_or_else(|| {
                        ErrorData::internal_error(
                            "Task references missing project",
                            Some(json!({ "project_id": task.project_id, "task_id": task_id })),
                        )
                    })?;

                let agent_working_dir = project
                    .default_agent_working_dir
                    .as_ref()
                    .filter(|dir| !dir.is_empty())
                    .cloned();

                let attempt_id = Uuid::new_v4();
                let git_branch_name = self
                    .deployment
                    .container()
                    .git_branch_from_workspace(&attempt_id, &task.title)
                    .await;

                let workspace = Workspace::create(
                    pool,
                    &CreateWorkspace {
                        branch: git_branch_name.clone(),
                        agent_working_dir,
                    },
                    attempt_id,
                    task_id,
                )
                .await
                .map_err(|e| {
                    ErrorData::internal_error(
                        "Failed to create workspace",
                        Some(json!({ "error": e.to_string(), "task_id": task_id })),
                    )
                })?;

                WorkspaceRepo::create_many(pool, workspace.id, &workspace_repos)
                    .await
                    .map_err(|e| {
                        ErrorData::internal_error(
                            "Failed to create workspace repos",
                            Some(json!({
                                "error": e.to_string(),
                                "attempt_id": workspace.id,
                                "task_id": task_id,
                            })),
                        )
                    })?;

                let exec = self
                    .deployment
                    .container()
                    .start_workspace(&workspace, executor_profile_id.clone(), prompt.clone())
                    .await
                    .map_err(|e| {
                        ErrorData::internal_error(
                            "Failed to start workspace",
                            Some(json!({
                                "code": "executor_error",
                                "error": e.to_string(),
                                "attempt_id": workspace.id,
                                "task_id": task_id,
                            })),
                        )
                    })?;

                let claimed_by_client_id = self.normalize_claimed_by_client_id(None);
                let lease_ttl = chrono::Duration::seconds(DEFAULT_ATTEMPT_CONTROL_LEASE_TTL_SECS);
                let lease = match attempt_control_lease_model::claim(
                    pool,
                    workspace.id,
                    claimed_by_client_id,
                    lease_ttl,
                    false,
                )
                .await
                .map_err(|e| {
                    ErrorData::internal_error(
                        "Failed to claim attempt control lease",
                        Some(json!({
                            "error": e.to_string(),
                            "attempt_id": workspace.id,
                            "task_id": task_id,
                        })),
                    )
                })? {
                    attempt_control_lease_model::ClaimOutcome::Claimed { lease, .. } => lease,
                    attempt_control_lease_model::ClaimOutcome::Conflict { current } => {
                        return Err(ErrorData::internal_error(
                            "Unexpected attempt control lease conflict",
                            Some(json!({
                                "attempt_id": workspace.id,
                                "task_id": task_id,
                                "claimed_by_client_id": current.claimed_by_client_id,
                                "expires_at": current.expires_at.to_rfc3339(),
                            })),
                        ));
                    }
                };

                Ok(StartAttemptResponse {
                    attempt_id: workspace.id.to_string(),
                    session_id: exec.session_id.to_string(),
                    execution_process_id: exec.id.to_string(),
                    control_token: lease.control_token.to_string(),
                    control_expires_at: lease.expires_at.to_rfc3339(),
                })
            })
            .await
        {
            Ok(response) => response,
            Err(ToolOrRpcError::Tool(tool_error)) => return Ok(tool_error),
            Err(ToolOrRpcError::Rpc(err)) => return Err(err),
        };

        Self::success(&response)
    }

    #[tool(
        description = r#"Use when: Claim/renew attempt control (lease) to perform mutating attempt operations.
Required: attempt_id
Optional: ttl_secs, force, claimed_by_client_id
Next: send_follow_up, stop_attempt, release_attempt_control
Avoid: Using long TTLs; forgetting force=true when taking over."#,
        output_schema = tool_output_schema::<ClaimAttemptControlResponse>(),
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false
        )
    )]
    async fn claim_attempt_control(
        &self,
        Parameters(ClaimAttemptControlRequest {
            attempt_id,
            ttl_secs,
            force,
            claimed_by_client_id,
        }): Parameters<ClaimAttemptControlRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let pool = &self.deployment.db().pool;
        let _ = Workspace::find_by_id(pool, attempt_id)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to load workspace",
                    Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
                )
            })?
            .ok_or_else(|| {
                ErrorData::invalid_params(
                    "Attempt not found",
                    Some(json!({ "attempt_id": attempt_id })),
                )
            })?;

        let ttl = match Self::lease_ttl(ttl_secs) {
            Ok(ttl) => ttl,
            Err(err) => return Ok(err),
        };

        let claimed_by_client_id = self.normalize_claimed_by_client_id(claimed_by_client_id);
        let force = force.unwrap_or(false);

        let outcome = attempt_control_lease_model::claim(
            pool,
            attempt_id,
            claimed_by_client_id.clone(),
            ttl,
            force,
        )
        .await
        .map_err(|e| {
            ErrorData::internal_error(
                "Failed to claim attempt control lease",
                Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
            )
        })?;

        match outcome {
            attempt_control_lease_model::ClaimOutcome::Claimed {
                lease,
                token_rotated,
            } => Self::success(&ClaimAttemptControlResponse {
                attempt_id: attempt_id.to_string(),
                control_token: lease.control_token.to_string(),
                claimed_by_client_id: lease.claimed_by_client_id,
                expires_at: lease.expires_at.to_rfc3339(),
                token_rotated,
            }),
            attempt_control_lease_model::ClaimOutcome::Conflict { current } => {
                let hint = format!(
                    "Attempt is controlled by {} until {}. Retry after expiry or call claim_attempt_control(force=true) to take over.",
                    current.claimed_by_client_id,
                    current.expires_at.to_rfc3339(),
                );
                Self::err_with(
                    "Attempt control lease is held by another client.",
                    Some(json!({
                        "attempt_id": attempt_id,
                        "claimed_by_client_id": current.claimed_by_client_id,
                        "expires_at": current.expires_at.to_rfc3339(),
                    })),
                    Some(hint),
                    Some(MCP_CODE_ATTEMPT_CLAIM_CONFLICT),
                    Some(false),
                )
            }
        }
    }

    #[tool(
        description = r#"Use when: Inspect current attempt control lease status (owner + expiry).
Required: attempt_id
Optional: (none)
Next: claim_attempt_control, send_follow_up, stop_attempt
Avoid: Assuming control_token can be recovered (store it from start_attempt/claim_attempt_control)."#,
        output_schema = tool_output_schema::<GetAttemptControlResponse>(),
        annotations(read_only_hint = true)
    )]
    async fn get_attempt_control(
        &self,
        Parameters(GetAttemptControlRequest { attempt_id }): Parameters<GetAttemptControlRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let pool = &self.deployment.db().pool;
        let _ = Workspace::find_by_id(pool, attempt_id)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to load workspace",
                    Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
                )
            })?
            .ok_or_else(|| {
                ErrorData::invalid_params(
                    "Attempt not found",
                    Some(json!({ "attempt_id": attempt_id })),
                )
            })?;

        let lease = attempt_control_lease_model::get_by_attempt_id(pool, attempt_id)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to load attempt control lease",
                    Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
                )
            })?;

        let now = chrono::Utc::now();
        let response = if let Some(lease) = lease {
            let expired = lease.is_expired_at(now);
            let claimed_by_client_id = lease.claimed_by_client_id;
            let expires_at = lease.expires_at.to_rfc3339();
            GetAttemptControlResponse {
                attempt_id: attempt_id.to_string(),
                has_lease: true,
                claimed_by_client_id: Some(claimed_by_client_id),
                expires_at: Some(expires_at),
                expired: Some(expired),
            }
        } else {
            GetAttemptControlResponse {
                attempt_id: attempt_id.to_string(),
                has_lease: false,
                claimed_by_client_id: None,
                expires_at: None,
                expired: None,
            }
        };

        Self::success(&response)
    }

    #[tool(
        description = r#"Use when: Release attempt control lease after finishing mutating operations.
Required: attempt_id, control_token
Optional: (none)
Next: claim_attempt_control
Avoid: Releasing with a mismatched token."#,
        output_schema = tool_output_schema::<ReleaseAttemptControlResponse>(),
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false
        )
    )]
    async fn release_attempt_control(
        &self,
        Parameters(ReleaseAttemptControlRequest {
            attempt_id,
            control_token,
        }): Parameters<ReleaseAttemptControlRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let pool = &self.deployment.db().pool;
        let _ = Workspace::find_by_id(pool, attempt_id)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to load workspace",
                    Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
                )
            })?
            .ok_or_else(|| {
                ErrorData::invalid_params(
                    "Attempt not found",
                    Some(json!({ "attempt_id": attempt_id })),
                )
            })?;

        match attempt_control_lease_model::release(pool, attempt_id, control_token).await {
            Ok(attempt_control_lease_model::ReleaseOutcome::Released) => {
                Self::success(&ReleaseAttemptControlResponse {
                    attempt_id: attempt_id.to_string(),
                    released: true,
                })
            }
            Ok(attempt_control_lease_model::ReleaseOutcome::NotFound) => Self::err_with(
                "Attempt control lease not found.",
                Some(json!({ "attempt_id": attempt_id })),
                Some("Nothing to release. Call claim_attempt_control(attempt_id) to acquire control.".to_string()),
                Some(MCP_CODE_ATTEMPT_CLAIM_REQUIRED),
                Some(false),
            ),
            Ok(attempt_control_lease_model::ReleaseOutcome::TokenMismatch { current }) => {
                Self::err_with(
                    "Invalid control_token for release_attempt_control.",
                    Some(json!({
                        "attempt_id": attempt_id,
                        "provided_control_token": control_token,
                        "claimed_by_client_id": current.claimed_by_client_id,
                        "expires_at": current.expires_at.to_rfc3339(),
                    })),
                    Some("Re-run claim_attempt_control(attempt_id) to obtain a fresh control_token.".to_string()),
                    Some(MCP_CODE_INVALID_CONTROL_TOKEN),
                    Some(false),
                )
            }
            Err(e) => Err(ErrorData::internal_error(
                "Failed to release attempt control lease",
                Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
            )),
        }
    }

    #[tool(
        description = r#"Use when: Send a follow-up message to the coding agent for a specific session (or an attempt's latest session).
Required: exactly one of {attempt_id, session_id}, prompt
Also required (mutating): control_token
Optional: variant, request_id
Next: tail_attempt_feed
Avoid: Providing both attempt_id and session_id; missing prompt."#,
        output_schema = tool_output_schema::<SendFollowUpResponse>(),
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true
        ),
        execution(task_support = "optional")
    )]
    async fn send_follow_up(
        &self,
        Parameters(SendFollowUpRequest {
            attempt_id,
            session_id,
            control_token,
            prompt,
            variant,
            request_id,
        }): Parameters<SendFollowUpRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let prompt_trim = prompt.trim();
        if prompt_trim.is_empty() {
            return Self::err_with(
                "Prompt must not be empty.",
                None,
                Some("Provide a prompt string.".to_string()),
                Some("missing_required"),
                None,
            );
        }

        let session_id = match self
            .resolve_session_id(session_id, attempt_id, "send_follow_up")
            .await
        {
            Ok(session_id) => session_id,
            Err(e) => return Ok(e),
        };

        let pool = &self.deployment.db().pool;
        let attempt_id_for_control = if let Some(attempt_id) = attempt_id {
            attempt_id
        } else {
            let session = Session::find_by_id(pool, session_id)
                .await
                .map_err(|e| {
                    ErrorData::internal_error(
                        "Failed to load session",
                        Some(json!({ "error": e.to_string(), "session_id": session_id })),
                    )
                })?
                .ok_or_else(|| {
                    ErrorData::invalid_params(
                        "Session not found",
                        Some(json!({ "session_id": session_id })),
                    )
                })?;
            session.workspace_id
        };

        let _ = Workspace::find_by_id(pool, attempt_id_for_control)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to load workspace",
                    Some(json!({ "error": e.to_string(), "attempt_id": attempt_id_for_control })),
                )
            })?
            .ok_or_else(|| {
                ErrorData::invalid_params(
                    "Attempt not found",
                    Some(json!({ "attempt_id": attempt_id_for_control })),
                )
            })?;

        if let Err(err) = self
            .require_attempt_control_token(attempt_id_for_control, control_token, "send_follow_up")
            .await
        {
            return Ok(err);
        }

        #[derive(Serialize)]
        struct FollowUpIdempotencyPayload<'a> {
            session_id: Uuid,
            control_token: &'a Option<Uuid>,
            prompt: &'a str,
            variant: &'a Option<String>,
        }

        let hash = Self::request_hash(&FollowUpIdempotencyPayload {
            session_id,
            control_token: &control_token,
            prompt: prompt_trim,
            variant: &variant,
        })?;
        let key = Self::stable_tool_idempotency_key(request_id);

        let response = match self
            .idempotent("send_follow_up", key, hash, || async {
                let pool = &self.deployment.db().pool;
                let session = Session::find_by_id(pool, session_id)
                    .await
                    .map_err(|e| {
                        ErrorData::internal_error(
                            "Failed to load session",
                            Some(json!({ "error": e.to_string(), "session_id": session_id })),
                        )
                    })?
                    .ok_or_else(|| {
                        ErrorData::invalid_params(
                            "Session not found",
                            Some(json!({
                                "code": "not_found",
                                "retryable": false,
                                "session_id": session_id,
                            })),
                        )
                    })?;

                let workspace = Workspace::find_by_id(pool, session.workspace_id)
                    .await
                    .map_err(|e| {
                        ErrorData::internal_error(
                            "Failed to load workspace",
                            Some(json!({
                                "error": e.to_string(),
                                "workspace_id": session.workspace_id,
                                "session_id": session_id,
                            })),
                        )
                    })?
                    .ok_or_else(|| {
                        ErrorData::internal_error(
                            "Session references missing workspace",
                            Some(json!({
                                "workspace_id": session.workspace_id,
                                "session_id": session_id,
                            })),
                        )
                    })?;

                self.deployment
                    .container()
                    .ensure_container_exists(&workspace)
                    .await
                    .map_err(|e| {
                        ErrorData::internal_error(
                            "Failed to ensure workspace container",
                            Some(json!({
                                "code": "container_error",
                                "error": e.to_string(),
                                "attempt_id": workspace.id,
                                "session_id": session_id,
                            })),
                        )
                    })?;

                let initial_executor_profile_id =
                    ExecutionProcess::latest_executor_profile_for_session(pool, session.id)
                        .await
                        .map_err(|e| {
                            ErrorData::internal_error(
                                "Failed to resolve executor profile for session",
                                Some(json!({
                                    "code": "invalid_state",
                                    "error": e.to_string(),
                                    "session_id": session.id,
                                })),
                            )
                        })?;

                let executor_profile_id = ExecutorProfileId {
                    executor: initial_executor_profile_id.executor,
                    variant: variant
                        .as_ref()
                        .map(|v| v.trim().to_string())
                        .filter(|v| !v.is_empty()),
                };

                let latest_agent_session_id =
                    ExecutionProcess::find_latest_coding_agent_turn_session_id(pool, session.id)
                        .await
                        .map_err(|e| {
                            ErrorData::internal_error(
                                "Failed to resolve agent session id",
                                Some(json!({ "error": e.to_string(), "session_id": session.id })),
                            )
                        })?;

                let task = Task::find_by_id(pool, workspace.task_id)
                    .await
                    .map_err(|e| {
                        ErrorData::internal_error(
                            "Failed to load task",
                            Some(json!({ "error": e.to_string(), "task_id": workspace.task_id })),
                        )
                    })?
                    .ok_or_else(|| {
                        ErrorData::internal_error(
                            "Workspace references missing task",
                            Some(json!({
                                "task_id": workspace.task_id,
                                "attempt_id": workspace.id,
                            })),
                        )
                    })?;

                let project_repos = ProjectRepo::find_by_project_id_with_names(
                    pool,
                    task.project_id,
                )
                .await
                .map_err(|e| {
                    ErrorData::internal_error(
                        "Failed to load project repos",
                        Some(json!({ "error": e.to_string(), "project_id": task.project_id })),
                    )
                })?;
                let cleanup_action = self
                    .deployment
                    .container()
                    .cleanup_actions_for_repos(&project_repos);

                let working_dir = workspace
                    .agent_working_dir
                    .as_ref()
                    .filter(|dir| !dir.is_empty())
                    .cloned();

                let action_type = if let Some(agent_session_id) = latest_agent_session_id {
                    ExecutorActionType::CodingAgentFollowUpRequest(CodingAgentFollowUpRequest {
                        prompt: prompt_trim.to_string(),
                        session_id: agent_session_id,
                        executor_profile_id: executor_profile_id.clone(),
                        working_dir: working_dir.clone(),
                        image_paths: None,
                    })
                } else {
                    ExecutorActionType::CodingAgentInitialRequest(CodingAgentInitialRequest {
                        prompt: prompt_trim.to_string(),
                        executor_profile_id: executor_profile_id.clone(),
                        working_dir,
                        image_paths: None,
                    })
                };

                let action = ExecutorAction::new(action_type, cleanup_action.map(Box::new));

                // Compute repo states (best-effort) from workspace repos.
                let repositories = WorkspaceRepo::find_repos_for_workspace(pool, workspace.id)
                    .await
                    .map_err(|e| {
                        ErrorData::internal_error(
                            "Failed to load workspace repos",
                            Some(json!({
                                "error": e.to_string(),
                                "attempt_id": workspace.id,
                                "session_id": session_id,
                            })),
                        )
                    })?;
                let repo_states = repositories
                    .iter()
                    .map(|repo| CreateExecutionProcessRepoState {
                        repo_id: repo.id,
                        before_head_commit: None,
                        after_head_commit: None,
                        merge_commit: None,
                    })
                    .collect::<Vec<_>>();

                let exec = self
                    .deployment
                    .container()
                    .start_execution(
                        &workspace,
                        &session,
                        &action,
                        &ExecutionProcessRunReason::CodingAgent,
                    )
                    .await
                    .map_err(|e| {
                        ErrorData::internal_error(
                            "Failed to start follow-up execution",
                            Some(json!({
                                "code": "executor_error",
                                "error": e.to_string(),
                                "attempt_id": workspace.id,
                                "session_id": session_id,
                            })),
                        )
                    })?;

                // Ensure the execution process has repo state rows for downstream tooling.
                // start_execution already does this, but we keep this payload stable for idempotency hashing.
                let _ = repo_states;

                Ok(SendFollowUpResponse {
                    session_id: session.id.to_string(),
                    execution_process_id: exec.id.to_string(),
                })
            })
            .await
        {
            Ok(response) => response,
            Err(ToolOrRpcError::Tool(tool_error)) => return Ok(tool_error),
            Err(ToolOrRpcError::Rpc(err)) => return Err(err),
        };

        Self::success(&response)
    }

    #[tool(
        description = r#"Use when: Stop a running attempt's non-dev-server execution.
Required: attempt_id
Also required (mutating): control_token
Optional: force
Next: tail_attempt_feed
Avoid: Expecting this to stop dev servers."#,
        output_schema = tool_output_schema::<StopAttemptResponse>(),
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true
        ),
        execution(task_support = "optional")
    )]
    async fn stop_attempt(
        &self,
        Parameters(StopAttemptRequest {
            attempt_id,
            control_token,
            force,
        }): Parameters<StopAttemptRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let pool = &self.deployment.db().pool;
        let workspace = Workspace::find_by_id(pool, attempt_id)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to load workspace",
                    Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
                )
            })?
            .ok_or_else(|| {
                ErrorData::invalid_params(
                    "Attempt not found",
                    Some(json!({ "attempt_id": attempt_id })),
                )
            })?;

        if let Err(err) = self
            .require_attempt_control_token(attempt_id, control_token, "stop_attempt")
            .await
        {
            return Ok(err);
        }

        if force.unwrap_or(false) {
            self.deployment
                .container()
                .try_stop_force(&workspace, false)
                .await;
        } else {
            self.deployment
                .container()
                .try_stop(&workspace, false)
                .await;
        }

        Self::success(&StopAttemptResponse {
            attempt_id: attempt_id.to_string(),
            force: force.unwrap_or(false),
        })
    }

    #[tool(
        description = r#"Use when: Tail attempt feed (state + normalized logs + pending approvals).
Required: attempt_id
Optional: limit, cursor, after_log_index, wait_ms
Next: respond_approval, get_attempt_changes
Avoid: Mixing cursor with after_log_index; using wait_ms without after_log_index."#,
        output_schema = rmcp::handler::server::tool::schema_for_output::<TailAttemptFeedResponse>()
            .unwrap_or_else(|e| {
                panic!(
                    "Invalid output schema for {}: {}",
                    std::any::type_name::<TailAttemptFeedResponse>(),
                    e
                )
            }),
        annotations(read_only_hint = true)
    )]
    async fn tail_attempt_feed(
        &self,
        Parameters(TailAttemptFeedRequest {
            attempt_id,
            limit,
            cursor,
            after_log_index,
            wait_ms,
        }): Parameters<TailAttemptFeedRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if cursor.is_some() && after_log_index.is_some() {
            return Self::err_with(
                "cursor and after_log_index are mutually exclusive.",
                Some(json!({ "cursor": cursor, "after_log_index": after_log_index })),
                Some(
                    "Use cursor to page older history; use after_log_index to fetch only new entries."
                        .to_string(),
                ),
                Some(MCP_CODE_MIXED_PAGINATION),
                Some(false),
            );
        }

        let wait_ms = wait_ms.unwrap_or(0);
        if wait_ms > 0 {
            if after_log_index.is_none() {
                return Self::err_with(
                    "wait_ms is only supported when after_log_index is set.",
                    Some(json!({ "wait_ms": wait_ms, "after_log_index": after_log_index })),
                    Some("Provide after_log_index when using wait_ms.".to_string()),
                    Some(MCP_CODE_WAIT_MS_REQUIRES_AFTER_LOG_INDEX),
                    Some(false),
                );
            }

            if wait_ms > TAIL_ATTEMPT_FEED_MAX_WAIT_MS {
                return Self::err_with(
                    "wait_ms exceeds the server limit.",
                    Some(json!({
                        "wait_ms": wait_ms,
                        "max_wait_ms": TAIL_ATTEMPT_FEED_MAX_WAIT_MS,
                    })),
                    Some(format!(
                        "Reduce wait_ms to <= {}.",
                        TAIL_ATTEMPT_FEED_MAX_WAIT_MS
                    )),
                    Some(MCP_CODE_WAIT_MS_TOO_LARGE),
                    Some(false),
                );
            }
        }

        let pool = &self.deployment.db().pool;
        let workspace = Workspace::find_by_id(pool, attempt_id)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to load workspace",
                    Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
                )
            })?
            .ok_or_else(|| {
                ErrorData::invalid_params(
                    "Attempt not found",
                    Some(json!({ "attempt_id": attempt_id })),
                )
            })?;

        let latest_session = Session::find_latest_by_workspace_id(pool, workspace.id)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to resolve latest session",
                    Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
                )
            })?;

        let mut latest_process: Option<ExecutionProcess> = None;
        for run_reason in [
            ExecutionProcessRunReason::CodingAgent,
            ExecutionProcessRunReason::SetupScript,
            ExecutionProcessRunReason::CleanupScript,
        ] {
            let Some(process) = ExecutionProcess::find_latest_by_workspace_and_run_reason(
                pool,
                workspace.id,
                &run_reason,
            )
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to resolve latest execution process",
                    Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
                )
            })?
            else {
                continue;
            };

            let replace = match &latest_process {
                Some(existing) => process.created_at > existing.created_at,
                None => true,
            };
            if replace {
                latest_process = Some(process);
            }
        }

        let (mut state, mut failure_summary) =
            Self::map_attempt_state(latest_process.as_ref().map(|p| p.status.clone()));

        let (mut page, mut latest_execution_process_id, mut next_after_log_index) = if let Some(
            process,
        ) =
            latest_process.as_ref()
        {
            let limit = limit.unwrap_or(50).clamp(1, 1000);

            if let Some(after) = after_log_index {
                let (entries, history_truncated) = self
                    .deployment
                    .container()
                    .log_history_after(
                        process,
                        utils_core::log_entries::LogEntryChannel::Normalized,
                        limit,
                        after,
                    )
                    .await
                    .map_err(|e| {
                        ErrorData::internal_error(
                            "Failed to load log history",
                            Some(json!({ "error": e.to_string(), "execution_process_id": process.id })),
                        )
                    })?;

                let entries = entries
                    .into_iter()
                    .map(|entry| McpIndexedLogEntry {
                        entry_index: i64::try_from(entry.entry_index).unwrap_or(i64::MAX),
                        entry: entry.entry_json,
                    })
                    .collect::<Vec<_>>();

                let next_after = entries.last().map(|e| e.entry_index).or(Some(after));

                (
                    McpLogHistoryPage {
                        entries,
                        next_cursor: None,
                        has_more: false,
                        history_truncated,
                    },
                    Some(process.id.to_string()),
                    next_after,
                )
            } else {
                let page = self
                    .deployment
                    .container()
                    .log_history_page(
                        process,
                        utils_core::log_entries::LogEntryChannel::Normalized,
                        limit,
                        cursor,
                    )
                    .await
                    .map_err(|e| {
                        ErrorData::internal_error(
                            "Failed to load log history",
                            Some(json!({ "error": e.to_string(), "execution_process_id": process.id })),
                        )
                    })?;

                let next_cursor = if page.has_more {
                    page.entries.first().map(|e| e.entry_index as i64)
                } else {
                    None
                };

                let entries = page
                    .entries
                    .into_iter()
                    .map(|entry| McpIndexedLogEntry {
                        entry_index: i64::try_from(entry.entry_index).unwrap_or(i64::MAX),
                        entry: entry.entry_json,
                    })
                    .collect::<Vec<_>>();

                let next_after = entries.last().map(|e| e.entry_index);

                (
                    McpLogHistoryPage {
                        entries,
                        next_cursor,
                        has_more: page.has_more,
                        history_truncated: page.history_truncated,
                    },
                    Some(process.id.to_string()),
                    next_after,
                )
            }
        } else {
            (
                McpLogHistoryPage {
                    entries: Vec::new(),
                    next_cursor: None,
                    has_more: false,
                    history_truncated: false,
                },
                None,
                None,
            )
        };

        let (pending, _) = self
            .deployment
            .approvals()
            .list_approvals_by_attempt(pool, attempt_id, Some("pending"), 200, None)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to list approvals",
                    Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
                )
            })?;

        let mut pending_approvals: Vec<McpApprovalSummary> =
            pending.into_iter().map(Self::approval_to_summary).collect();

        if wait_ms > 0
            && after_log_index.is_some()
            && pending_approvals.is_empty()
            && page.entries.is_empty()
        {
            let Some(process) = latest_process.as_ref() else {
                // No running process to wait on; return current snapshot.
                return Self::success(&TailAttemptFeedResponse {
                    attempt_id: workspace.id.to_string(),
                    task_id: workspace.task_id.to_string(),
                    workspace_branch: workspace.branch,
                    state,
                    latest_session_id: latest_session.as_ref().map(|s| s.id.to_string()),
                    latest_execution_process_id,
                    failure_summary,
                    page,
                    next_after_log_index,
                    pending_approvals,
                });
            };

            let after = after_log_index.unwrap_or(-1);
            let process_id = process.id;
            let wait_for = std::time::Duration::from_millis(wait_ms);

            let mut approvals_rx = self.deployment.approvals().subscribe_created();
            let store = self
                .deployment
                .container()
                .get_msg_store_by_id(&process_id)
                .await;
            let mut log_rx = store.map(|store| store.subscribe_normalized_entries());

            let deadline = tokio::time::sleep(wait_for);
            tokio::pin!(deadline);

            loop {
                tokio::select! {
                    _ = &mut deadline => break,
                    recv = async { log_rx.as_mut().unwrap().recv().await }, if log_rx.is_some() => {
                        match recv {
                            Ok(event) => match event {
                                logs_store::LogEntryEvent::Append { entry_index, .. }
                                | logs_store::LogEntryEvent::Replace { entry_index, .. } => {
                                    let idx = i64::try_from(entry_index).unwrap_or(i64::MAX);
                                    if idx > after {
                                        break;
                                    }
                                }
                                logs_store::LogEntryEvent::Finished => break,
                            },
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => break,
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                log_rx = None;
                            }
                        }
                    }
                    recv = approvals_rx.recv() => {
                        match recv {
                            Ok(approval) if approval.execution_process_id == process_id => break,
                            Ok(_) => {}
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => break,
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {}
                        }
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_millis(250)), if log_rx.is_none() => {
                        let check = self
                            .deployment
                            .container()
                            .log_history_after(
                                process,
                                utils_core::log_entries::LogEntryChannel::Normalized,
                                1,
                                after,
                            )
                            .await;
                        match check {
                            Ok((entries, _truncated)) if !entries.is_empty() => break,
                            Ok(_) => {}
                            Err(err) => {
                                return Err(ErrorData::internal_error(
                                    "Failed to check log history during wait",
                                    Some(json!({ "error": err.to_string(), "execution_process_id": process_id })),
                                ));
                            }
                        }
                    }
                }
            }

            // Refresh attempt state and data after waiting.
            if let Some(fresh) = ExecutionProcess::find_by_id(pool, process_id)
                .await
                .map_err(|e| {
                    ErrorData::internal_error(
                        "Failed to reload execution process",
                        Some(json!({ "error": e.to_string(), "execution_process_id": process_id })),
                    )
                })?
            {
                latest_process = Some(fresh);
            }

            let (fresh_state, fresh_failure) =
                Self::map_attempt_state(latest_process.as_ref().map(|p| p.status.clone()));
            state = fresh_state;
            failure_summary = fresh_failure;

            // Refresh logs (after mode only).
            if let Some(process) = latest_process.as_ref() {
                let limit = limit.unwrap_or(50).clamp(1, 1000);
                let (entries, history_truncated) = self
                    .deployment
                    .container()
                    .log_history_after(
                        process,
                        utils_core::log_entries::LogEntryChannel::Normalized,
                        limit,
                        after,
                    )
                    .await
                    .map_err(|e| {
                        ErrorData::internal_error(
                            "Failed to load log history",
                            Some(json!({ "error": e.to_string(), "execution_process_id": process.id })),
                        )
                    })?;

                let entries = entries
                    .into_iter()
                    .map(|entry| McpIndexedLogEntry {
                        entry_index: i64::try_from(entry.entry_index).unwrap_or(i64::MAX),
                        entry: entry.entry_json,
                    })
                    .collect::<Vec<_>>();

                next_after_log_index = entries.last().map(|e| e.entry_index).or(Some(after));
                latest_execution_process_id = Some(process.id.to_string());
                page = McpLogHistoryPage {
                    entries,
                    next_cursor: None,
                    has_more: false,
                    history_truncated,
                };
            }

            // Refresh pending approvals.
            let (pending, _) = self
                .deployment
                .approvals()
                .list_approvals_by_attempt(pool, attempt_id, Some("pending"), 200, None)
                .await
                .map_err(|e| {
                    ErrorData::internal_error(
                        "Failed to list approvals",
                        Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
                    )
                })?;
            pending_approvals = pending.into_iter().map(Self::approval_to_summary).collect();
        }

        Self::success(&TailAttemptFeedResponse {
            attempt_id: workspace.id.to_string(),
            task_id: workspace.task_id.to_string(),
            workspace_branch: workspace.branch,
            state,
            latest_session_id: latest_session.as_ref().map(|s| s.id.to_string()),
            latest_execution_process_id,
            failure_summary,
            page,
            next_after_log_index,
            pending_approvals,
        })
    }

    #[tool(
        description = r#"Use when: Tail session transcript context (prompt + summary per turn).
Required: exactly one of {attempt_id, session_id}
Optional: limit, cursor
Next: send_follow_up
Avoid: Expecting raw tool logs (use tail_attempt_feed)."#,
        output_schema = tool_output_schema::<TailSessionMessagesResponse>(),
        annotations(read_only_hint = true)
    )]
    async fn tail_session_messages(
        &self,
        Parameters(TailSessionMessagesRequest {
            attempt_id,
            session_id,
            limit,
            cursor,
        }): Parameters<TailSessionMessagesRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let session_id = match self
            .resolve_session_id(session_id, attempt_id, "tail_session_messages")
            .await
        {
            Ok(session_id) => session_id,
            Err(e) => return Ok(e),
        };

        let limit = limit.unwrap_or(20).clamp(1, 200);
        let turns = CodingAgentTurn::tail_by_session_id(
            &self.deployment.db().pool,
            session_id,
            limit,
            cursor,
        )
        .await
        .map_err(|e| {
            ErrorData::internal_error(
                "Failed to tail session messages",
                Some(json!({ "error": e.to_string(), "session_id": session_id })),
            )
        })?;

        let entries = turns
            .entries
            .into_iter()
            .map(|turn| McpSessionMessageTurn {
                entry_index: turn.entry_index,
                turn_id: turn.turn_id.to_string(),
                prompt: turn.prompt,
                summary: turn.summary,
                created_at: turn.created_at.to_rfc3339(),
                updated_at: turn.updated_at.to_rfc3339(),
            })
            .collect::<Vec<_>>();

        Self::success(&TailSessionMessagesResponse {
            session_id: session_id.to_string(),
            page: McpSessionMessagesPage {
                entries,
                next_cursor: turns.next_cursor,
                has_more: turns.has_more,
            },
        })
    }

    #[tool(
        description = r#"Use when: Get a diff summary and (if allowed) a changed-file list for an attempt.
Required: attempt_id
Optional: force
Next: get_attempt_patch
Avoid: Assuming files will be returned when blocked=true; using force unless you accept larger output."#,
        output_schema = tool_output_schema::<GetAttemptChangesResponse>(),
        annotations(read_only_hint = true),
        execution(task_support = "optional")
    )]
    async fn get_attempt_changes(
        &self,
        Parameters(GetAttemptChangesRequest { attempt_id, force }): Parameters<
            GetAttemptChangesRequest,
        >,
    ) -> Result<CallToolResult, ErrorData> {
        let force = force.unwrap_or(false);
        let workspace = Workspace::find_by_id(&self.deployment.db().pool, attempt_id)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to load workspace",
                    Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
                )
            })?
            .ok_or_else(|| {
                ErrorData::invalid_params(
                    "Attempt not found",
                    Some(json!({ "attempt_id": attempt_id })),
                )
            })?;

        let query = crate::routes::task_attempts::AttemptChangesQuery { force };
        let ResponseJson(response) = crate::routes::task_attempts::get_task_attempt_changes(
            axum::Extension(workspace),
            axum::extract::State(self.deployment.clone()),
            axum::extract::Query(query),
        )
        .await
        .map_err(|e| {
            ErrorData::internal_error(
                "Failed to compute attempt changes",
                Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
            )
        })?;

        let message = response.message().map(str::to_string);
        let changes = response.into_data().ok_or_else(|| {
            ErrorData::internal_error(
                "Attempt changes response missing data",
                Some(json!({ "attempt_id": attempt_id, "message": message })),
            )
        })?;
        let blocked_reason = match changes.blocked_reason {
            Some(crate::routes::task_attempts::AttemptChangesBlockedReason::SummaryFailed) => {
                Some(McpAttemptChangesBlockedReason::SummaryFailed)
            }
            Some(crate::routes::task_attempts::AttemptChangesBlockedReason::ThresholdExceeded) => {
                Some(McpAttemptChangesBlockedReason::ThresholdExceeded)
            }
            None => None,
        };

        let summary = McpAttemptChangesSummary {
            file_count: changes.summary.file_count,
            added: changes.summary.added,
            deleted: changes.summary.deleted,
            total_bytes: changes.summary.total_bytes,
        };

        if changes.blocked {
            let hint = match blocked_reason {
                Some(McpAttemptChangesBlockedReason::ThresholdExceeded) => {
                    if force {
                        "Changed-file list blocked by guardrails even with force=true. Reduce the scope or try again later.".to_string()
                    } else {
                        "Changed-file list blocked by diff preview guardrails. Retry with force=true if you accept a larger file list.".to_string()
                    }
                }
                Some(McpAttemptChangesBlockedReason::SummaryFailed) => {
                    "Changed-file list blocked due to summary failure. Retry later.".to_string()
                }
                None => "Changed-file list blocked by guardrails.".to_string(),
            };

            return Self::err_with(
                "Changed-file list blocked by guardrails.",
                Some(json!({
                    "attempt_id": attempt_id,
                    "blocked_reason": blocked_reason,
                    "summary": summary,
                })),
                Some(hint),
                Some(MCP_CODE_BLOCKED_GUARDRAILS),
                Some(false),
            );
        }

        let files = if changes.blocked {
            None
        } else {
            Some(changes.files)
        };

        Self::success(&GetAttemptChangesResponse {
            attempt_id: attempt_id.to_string(),
            summary,
            blocked: changes.blocked,
            blocked_reason,
            code: None,
            retryable: None,
            hint: None,
            files,
        })
    }

    #[tool(
        description = r#"Use when: Fetch a file inside an attempt workspace.
Required: attempt_id, path
Optional: start, max_bytes
Next: get_attempt_patch
Avoid: Absolute paths or .. traversal."#,
        output_schema = tool_output_schema::<GetAttemptFileResponse>(),
        annotations(read_only_hint = true),
        execution(task_support = "optional")
    )]
    async fn get_attempt_file(
        &self,
        Parameters(GetAttemptFileRequest {
            attempt_id,
            path,
            start,
            max_bytes,
        }): Parameters<GetAttemptFileRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let workspace = Workspace::find_by_id(&self.deployment.db().pool, attempt_id)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to load workspace",
                    Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
                )
            })?
            .ok_or_else(|| {
                ErrorData::invalid_params(
                    "Attempt not found",
                    Some(json!({ "attempt_id": attempt_id })),
                )
            })?;

        let query = crate::routes::task_attempts::AttemptFileQuery {
            path: Some(path.clone()),
            start,
            max_bytes,
        };
        let ResponseJson(response) = crate::routes::task_attempts::get_task_attempt_file(
            axum::Extension(workspace),
            axum::extract::State(self.deployment.clone()),
            axum::extract::Query(query),
        )
        .await
        .map_err(|e| {
            ErrorData::internal_error(
                "Failed to read attempt file",
                Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
            )
        })?;

        let file =
            response
                .into_data()
                .unwrap_or(crate::routes::task_attempts::AttemptFileResponse {
                    path,
                    blocked: true,
                    blocked_reason: Some(
                        crate::routes::task_attempts::AttemptArtifactBlockedReason::SummaryFailed,
                    ),
                    truncated: false,
                    start: 0,
                    bytes: 0,
                    total_bytes: None,
                    content: None,
                });

        let blocked_reason = file.blocked_reason.map(|reason| match reason {
            crate::routes::task_attempts::AttemptArtifactBlockedReason::PathOutsideWorkspace => {
                McpAttemptArtifactBlockedReason::PathOutsideWorkspace
            }
            crate::routes::task_attempts::AttemptArtifactBlockedReason::SizeExceeded => {
                McpAttemptArtifactBlockedReason::SizeExceeded
            }
            crate::routes::task_attempts::AttemptArtifactBlockedReason::TooManyPaths => {
                McpAttemptArtifactBlockedReason::TooManyPaths
            }
            crate::routes::task_attempts::AttemptArtifactBlockedReason::SummaryFailed => {
                McpAttemptArtifactBlockedReason::SummaryFailed
            }
            crate::routes::task_attempts::AttemptArtifactBlockedReason::ThresholdExceeded => {
                McpAttemptArtifactBlockedReason::ThresholdExceeded
            }
        });

        if file.blocked {
            let hint = match blocked_reason {
                Some(McpAttemptArtifactBlockedReason::PathOutsideWorkspace) => {
                    "Path is outside workspace. Provide a path within the attempt workspace."
                        .to_string()
                }
                Some(McpAttemptArtifactBlockedReason::SizeExceeded) => {
                    "File too large. Reduce max_bytes or page with start.".to_string()
                }
                Some(McpAttemptArtifactBlockedReason::TooManyPaths) => {
                    "Too many paths. Provide a single path.".to_string()
                }
                Some(McpAttemptArtifactBlockedReason::SummaryFailed) => {
                    "File retrieval blocked due to summary failure. Retry later.".to_string()
                }
                Some(McpAttemptArtifactBlockedReason::ThresholdExceeded) => {
                    "File retrieval blocked by guardrails. Reduce max_bytes or request a smaller range."
                        .to_string()
                }
                None => "File retrieval blocked by guardrails.".to_string(),
            };

            return Self::err_with(
                "File retrieval blocked by guardrails.",
                Some(json!({
                    "attempt_id": attempt_id,
                    "path": file.path,
                    "start": file.start,
                    "max_bytes": max_bytes,
                    "blocked_reason": blocked_reason,
                })),
                Some(hint),
                Some(MCP_CODE_BLOCKED_GUARDRAILS),
                Some(false),
            );
        }

        Self::success(&GetAttemptFileResponse {
            attempt_id: attempt_id.to_string(),
            blocked: file.blocked,
            blocked_reason,
            code: None,
            retryable: None,
            hint: None,
            truncated: file.truncated,
            start: file.start,
            bytes: file.bytes,
            total_bytes: file.total_bytes,
            path: file.path,
            content: file.content,
        })
    }

    #[tool(
        description = r#"Use when: Fetch a unified diff patch for selected paths in an attempt.
Required: attempt_id, paths
Optional: force, max_bytes
Next: send_follow_up
Avoid: Too many paths; huge max_bytes."#,
        output_schema = tool_output_schema::<GetAttemptPatchResponse>(),
        annotations(read_only_hint = true),
        execution(task_support = "optional")
    )]
    async fn get_attempt_patch(
        &self,
        Parameters(GetAttemptPatchRequest {
            attempt_id,
            paths,
            force,
            max_bytes,
        }): Parameters<GetAttemptPatchRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let force = force.unwrap_or(false);

        let workspace = Workspace::find_by_id(&self.deployment.db().pool, attempt_id)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to load workspace",
                    Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
                )
            })?
            .ok_or_else(|| {
                ErrorData::invalid_params(
                    "Attempt not found",
                    Some(json!({ "attempt_id": attempt_id })),
                )
            })?;

        let req = crate::routes::task_attempts::AttemptPatchRequest {
            paths: paths.clone(),
            force,
            max_bytes,
        };
        let ResponseJson(response) = crate::routes::task_attempts::get_task_attempt_patch(
            axum::Extension(workspace),
            axum::extract::State(self.deployment.clone()),
            axum::Json(req),
        )
        .await
        .map_err(|e| {
            ErrorData::internal_error(
                "Failed to compute attempt patch",
                Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
            )
        })?;

        let patch =
            response
                .into_data()
                .unwrap_or(crate::routes::task_attempts::AttemptPatchResponse {
                    blocked: true,
                    blocked_reason: Some(
                        crate::routes::task_attempts::AttemptArtifactBlockedReason::SummaryFailed,
                    ),
                    truncated: false,
                    bytes: 0,
                    paths,
                    patch: None,
                });

        let blocked_reason = patch.blocked_reason.map(|reason| match reason {
            crate::routes::task_attempts::AttemptArtifactBlockedReason::PathOutsideWorkspace => {
                McpAttemptArtifactBlockedReason::PathOutsideWorkspace
            }
            crate::routes::task_attempts::AttemptArtifactBlockedReason::SizeExceeded => {
                McpAttemptArtifactBlockedReason::SizeExceeded
            }
            crate::routes::task_attempts::AttemptArtifactBlockedReason::TooManyPaths => {
                McpAttemptArtifactBlockedReason::TooManyPaths
            }
            crate::routes::task_attempts::AttemptArtifactBlockedReason::SummaryFailed => {
                McpAttemptArtifactBlockedReason::SummaryFailed
            }
            crate::routes::task_attempts::AttemptArtifactBlockedReason::ThresholdExceeded => {
                McpAttemptArtifactBlockedReason::ThresholdExceeded
            }
        });

        if patch.blocked {
            let hint = match blocked_reason {
                Some(McpAttemptArtifactBlockedReason::PathOutsideWorkspace) => {
                    "Paths are outside workspace. Provide only paths within the attempt workspace."
                        .to_string()
                }
                Some(McpAttemptArtifactBlockedReason::SizeExceeded) => {
                    "Patch too large. Reduce max_bytes or request fewer paths.".to_string()
                }
                Some(McpAttemptArtifactBlockedReason::TooManyPaths) => {
                    "Too many paths. Reduce the number of paths requested.".to_string()
                }
                Some(McpAttemptArtifactBlockedReason::SummaryFailed) => {
                    "Patch retrieval blocked due to summary failure. Retry later.".to_string()
                }
                Some(McpAttemptArtifactBlockedReason::ThresholdExceeded) => {
                    if force {
                        "Patch blocked by guardrails even with force=true. Reduce max_bytes or request fewer paths."
                            .to_string()
                    } else {
                        "Patch blocked by diff preview guardrails. Retry with force=true to bypass."
                            .to_string()
                    }
                }
                None => "Patch blocked by guardrails.".to_string(),
            };

            return Self::err_with(
                "Patch blocked by guardrails.",
                Some(json!({
                    "attempt_id": attempt_id,
                    "paths": patch.paths,
                    "force": force,
                    "max_bytes": max_bytes,
                    "blocked_reason": blocked_reason,
                    "bytes": patch.bytes,
                    "truncated": patch.truncated,
                })),
                Some(hint),
                Some(MCP_CODE_BLOCKED_GUARDRAILS),
                Some(false),
            );
        }

        Self::success(&GetAttemptPatchResponse {
            attempt_id: attempt_id.to_string(),
            blocked: patch.blocked,
            blocked_reason,
            code: None,
            retryable: None,
            hint: None,
            truncated: patch.truncated,
            bytes: patch.bytes,
            paths: patch.paths,
            patch: patch.patch,
        })
    }

    #[tool(
        description = r#"Use when: List approvals for an attempt.
Required: attempt_id
Optional: status, limit, cursor
Next: get_approval, respond_approval
Avoid: Guessing attempt_id."#,
        output_schema = tool_output_schema::<ListApprovalsResponse>(),
        annotations(read_only_hint = true)
    )]
    async fn list_approvals(
        &self,
        Parameters(ListApprovalsRequest {
            attempt_id,
            status,
            limit,
            cursor,
        }): Parameters<ListApprovalsRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let status = status.and_then(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
        if let Some(status) = status.as_deref()
            && !matches!(status, "pending" | "approved" | "denied" | "timed_out")
        {
            return Self::err_with(
                "Invalid status filter",
                Some(json!({ "value": status })),
                Some("Valid values: pending, approved, denied, timed_out.".to_string()),
                Some("invalid_argument"),
                Some(false),
            );
        }

        let (approvals, next_cursor) = self
            .deployment
            .approvals()
            .list_approvals_by_attempt(
                &self.deployment.db().pool,
                attempt_id,
                status.as_deref(),
                limit.unwrap_or(50),
                cursor,
            )
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to list approvals",
                    Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
                )
            })?;

        let approvals = approvals
            .into_iter()
            .map(Self::approval_to_summary)
            .collect();
        Self::success(&ListApprovalsResponse {
            attempt_id: attempt_id.to_string(),
            approvals,
            next_cursor,
        })
    }

    #[tool(
        description = r#"Use when: Fetch approval details to render a prompt.
Required: approval_id
Optional: (none)
Next: respond_approval
Avoid: Assuming approval exists."#,
        output_schema = tool_output_schema::<GetApprovalResponse>(),
        annotations(read_only_hint = true)
    )]
    async fn get_approval(
        &self,
        Parameters(GetApprovalRequest { approval_id }): Parameters<GetApprovalRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let approval = self
            .deployment
            .approvals()
            .get_approval(&self.deployment.db().pool, &approval_id)
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to load approval",
                    Some(json!({ "error": e.to_string(), "approval_id": approval_id })),
                )
            })?;
        Self::success(&GetApprovalResponse {
            approval: Self::approval_to_summary(approval),
        })
    }

    #[tool(
        description = r#"Use when: Respond to a pending approval (approve/deny).
Required: approval_id, execution_process_id, status
Optional: denial_reason, responded_by_client_id, request_id
Next: tail_attempt_feed
Avoid: Responding with mismatched execution_process_id."#,
        output_schema = tool_output_schema::<RespondApprovalResponse>(),
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true
        )
    )]
    async fn respond_approval(
        &self,
        Parameters(RespondApprovalRequest {
            approval_id,
            execution_process_id,
            status,
            denial_reason,
            responded_by_client_id,
            request_id,
        }): Parameters<RespondApprovalRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let status_trim = status.trim().to_string();
        if !matches!(status_trim.as_str(), "approved" | "denied" | "timed_out") {
            return Self::err_with(
                "Invalid status",
                Some(json!({ "value": status })),
                Some("Valid values: approved, denied, timed_out.".to_string()),
                Some("invalid_argument"),
                Some(false),
            );
        }

        let responded_by_client_id = responded_by_client_id
            .and_then(|value| {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            })
            .or_else(|| self.default_peer_client_id())
            .or_else(|| Some("mcp:unknown".to_string()));

        #[derive(Serialize)]
        struct RespondIdempotencyPayload<'a> {
            approval_id: &'a str,
            execution_process_id: Uuid,
            status: &'a str,
            denial_reason: &'a Option<String>,
            responded_by_client_id: &'a Option<String>,
        }

        let hash = Self::request_hash(&RespondIdempotencyPayload {
            approval_id: &approval_id,
            execution_process_id,
            status: &status_trim,
            denial_reason: &denial_reason,
            responded_by_client_id: &responded_by_client_id,
        })?;
        let key = Self::stable_tool_idempotency_key(request_id);

        let response = match self
            .idempotent("respond_approval", key, hash, || async {
                let approval_status = match status_trim.as_str() {
                    "approved" => utils_core::approvals::ApprovalStatus::Approved,
                    "timed_out" => utils_core::approvals::ApprovalStatus::TimedOut,
                    "denied" => utils_core::approvals::ApprovalStatus::Denied {
                        reason: denial_reason.clone(),
                    },
                    _ => utils_core::approvals::ApprovalStatus::Denied {
                        reason: Some("invalid status".to_string()),
                    },
                };

                let (final_status, _) = self
                    .deployment
                    .approvals()
                    .respond_with_client_id(
                        &self.deployment.db().pool,
                        &approval_id,
                        utils_core::approvals::ApprovalResponse {
                            execution_process_id,
                            status: approval_status,
                        },
                        responded_by_client_id.clone(),
                    )
                    .await
                    .map_err(|e| {
                        ErrorData::internal_error(
                            "Failed to respond to approval",
                            Some(json!({
                                "error": e.to_string(),
                                "approval_id": approval_id,
                                "execution_process_id": execution_process_id,
                            })),
                        )
                    })?;

                let status_str = match final_status {
                    utils_core::approvals::ApprovalStatus::Pending => "pending".to_string(),
                    utils_core::approvals::ApprovalStatus::Approved => "approved".to_string(),
                    utils_core::approvals::ApprovalStatus::Denied { .. } => "denied".to_string(),
                    utils_core::approvals::ApprovalStatus::TimedOut => "timed_out".to_string(),
                };

                Ok(RespondApprovalResponse {
                    approval_id: approval_id.clone(),
                    status: status_str,
                })
            })
            .await
        {
            Ok(response) => response,
            Err(ToolOrRpcError::Tool(tool_error)) => return Ok(tool_error),
            Err(ToolOrRpcError::Rpc(err)) => return Err(err),
        };

        Self::success(&response)
    }

    #[tool(
        description = r#"Use when: Tail project activity events (incremental via after_event_id, or older paging via cursor).
Required: project_id
Optional: limit, cursor, after_event_id
Next: tail_task_activity
Avoid: Mixing cursor with after_event_id."#,
        output_schema = tool_output_schema::<TailActivityResponse>(),
        annotations(read_only_hint = true)
    )]
    async fn tail_project_activity(
        &self,
        Parameters(TailProjectActivityRequest {
            project_id,
            limit,
            cursor,
            after_event_id,
        }): Parameters<TailProjectActivityRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if cursor.is_some() && after_event_id.is_some() {
            return Self::err_with(
                "cursor and after_event_id are mutually exclusive.",
                Some(json!({ "cursor": cursor, "after_event_id": after_event_id })),
                Some("Use cursor to page older history; use after_event_id to fetch only new events.".to_string()),
                Some(MCP_CODE_MIXED_PAGINATION),
                Some(false),
            );
        }

        let limit = limit.unwrap_or(50).clamp(1, 200);
        let pool = &self.deployment.db().pool;

        let mut task_project_cache = HashMap::new();
        let mut session_project_cache = HashMap::new();

        let (events, next_cursor, has_more, next_after) =
            if let Some(after_event_id) = after_event_id {
                let mut events = Vec::new();
                let mut last_seen_id = after_event_id;

                loop {
                    let batch = EventOutbox::tail_after(pool, last_seen_id, limit)
                        .await
                        .map_err(|e| {
                            ErrorData::internal_error(
                                "Failed to tail events",
                                Some(json!({ "error": e.to_string() })),
                            )
                        })?;
                    if batch.is_empty() {
                        break;
                    }

                    for entry in batch {
                        last_seen_id = entry.id;
                        let Some(pid) = self
                            .project_id_for_event(
                                &entry,
                                &mut task_project_cache,
                                &mut session_project_cache,
                            )
                            .await
                        else {
                            continue;
                        };
                        if pid == project_id {
                            events.push(Self::activity_event_from_outbox(entry));
                            if events.len() >= limit as usize {
                                break;
                            }
                        }
                    }

                    if events.len() >= limit as usize {
                        break;
                    }
                }

                let has_more = events.len() >= limit as usize;
                (events, None, has_more, Some(last_seen_id))
            } else {
                let (page, next_cursor, has_more) = EventOutbox::page_older(pool, cursor, limit)
                    .await
                    .map_err(|e| {
                        ErrorData::internal_error(
                            "Failed to page events",
                            Some(json!({ "error": e.to_string() })),
                        )
                    })?;

                let mut events = Vec::new();
                for entry in page {
                    let Some(pid) = self
                        .project_id_for_event(
                            &entry,
                            &mut task_project_cache,
                            &mut session_project_cache,
                        )
                        .await
                    else {
                        continue;
                    };
                    if pid == project_id {
                        events.push(Self::activity_event_from_outbox(entry));
                    }
                }

                (events, next_cursor, has_more, None)
            };

        Self::success(&TailActivityResponse {
            events,
            next_cursor,
            has_more,
            next_after_event_id: next_after,
        })
    }

    #[tool(
        description = r#"Use when: Tail task activity events (incremental via after_event_id, or older paging via cursor).
Required: task_id
Optional: limit, cursor, after_event_id
Next: tail_attempt_feed
Avoid: Mixing cursor with after_event_id."#,
        output_schema = tool_output_schema::<TailActivityResponse>(),
        annotations(read_only_hint = true)
    )]
    async fn tail_task_activity(
        &self,
        Parameters(TailTaskActivityRequest {
            task_id,
            limit,
            cursor,
            after_event_id,
        }): Parameters<TailTaskActivityRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if cursor.is_some() && after_event_id.is_some() {
            return Self::err_with(
                "cursor and after_event_id are mutually exclusive.",
                Some(json!({ "cursor": cursor, "after_event_id": after_event_id })),
                Some("Use cursor to page older history; use after_event_id to fetch only new events.".to_string()),
                Some(MCP_CODE_MIXED_PAGINATION),
                Some(false),
            );
        }

        let limit = limit.unwrap_or(50).clamp(1, 200);
        let pool = &self.deployment.db().pool;

        let mut session_task_cache = HashMap::new();

        let (events, next_cursor, has_more, next_after) =
            if let Some(after_event_id) = after_event_id {
                let mut events = Vec::new();
                let mut last_seen_id = after_event_id;

                loop {
                    let batch = EventOutbox::tail_after(pool, last_seen_id, limit)
                        .await
                        .map_err(|e| {
                            ErrorData::internal_error(
                                "Failed to tail events",
                                Some(json!({ "error": e.to_string() })),
                            )
                        })?;
                    if batch.is_empty() {
                        break;
                    }

                    for entry in batch {
                        last_seen_id = entry.id;
                        let Some(tid) = self
                            .task_id_for_event(&entry, &mut session_task_cache)
                            .await
                        else {
                            continue;
                        };
                        if tid == task_id {
                            events.push(Self::activity_event_from_outbox(entry));
                            if events.len() >= limit as usize {
                                break;
                            }
                        }
                    }

                    if events.len() >= limit as usize {
                        break;
                    }
                }

                let has_more = events.len() >= limit as usize;
                (events, None, has_more, Some(last_seen_id))
            } else {
                let (page, next_cursor, has_more) = EventOutbox::page_older(pool, cursor, limit)
                    .await
                    .map_err(|e| {
                        ErrorData::internal_error(
                            "Failed to page events",
                            Some(json!({ "error": e.to_string() })),
                        )
                    })?;

                let mut events = Vec::new();
                for entry in page {
                    let Some(tid) = self
                        .task_id_for_event(&entry, &mut session_task_cache)
                        .await
                    else {
                        continue;
                    };
                    if tid == task_id {
                        events.push(Self::activity_event_from_outbox(entry));
                    }
                }

                (events, next_cursor, has_more, None)
            };

        Self::success(&TailActivityResponse {
            events,
            next_cursor,
            has_more,
            next_after_event_id: next_after,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        path::Path,
        process::Command,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use db::models::{
        execution_process::CreateExecutionProcess, repo::Repo, session::CreateSession,
    };
    use deployment::Deployment;
    use executors_protocol::actions::{ExecutorActionType, script::ScriptContext};
    use rmcp::{
        ServiceExt,
        handler::{client::ClientHandler, server::tool::IntoCallToolResult},
    };
    use services::services::config::DiffPreviewGuardPreset;

    use super::*;
    use crate::test_support::TestEnvGuard;

    #[derive(Clone)]
    struct TestElicitationClient {
        info: rmcp::model::ClientInfo,
        response: serde_json::Value,
        create_elicitation_calls: Arc<AtomicUsize>,
    }

    impl TestElicitationClient {
        fn new(info: rmcp::model::ClientInfo, response: serde_json::Value) -> Self {
            Self {
                info,
                response,
                create_elicitation_calls: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn call_count(&self) -> usize {
            self.create_elicitation_calls.load(Ordering::SeqCst)
        }
    }

    impl ClientHandler for TestElicitationClient {
        fn get_info(&self) -> rmcp::model::ClientInfo {
            self.info.clone()
        }

        fn create_elicitation(
            &self,
            request: rmcp::model::CreateElicitationRequestParams,
            context: rmcp::service::RequestContext<rmcp::RoleClient>,
        ) -> impl std::future::Future<
            Output = Result<rmcp::model::CreateElicitationResult, rmcp::ErrorData>,
        > + Send
        + '_ {
            let response = self.response.clone();
            let calls = self.create_elicitation_calls.clone();
            async move {
                let _ = (request, context);
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(rmcp::model::CreateElicitationResult {
                    action: rmcp::model::ElicitationAction::Accept,
                    content: Some(response),
                })
            }
        }
    }

    #[test]
    fn tool_router_includes_feed_and_approvals_tools() {
        let router = build_tool_router();
        assert!(router.map.contains_key("tail_attempt_feed"));
        assert!(router.map.contains_key("respond_approval"));
        assert!(router.map.contains_key("claim_attempt_control"));
    }

    #[test]
    fn guidance_registry_only_references_existing_tools() {
        let tools = build_tool_router().list_all();
        let tool_names: HashSet<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();

        for name in crate::mcp::params::guidance_referenced_tool_names() {
            assert!(
                tool_names.contains(name),
                "Guidance references missing tool: {}",
                name
            );
        }
    }

    #[test]
    fn tool_router_exposes_output_schema_for_key_tools() {
        let tools = build_tool_router().list_all();

        let tool = |name: &str| {
            tools
                .iter()
                .find(|tool| tool.name.as_ref() == name)
                .unwrap_or_else(|| panic!("Missing tool: {}", name))
        };

        let expected_tool_names = [
            "archive_project_kanban",
            "claim_attempt_control",
            "cli_dependency_preflight",
            "create_task",
            "delete_task",
            "get_approval",
            "get_attempt_changes",
            "get_attempt_control",
            "get_attempt_file",
            "get_attempt_patch",
            "get_task",
            "list_archived_kanbans",
            "list_approvals",
            "list_executors",
            "list_projects",
            "list_repos",
            "list_task_attempts",
            "list_tasks",
            "release_attempt_control",
            "respond_approval",
            "restore_archived_kanban",
            "send_follow_up",
            "start_attempt",
            "stop_attempt",
            "tail_attempt_feed",
            "tail_project_activity",
            "tail_session_messages",
            "tail_task_activity",
            "update_task",
        ];

        for name in expected_tool_names {
            let tool = tool(name);
            assert!(
                tool.output_schema.is_some(),
                "Expected outputSchema for {}",
                name
            );
        }

        for name in ["list_projects", "list_tasks", "tail_attempt_feed"] {
            let tool = tool(name);
            assert_eq!(
                tool.output_schema
                    .as_ref()
                    .and_then(|schema| schema.get("type"))
                    .and_then(|t| t.as_str()),
                Some("object"),
                "Expected outputSchema root type=object for {}",
                name
            );
        }
    }

    #[test]
    fn tool_router_exposes_annotations_for_key_tools() {
        let tools = build_tool_router().list_all();

        let tool = |name: &str| {
            tools
                .iter()
                .find(|tool| tool.name.as_ref() == name)
                .unwrap_or_else(|| panic!("Missing tool: {}", name))
        };

        for name in [
            "list_projects",
            "list_tasks",
            "tail_attempt_feed",
            "get_attempt_changes",
        ] {
            let annotations = tool(name)
                .annotations
                .as_ref()
                .unwrap_or_else(|| panic!("Missing annotations for {}", name));
            assert_eq!(
                annotations.read_only_hint,
                Some(true),
                "Expected readOnlyHint=true for {}",
                name
            );
        }

        let create_task = tool("create_task")
            .annotations
            .as_ref()
            .expect("Missing create_task annotations");
        assert_eq!(create_task.read_only_hint, Some(false));
        assert_eq!(create_task.destructive_hint, Some(false));
        assert_eq!(create_task.idempotent_hint, Some(true));

        let delete_task = tool("delete_task")
            .annotations
            .as_ref()
            .expect("Missing delete_task annotations");
        assert_eq!(delete_task.read_only_hint, Some(false));
        assert_eq!(delete_task.destructive_hint, Some(true));
        assert_eq!(delete_task.idempotent_hint, Some(true));

        let list_archived_kanbans = tool("list_archived_kanbans")
            .annotations
            .as_ref()
            .expect("Missing list_archived_kanbans annotations");
        assert_eq!(list_archived_kanbans.read_only_hint, Some(true));

        let archive_project_kanban = tool("archive_project_kanban")
            .annotations
            .as_ref()
            .expect("Missing archive_project_kanban annotations");
        assert_eq!(archive_project_kanban.read_only_hint, Some(false));
        assert_eq!(archive_project_kanban.destructive_hint, Some(true));

        let restore_archived_kanban = tool("restore_archived_kanban")
            .annotations
            .as_ref()
            .expect("Missing restore_archived_kanban annotations");
        assert_eq!(restore_archived_kanban.read_only_hint, Some(false));
        assert_eq!(restore_archived_kanban.destructive_hint, Some(true));
        assert_eq!(restore_archived_kanban.idempotent_hint, Some(true));
    }

    #[test]
    fn tool_router_marks_large_attempt_tools_as_task_optional() {
        let tools = build_tool_router().list_all();

        let tool = |name: &str| {
            tools
                .iter()
                .find(|tool| tool.name.as_ref() == name)
                .unwrap_or_else(|| panic!("Missing tool: {}", name))
        };

        for name in [
            "get_attempt_changes",
            "get_attempt_file",
            "get_attempt_patch",
            "start_attempt",
        ] {
            let execution = tool(name)
                .execution
                .as_ref()
                .unwrap_or_else(|| panic!("Missing execution for {}", name));
            assert_eq!(
                execution.task_support,
                Some(rmcp::model::TaskSupport::Optional),
                "Expected taskSupport=optional for {}",
                name
            );
        }
    }

    #[test]
    fn tool_router_marks_follow_up_and_stop_as_task_optional() {
        let tools = build_tool_router().list_all();

        let tool = |name: &str| {
            tools
                .iter()
                .find(|tool| tool.name.as_ref() == name)
                .unwrap_or_else(|| panic!("Missing tool: {}", name))
        };

        for name in ["send_follow_up", "stop_attempt"] {
            let execution = tool(name)
                .execution
                .as_ref()
                .unwrap_or_else(|| panic!("Missing execution for {}", name));
            assert_eq!(
                execution.task_support,
                Some(rmcp::model::TaskSupport::Optional),
                "Expected taskSupport=optional for {}",
                name
            );
        }
    }

    #[test]
    fn tool_router_marks_archived_kanban_tools_as_task_optional() {
        let tools = build_tool_router().list_all();

        let tool = |name: &str| {
            tools
                .iter()
                .find(|tool| tool.name.as_ref() == name)
                .unwrap_or_else(|| panic!("Missing tool: {}", name))
        };

        for name in ["archive_project_kanban", "restore_archived_kanban"] {
            let execution = tool(name)
                .execution
                .as_ref()
                .unwrap_or_else(|| panic!("Missing execution for {}", name));
            assert_eq!(
                execution.task_support,
                Some(rmcp::model::TaskSupport::Optional),
                "Expected taskSupport=optional for {}",
                name
            );
        }
    }

    #[test]
    fn tool_router_does_not_expose_delete_archived_kanban() {
        let tools = build_tool_router().list_all();
        assert!(
            tools
                .iter()
                .all(|tool| tool.name.as_ref() != "delete_archived_kanban")
        );
    }

    #[tokio::test]
    async fn server_info_declares_latest_protocol_and_tasks_capability() {
        let temp_root = std::env::temp_dir().join(format!("vk-mcp-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _guard = TestEnvGuard::new(&temp_root, "sqlite::memory:".to_string());
        let deployment = DeploymentImpl::new().await.expect("deployment");
        let server = TaskServer::new(deployment);

        let info = server.get_info();
        assert_eq!(info.protocol_version, ProtocolVersion::LATEST);

        let tasks = info
            .capabilities
            .tasks
            .as_ref()
            .expect("Missing tasks capability");
        assert!(tasks.supports_list());
        assert!(tasks.supports_cancel());
        assert!(tasks.supports_tools_call());

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn list_projects_and_list_tasks_return_structured_content() {
        let temp_root = std::env::temp_dir().join(format!("vk-mcp-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _guard = TestEnvGuard::new(&temp_root, "sqlite::memory:".to_string());

        let deployment = DeploymentImpl::new().await.unwrap();
        let pool = deployment.db().pool.clone();

        let project_id = Uuid::new_v4();
        Project::create(
            &pool,
            &db::models::project::CreateProject {
                name: "Test project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        Task::create(
            &pool,
            &CreateTask::from_title_description(
                project_id,
                "Test task".to_string(),
                Some("Test description".to_string()),
            ),
            task_id,
        )
        .await
        .unwrap();

        let server = TaskServer::new(deployment);

        let list_projects_result = server
            .list_projects()
            .await
            .into_call_tool_result()
            .unwrap();
        assert!(list_projects_result.structured_content.is_some());

        let list_tasks_result = server
            .list_tasks(Parameters(ListTasksRequest {
                project_id,
                status: None,
                limit: Some(10),
            }))
            .await
            .into_call_tool_result()
            .unwrap();
        assert!(list_tasks_result.structured_content.is_some());
    }

    #[tokio::test]
    async fn start_attempt_rejects_archived_tasks_with_structured_error() {
        let temp_root = std::env::temp_dir().join(format!("vk-mcp-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _guard = TestEnvGuard::new(&temp_root, "sqlite::memory:".to_string());

        let deployment = DeploymentImpl::new().await.unwrap();
        let pool = deployment.db().pool.clone();

        let project_id = Uuid::new_v4();
        Project::create(
            &pool,
            &db::models::project::CreateProject {
                name: "Archive guard".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        Task::create(
            &pool,
            &CreateTask::from_title_description(project_id, "Archived task".to_string(), None),
            task_id,
        )
        .await
        .unwrap();

        let project = Project::find_by_id(&pool, project_id)
            .await
            .unwrap()
            .unwrap();
        let _ = crate::routes::archived_kanbans::archive_project_kanban(
            axum::Extension(project),
            axum::extract::State(deployment.clone()),
            axum::Json(
                crate::routes::archived_kanbans::ArchiveProjectKanbanRequest {
                    statuses: vec![TaskStatus::Todo],
                    title: Some("Test archive".to_string()),
                },
            ),
        )
        .await
        .unwrap();

        let server = TaskServer::new(deployment);
        let result = server
            .start_attempt(Parameters(StartAttemptRequest {
                task_id,
                executor: "CLAUDE_CODE".to_string(),
                variant: None,
                repos: Vec::new(),
                request_id: None,
                prompt: None,
            }))
            .await
            .into_call_tool_result()
            .unwrap();

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.expect("structured content");
        assert_eq!(
            structured.get("code").and_then(|v| v.as_str()),
            Some(MCP_CODE_BLOCKED_GUARDRAILS)
        );
    }

    #[tokio::test]
    async fn list_tasks_missing_project_id_is_guided_invalid_params_error_over_mcp() {
        let temp_root = std::env::temp_dir().join(format!("vk-mcp-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _guard = TestEnvGuard::new(&temp_root, "sqlite::memory:".to_string());

        let deployment = DeploymentImpl::new().await.unwrap();
        let pool = &deployment.db().pool;

        let project_id = Uuid::new_v4();
        Project::create(
            pool,
            &db::models::project::CreateProject {
                name: "Test project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let client = TestElicitationClient::new(
            rmcp::model::ClientInfo {
                meta: None,
                protocol_version: ProtocolVersion::V_2025_03_26,
                capabilities: rmcp::model::ClientCapabilities::default(),
                client_info: rmcp::model::Implementation {
                    name: "vk-tool-client".to_string(),
                    title: None,
                    version: "0.0.42".to_string(),
                    description: None,
                    icons: None,
                    website_url: None,
                },
            },
            serde_json::Value::Null,
        );

        let server = TaskServer::new(deployment);

        let (server_io, client_io) = tokio::io::duplex(1024 * 1024);
        let (server_running, client_running) =
            tokio::join!(server.serve(server_io), client.clone().serve(client_io));
        let server_running = server_running.unwrap();
        let client_running = client_running.unwrap();

        let err = client_running
            .call_tool(rmcp::model::CallToolRequestParams {
                meta: None,
                name: "list_tasks".into(),
                arguments: None,
                task: None,
            })
            .await
            .expect_err("expected invalid params error");

        let rmcp::service::ServiceError::McpError(err) = err else {
            panic!("Expected MCP error, got: {err:?}");
        };

        assert_eq!(
            err.message, "Missing required field(s): project_id",
            "Inspector headline should be actionable"
        );

        let payload = err.data.expect("structured data payload");
        assert_eq!(
            payload.get("code").and_then(|v| v.as_str()),
            Some("missing_required")
        );
        assert_eq!(
            payload.get("retryable").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert!(
            payload
                .get("hint")
                .and_then(|v| v.as_str())
                .is_some_and(|h| h.contains("list_projects")),
            "Hint should guide id discovery"
        );

        let details = payload
            .get("details")
            .and_then(|v| v.as_object())
            .expect("details");
        assert_eq!(
            details
                .get("missing_fields")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>()),
            Some(vec!["project_id"])
        );
        assert!(
            details
                .get("next_tools")
                .and_then(|v| v.as_array())
                .is_some_and(|steps| {
                    steps.iter().any(|step| {
                        step.get("tool")
                            .and_then(|v| v.as_str())
                            .is_some_and(|name| name == "list_projects")
                    })
                }),
            "next_tools should suggest list_projects"
        );
        assert!(details.get("example_args").is_some());

        let mut args = serde_json::Map::new();
        args.insert("project_id".to_string(), json!(project_id));
        let result = client_running
            .call_tool(rmcp::model::CallToolRequestParams {
                meta: None,
                name: "list_tasks".into(),
                arguments: Some(args),
                task: None,
            })
            .await
            .expect("expected successful tool call");

        assert_eq!(result.is_error, Some(false));

        let _ = client_running.cancel().await;
        let _ = server_running.cancel().await;
        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn list_tasks_invalid_status_is_structured_invalid_argument() {
        let temp_root = std::env::temp_dir().join(format!("vk-mcp-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _guard = TestEnvGuard::new(&temp_root, "sqlite::memory:".to_string());

        let deployment = DeploymentImpl::new().await.unwrap();
        let server = TaskServer::new(deployment);

        let project_id = Uuid::new_v4();
        let err = server
            .list_tasks(Parameters(ListTasksRequest {
                project_id,
                status: Some("not-a-status".to_string()),
                limit: None,
            }))
            .await
            .err()
            .expect("expected invalid status error");

        assert_eq!(err.message, "Invalid status filter");

        let payload = err.data.expect("structured data payload");
        assert_eq!(
            payload.get("code").and_then(|v| v.as_str()),
            Some("invalid_argument")
        );
        assert_eq!(
            payload.get("retryable").and_then(|v| v.as_bool()),
            Some(false)
        );

        let details = payload
            .get("details")
            .and_then(|v| v.as_object())
            .expect("details");
        assert_eq!(details.get("path").and_then(|v| v.as_str()), Some("status"));
        assert_eq!(
            details.get("value").and_then(|v| v.as_str()),
            Some("not-a-status")
        );
        assert!(details.get("valid_values").is_some());
        assert!(details.get("example_args").is_some());

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn tail_attempt_feed_rejects_mixed_pagination_with_structured_error() {
        let temp_root = std::env::temp_dir().join(format!("vk-mcp-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _guard = TestEnvGuard::new(&temp_root, "sqlite::memory:".to_string());

        let deployment = DeploymentImpl::new().await.unwrap();
        let server = TaskServer::new(deployment);

        let attempt_id = Uuid::new_v4();
        let result = server
            .tail_attempt_feed(Parameters(TailAttemptFeedRequest {
                attempt_id,
                limit: Some(10),
                cursor: Some(123),
                after_log_index: Some(1),
                wait_ms: None,
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));

        let structured = result
            .structured_content
            .clone()
            .expect("structured_content should be present");

        assert_eq!(structured["code"].as_str(), Some(MCP_CODE_MIXED_PAGINATION));
        assert!(structured["hint"].as_str().is_some());
    }

    #[tokio::test]
    async fn tail_attempt_feed_wait_ms_requires_after_log_index_is_structured_error() {
        let temp_root = std::env::temp_dir().join(format!("vk-mcp-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _guard = TestEnvGuard::new(&temp_root, "sqlite::memory:".to_string());

        let deployment = DeploymentImpl::new().await.unwrap();
        let server = TaskServer::new(deployment);

        let attempt_id = Uuid::new_v4();
        let result = server
            .tail_attempt_feed(Parameters(TailAttemptFeedRequest {
                attempt_id,
                limit: Some(10),
                cursor: None,
                after_log_index: None,
                wait_ms: Some(10),
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
        let structured = result
            .structured_content
            .clone()
            .expect("structured_content should be present");
        assert_eq!(
            structured["code"].as_str(),
            Some(MCP_CODE_WAIT_MS_REQUIRES_AFTER_LOG_INDEX)
        );

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn tail_attempt_feed_wait_ms_too_large_is_structured_error() {
        let temp_root = std::env::temp_dir().join(format!("vk-mcp-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _guard = TestEnvGuard::new(&temp_root, "sqlite::memory:".to_string());

        let deployment = DeploymentImpl::new().await.unwrap();
        let server = TaskServer::new(deployment);

        let attempt_id = Uuid::new_v4();
        let result = server
            .tail_attempt_feed(Parameters(TailAttemptFeedRequest {
                attempt_id,
                limit: Some(10),
                cursor: None,
                after_log_index: Some(0),
                wait_ms: Some(TAIL_ATTEMPT_FEED_MAX_WAIT_MS + 1),
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
        let structured = result
            .structured_content
            .clone()
            .expect("structured_content should be present");
        assert_eq!(
            structured["code"].as_str(),
            Some(MCP_CODE_WAIT_MS_TOO_LARGE)
        );

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn tail_attempt_feed_after_log_index_is_incremental_and_ordered() {
        let temp_root = std::env::temp_dir().join(format!("vk-mcp-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _guard = TestEnvGuard::new(&temp_root, "sqlite::memory:".to_string());

        let deployment = DeploymentImpl::new().await.unwrap();
        let pool = deployment.db().pool.clone();

        let project_id = Uuid::new_v4();
        Project::create(
            &pool,
            &db::models::project::CreateProject {
                name: "Test project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        Task::create(
            &pool,
            &CreateTask::from_title_description(
                project_id,
                "Test task".to_string(),
                Some("Test description".to_string()),
            ),
            task_id,
        )
        .await
        .unwrap();

        let repo = Repo::find_or_create(&pool, Path::new("/tmp/vk-test-repo"), "Test repo")
            .await
            .unwrap();

        let attempt_id = Uuid::new_v4();
        let workspace = Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "test-branch".to_string(),
                agent_working_dir: None,
            },
            attempt_id,
            task_id,
        )
        .await
        .unwrap();

        WorkspaceRepo::create_many(
            &pool,
            workspace.id,
            &[CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch: "main".to_string(),
            }],
        )
        .await
        .unwrap();

        let session = Session::create(
            &pool,
            &CreateSession {
                executor: Some("CLAUDE_CODE".to_string()),
            },
            Uuid::new_v4(),
            workspace.id,
        )
        .await
        .unwrap();

        let execution_process_id = Uuid::new_v4();
        let _execution_process = ExecutionProcess::create(
            &pool,
            &CreateExecutionProcess {
                session_id: session.id,
                executor_action: ExecutorAction::new(
                    ExecutorActionType::ScriptRequest(
                        executors_protocol::actions::script::ScriptRequest {
                            language:
                                executors_protocol::actions::script::ScriptRequestLanguage::Bash,
                            script: "echo hello".to_string(),
                            context: ScriptContext::SetupScript,
                            working_dir: None,
                        },
                    ),
                    None,
                ),
                run_reason: ExecutionProcessRunReason::CodingAgent,
            },
            execution_process_id,
            &[CreateExecutionProcessRepoState {
                repo_id: repo.id,
                before_head_commit: None,
                after_head_commit: None,
                merge_commit: None,
            }],
        )
        .await
        .unwrap();

        // Seed normalized log entries in the DB: entry_index 0..=4.
        for idx in 0..=4i64 {
            let entry_json = serde_json::json!({ "type": "test_log", "n": idx });
            db::models::execution_process_log_entries::ExecutionProcessLogEntry::upsert_entry(
                &pool,
                execution_process_id,
                utils_core::log_entries::LogEntryChannel::Normalized,
                idx,
                &entry_json.to_string(),
            )
            .await
            .unwrap();
        }

        let server = TaskServer::new(deployment);

        let result = server
            .tail_attempt_feed(Parameters(TailAttemptFeedRequest {
                attempt_id,
                limit: Some(2),
                cursor: None,
                after_log_index: Some(1),
                wait_ms: None,
            }))
            .await
            .unwrap();

        assert!(result.structured_content.is_some());

        let text = result.content[0].as_text().unwrap().text.clone();
        let payload: serde_json::Value = serde_json::from_str(&text).unwrap();

        let entries = payload["page"]["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["entry_index"], 2);
        assert_eq!(entries[1]["entry_index"], 3);
        assert_eq!(payload["next_after_log_index"], 3);

        // Next poll continues from next_after_log_index.
        let result = server
            .tail_attempt_feed(Parameters(TailAttemptFeedRequest {
                attempt_id,
                limit: Some(2),
                cursor: None,
                after_log_index: Some(3),
                wait_ms: None,
            }))
            .await
            .unwrap();
        let text = result.content[0].as_text().unwrap().text.clone();
        let payload: serde_json::Value = serde_json::from_str(&text).unwrap();
        let entries = payload["page"]["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["entry_index"], 4);
        assert_eq!(payload["next_after_log_index"], 4);

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn tail_attempt_feed_wait_ms_returns_when_new_log_appears() {
        let temp_root = std::env::temp_dir().join(format!("vk-mcp-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _guard = TestEnvGuard::new(&temp_root, "sqlite::memory:".to_string());

        let deployment = DeploymentImpl::new().await.unwrap();
        let pool = deployment.db().pool.clone();

        let project_id = Uuid::new_v4();
        Project::create(
            &pool,
            &db::models::project::CreateProject {
                name: "Test project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        Task::create(
            &pool,
            &CreateTask::from_title_description(
                project_id,
                "Test task".to_string(),
                Some("Test description".to_string()),
            ),
            task_id,
        )
        .await
        .unwrap();

        let repo = Repo::find_or_create(&pool, Path::new("/tmp/vk-test-repo"), "Test repo")
            .await
            .unwrap();

        let attempt_id = Uuid::new_v4();
        let workspace = Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "test-branch".to_string(),
                agent_working_dir: None,
            },
            attempt_id,
            task_id,
        )
        .await
        .unwrap();

        WorkspaceRepo::create_many(
            &pool,
            workspace.id,
            &[CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch: "main".to_string(),
            }],
        )
        .await
        .unwrap();

        let session = Session::create(
            &pool,
            &CreateSession {
                executor: Some("CLAUDE_CODE".to_string()),
            },
            Uuid::new_v4(),
            workspace.id,
        )
        .await
        .unwrap();

        let execution_process_id = Uuid::new_v4();
        let _execution_process = ExecutionProcess::create(
            &pool,
            &CreateExecutionProcess {
                session_id: session.id,
                executor_action: ExecutorAction::new(
                    ExecutorActionType::ScriptRequest(
                        executors_protocol::actions::script::ScriptRequest {
                            language:
                                executors_protocol::actions::script::ScriptRequestLanguage::Bash,
                            script: "echo hello".to_string(),
                            context: ScriptContext::SetupScript,
                            working_dir: None,
                        },
                    ),
                    None,
                ),
                run_reason: ExecutionProcessRunReason::CodingAgent,
            },
            execution_process_id,
            &[CreateExecutionProcessRepoState {
                repo_id: repo.id,
                before_head_commit: None,
                after_head_commit: None,
                merge_commit: None,
            }],
        )
        .await
        .unwrap();

        // Seed normalized log entries in the DB: entry_index 0..=4.
        for idx in 0..=4i64 {
            let entry_json = serde_json::json!({ "type": "test_log", "n": idx });
            db::models::execution_process_log_entries::ExecutionProcessLogEntry::upsert_entry(
                &pool,
                execution_process_id,
                utils_core::log_entries::LogEntryChannel::Normalized,
                idx,
                &entry_json.to_string(),
            )
            .await
            .unwrap();
        }

        // Insert a new log entry shortly after the call starts waiting.
        let pool2 = pool.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let entry_json = serde_json::json!({ "type": "test_log", "n": 5 });
            db::models::execution_process_log_entries::ExecutionProcessLogEntry::upsert_entry(
                &pool2,
                execution_process_id,
                utils_core::log_entries::LogEntryChannel::Normalized,
                5,
                &entry_json.to_string(),
            )
            .await
            .unwrap();
        });

        let server = TaskServer::new(deployment);
        let result = server
            .tail_attempt_feed(Parameters(TailAttemptFeedRequest {
                attempt_id,
                limit: Some(10),
                cursor: None,
                after_log_index: Some(4),
                wait_ms: Some(2_000),
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(false));
        let text = result.content[0].as_text().unwrap().text.clone();
        let payload: serde_json::Value = serde_json::from_str(&text).unwrap();
        let entries = payload["page"]["entries"].as_array().unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["entry_index"], 5);
        assert_eq!(payload["next_after_log_index"], 5);

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn attempt_control_lease_claim_conflict_and_release_are_structured() {
        let temp_root = std::env::temp_dir().join(format!("vk-mcp-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _guard = TestEnvGuard::new(&temp_root, "sqlite::memory:".to_string());

        let deployment = DeploymentImpl::new().await.unwrap();
        let pool = deployment.db().pool.clone();

        let project_id = Uuid::new_v4();
        Project::create(
            &pool,
            &db::models::project::CreateProject {
                name: "Test project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        Task::create(
            &pool,
            &CreateTask::from_title_description(project_id, "Test task".to_string(), None),
            task_id,
        )
        .await
        .unwrap();

        let attempt_id = Uuid::new_v4();
        Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "main".to_string(),
                agent_working_dir: None,
            },
            attempt_id,
            task_id,
        )
        .await
        .unwrap();

        let server = TaskServer::new(deployment);

        let first = server
            .claim_attempt_control(Parameters(ClaimAttemptControlRequest {
                attempt_id,
                ttl_secs: Some(3600),
                force: None,
                claimed_by_client_id: Some("client-a".to_string()),
            }))
            .await
            .unwrap();
        assert_eq!(first.is_error, Some(false));
        let first_payload = first.structured_content.clone().unwrap();
        let control_token = Uuid::parse_str(first_payload["control_token"].as_str().unwrap())
            .expect("control_token should be a UUID string");

        let conflict = server
            .claim_attempt_control(Parameters(ClaimAttemptControlRequest {
                attempt_id,
                ttl_secs: Some(3600),
                force: Some(false),
                claimed_by_client_id: Some("client-b".to_string()),
            }))
            .await
            .unwrap();
        assert_eq!(conflict.is_error, Some(true));
        let conflict_payload = conflict.structured_content.clone().unwrap();
        assert_eq!(
            conflict_payload["code"].as_str(),
            Some(MCP_CODE_ATTEMPT_CLAIM_CONFLICT)
        );
        assert_eq!(
            conflict_payload["details"]["claimed_by_client_id"].as_str(),
            Some("client-a")
        );

        let status = server
            .get_attempt_control(Parameters(GetAttemptControlRequest { attempt_id }))
            .await
            .unwrap();
        assert_eq!(status.is_error, Some(false));
        let status_payload = status.structured_content.clone().unwrap();
        assert_eq!(status_payload["has_lease"], true);
        assert_eq!(
            status_payload["claimed_by_client_id"].as_str(),
            Some("client-a")
        );

        let mismatch = server
            .release_attempt_control(Parameters(ReleaseAttemptControlRequest {
                attempt_id,
                control_token: Uuid::new_v4(),
            }))
            .await
            .unwrap();
        assert_eq!(mismatch.is_error, Some(true));
        let mismatch_payload = mismatch.structured_content.clone().unwrap();
        assert_eq!(
            mismatch_payload["code"].as_str(),
            Some(MCP_CODE_INVALID_CONTROL_TOKEN)
        );

        let released = server
            .release_attempt_control(Parameters(ReleaseAttemptControlRequest {
                attempt_id,
                control_token,
            }))
            .await
            .unwrap();
        assert_eq!(released.is_error, Some(false));
        let released_payload = released.structured_content.clone().unwrap();
        assert_eq!(released_payload["released"], true);

        let status = server
            .get_attempt_control(Parameters(GetAttemptControlRequest { attempt_id }))
            .await
            .unwrap();
        let status_payload = status.structured_content.clone().unwrap();
        assert_eq!(status_payload["has_lease"], false);

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn mutating_attempt_tools_require_control_token() {
        let temp_root = std::env::temp_dir().join(format!("vk-mcp-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _guard = TestEnvGuard::new(&temp_root, "sqlite::memory:".to_string());

        let deployment = DeploymentImpl::new().await.unwrap();
        let pool = deployment.db().pool.clone();

        let project_id = Uuid::new_v4();
        Project::create(
            &pool,
            &db::models::project::CreateProject {
                name: "Test project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        Task::create(
            &pool,
            &CreateTask::from_title_description(project_id, "Test task".to_string(), None),
            task_id,
        )
        .await
        .unwrap();

        let attempt_id = Uuid::new_v4();
        Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "main".to_string(),
                agent_working_dir: None,
            },
            attempt_id,
            task_id,
        )
        .await
        .unwrap();

        let session_id = Uuid::new_v4();
        Session::create(
            &pool,
            &CreateSession {
                executor: Some("test".to_string()),
            },
            session_id,
            attempt_id,
        )
        .await
        .unwrap();

        let server = TaskServer::new(deployment);

        // No lease yet -> claim required.
        let result = server
            .send_follow_up(Parameters(SendFollowUpRequest {
                attempt_id: Some(attempt_id),
                session_id: None,
                control_token: None,
                prompt: "hi".to_string(),
                variant: None,
                request_id: None,
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let payload = result.structured_content.clone().unwrap();
        assert_eq!(
            payload["code"].as_str(),
            Some(MCP_CODE_ATTEMPT_CLAIM_REQUIRED)
        );

        let ttl = chrono::Duration::seconds(3600);
        let lease = match attempt_control_lease_model::claim(
            &pool,
            attempt_id,
            "client-a".to_string(),
            ttl,
            false,
        )
        .await
        .unwrap()
        {
            attempt_control_lease_model::ClaimOutcome::Claimed { lease, .. } => lease,
            other => panic!("Expected claimed lease, got: {:?}", other),
        };

        // Lease exists but no token provided -> conflict.
        let result = server
            .send_follow_up(Parameters(SendFollowUpRequest {
                attempt_id: Some(attempt_id),
                session_id: None,
                control_token: None,
                prompt: "hi".to_string(),
                variant: None,
                request_id: None,
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let payload = result.structured_content.clone().unwrap();
        assert_eq!(
            payload["code"].as_str(),
            Some(MCP_CODE_ATTEMPT_CLAIM_CONFLICT)
        );

        // Wrong token -> invalid_control_token.
        let result = server
            .send_follow_up(Parameters(SendFollowUpRequest {
                attempt_id: Some(attempt_id),
                session_id: None,
                control_token: Some(Uuid::new_v4()),
                prompt: "hi".to_string(),
                variant: None,
                request_id: None,
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let payload = result.structured_content.clone().unwrap();
        assert_eq!(
            payload["code"].as_str(),
            Some(MCP_CODE_INVALID_CONTROL_TOKEN)
        );

        // stop_attempt follows the same rules.
        let result = server
            .stop_attempt(Parameters(StopAttemptRequest {
                attempt_id,
                control_token: None,
                force: None,
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let payload = result.structured_content.clone().unwrap();
        assert_eq!(
            payload["code"].as_str(),
            Some(MCP_CODE_ATTEMPT_CLAIM_CONFLICT)
        );

        let result = server
            .stop_attempt(Parameters(StopAttemptRequest {
                attempt_id,
                control_token: Some(Uuid::new_v4()),
                force: None,
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let payload = result.structured_content.clone().unwrap();
        assert_eq!(
            payload["code"].as_str(),
            Some(MCP_CODE_INVALID_CONTROL_TOKEN)
        );

        // Correct token passes validation.
        let result = server
            .stop_attempt(Parameters(StopAttemptRequest {
                attempt_id,
                control_token: Some(lease.control_token),
                force: None,
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(false));

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn respond_approval_derives_responded_by_client_id_from_peer() {
        let temp_root = std::env::temp_dir().join(format!("vk-mcp-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _guard = TestEnvGuard::new(&temp_root, "sqlite::memory:".to_string());

        let deployment = DeploymentImpl::new().await.unwrap();
        let pool = &deployment.db().pool;

        let client = TestElicitationClient::new(
            rmcp::model::ClientInfo {
                meta: None,
                protocol_version: ProtocolVersion::V_2025_03_26,
                capabilities: rmcp::model::ClientCapabilities::default(),
                client_info: rmcp::model::Implementation {
                    name: "vk-tool-client".to_string(),
                    title: None,
                    version: "0.0.42".to_string(),
                    description: None,
                    icons: None,
                    website_url: None,
                },
            },
            serde_json::Value::Null,
        );

        let server = TaskServer::new(deployment.clone());

        let (server_io, client_io) = tokio::io::duplex(1024 * 1024);
        let (server_running, client_running) =
            tokio::join!(server.serve(server_io), client.clone().serve(client_io));
        let server_running = server_running.unwrap();
        let client_running = client_running.unwrap();

        assert!(server_running.service().peer.read().unwrap().is_some());

        let project_id = Uuid::new_v4();
        Project::create(
            pool,
            &db::models::project::CreateProject {
                name: "Test project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        Task::create(
            pool,
            &CreateTask::from_title_description(project_id, "Test task".to_string(), None),
            task_id,
        )
        .await
        .unwrap();

        let attempt_id = Uuid::new_v4();
        Workspace::create(
            pool,
            &CreateWorkspace {
                branch: "main".to_string(),
                agent_working_dir: None,
            },
            attempt_id,
            task_id,
        )
        .await
        .unwrap();

        let session_id = Uuid::new_v4();
        Session::create(
            pool,
            &CreateSession {
                executor: Some("test".to_string()),
            },
            session_id,
            attempt_id,
        )
        .await
        .unwrap();

        let execution_process_id = Uuid::new_v4();
        ExecutionProcess::create(
            pool,
            &CreateExecutionProcess {
                session_id,
                executor_action: ExecutorAction::new(
                    ExecutorActionType::ScriptRequest(
                        executors_protocol::actions::script::ScriptRequest {
                            script: "echo hi".to_string(),
                            language:
                                executors_protocol::actions::script::ScriptRequestLanguage::Bash,
                            context: ScriptContext::SetupScript,
                            working_dir: None,
                        },
                    ),
                    None,
                ),
                run_reason: ExecutionProcessRunReason::CodingAgent,
            },
            execution_process_id,
            &[],
        )
        .await
        .unwrap();

        let request = utils_core::approvals::ApprovalRequest::from_create(
            utils_core::approvals::CreateApprovalRequest {
                tool_name: "test_tool".to_string(),
                tool_input: json!({ "x": 1 }),
                tool_call_id: "call-1".to_string(),
            },
            execution_process_id,
        );
        let (approval, waiter) = deployment
            .approvals()
            .create_with_waiter(pool, request)
            .await
            .unwrap();

        let mut arguments = serde_json::Map::new();
        arguments.insert("approval_id".to_string(), json!(approval.id));
        arguments.insert(
            "execution_process_id".to_string(),
            json!(execution_process_id.to_string()),
        );
        arguments.insert("status".to_string(), json!("approved"));

        let result = client_running
            .call_tool(rmcp::model::CallToolRequestParams {
                meta: None,
                name: "respond_approval".into(),
                arguments: Some(arguments),
                task: None,
            })
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(false));

        let _ = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("approval waiter should resolve")
            .clone();

        let approval_uuid = Uuid::parse_str(&approval.id).unwrap();
        let persisted = approval_model::get_by_id(pool, approval_uuid)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            persisted.responded_by_client_id.as_deref(),
            Some("mcp:vk-tool-client@0.0.42")
        );

        let _ = client_running.cancel().await;
        let _ = server_running.cancel().await;
        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn create_task_idempotency_conflict_is_structured_tool_error() {
        let temp_root = std::env::temp_dir().join(format!("vk-mcp-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _guard = TestEnvGuard::new(&temp_root, "sqlite::memory:".to_string());

        let deployment = DeploymentImpl::new().await.unwrap();
        let pool = &deployment.db().pool;

        let project_id = Uuid::new_v4();
        Project::create(
            pool,
            &db::models::project::CreateProject {
                name: "Test project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let server = TaskServer::new(deployment);

        let request_id = Some("req-123".to_string());

        let first = server
            .create_task(Parameters(CreateTaskRequest {
                project_id,
                title: "Task A".to_string(),
                description: None,
                request_id: request_id.clone(),
            }))
            .await
            .unwrap();
        assert_eq!(first.is_error, Some(false));

        let second = server
            .create_task(Parameters(CreateTaskRequest {
                project_id,
                title: "Task B".to_string(),
                description: None,
                request_id: request_id.clone(),
            }))
            .await
            .unwrap();

        assert_eq!(second.is_error, Some(true));
        let structured = second
            .structured_content
            .clone()
            .expect("structured_content should be present");
        assert_eq!(
            structured["code"].as_str(),
            Some(MCP_CODE_IDEMPOTENCY_CONFLICT)
        );
        assert_eq!(structured["retryable"].as_bool(), Some(false));
        assert!(structured["hint"].as_str().is_some());
        assert_eq!(
            structured["details"]["request_id"].as_str(),
            Some("req-123")
        );

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn get_attempt_changes_guardrails_blocked_is_structured_tool_error() {
        let temp_root = std::env::temp_dir().join(format!("vk-mcp-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _guard = TestEnvGuard::new(&temp_root, "sqlite::memory:".to_string());

        let deployment = DeploymentImpl::new().await.unwrap();
        {
            let mut config = deployment.config().write().await;
            config.diff_preview_guard = DiffPreviewGuardPreset::Safe;
        }
        let pool = &deployment.db().pool;

        let repo_dir = temp_root.join("repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        assert!(
            Command::new("git")
                .arg("init")
                .current_dir(&repo_dir)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["config", "user.email", "vk-test@example.com"])
                .current_dir(&repo_dir)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["config", "user.name", "vk-test"])
                .current_dir(&repo_dir)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["checkout", "-b", "main"])
                .current_dir(&repo_dir)
                .status()
                .unwrap()
                .success()
        );
        std::fs::write(repo_dir.join("README.md"), "hello").unwrap();
        assert!(
            Command::new("git")
                .args(["add", "."])
                .current_dir(&repo_dir)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["commit", "-m", "init"])
                .current_dir(&repo_dir)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["checkout", "-b", "test-branch"])
                .current_dir(&repo_dir)
                .status()
                .unwrap()
                .success()
        );

        let workspace_root = temp_root.join("worktree_root");
        std::fs::create_dir_all(&workspace_root).unwrap();
        let worktree_dir = workspace_root.join("repo");
        assert!(
            Command::new("git")
                .args([
                    "clone",
                    repo_dir.to_str().unwrap(),
                    worktree_dir.to_str().unwrap()
                ])
                .current_dir(&workspace_root)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["checkout", "-B", "test-branch", "origin/test-branch"])
                .current_dir(&worktree_dir)
                .status()
                .unwrap()
                .success()
        );

        // Safe preset blocks at >200 changed files.
        for idx in 0..201usize {
            std::fs::write(worktree_dir.join(format!("file_{idx:04}.txt")), "x").unwrap();
        }

        let project_id = Uuid::new_v4();
        Project::create(
            pool,
            &db::models::project::CreateProject {
                name: "Test project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        Task::create(
            pool,
            &CreateTask::from_title_description(project_id, "Test task".to_string(), None),
            task_id,
        )
        .await
        .unwrap();

        let attempt_id = Uuid::new_v4();
        let workspace = Workspace::create(
            pool,
            &CreateWorkspace {
                branch: "test-branch".to_string(),
                agent_working_dir: None,
            },
            attempt_id,
            task_id,
        )
        .await
        .unwrap();

        let repo = Repo::find_or_create(pool, &repo_dir, "Repo").await.unwrap();
        WorkspaceRepo::create_many(
            pool,
            workspace.id,
            &[CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch: "main".to_string(),
            }],
        )
        .await
        .unwrap();
        Workspace::update_container_ref(pool, workspace.id, workspace_root.to_str().unwrap())
            .await
            .unwrap();

        let server = TaskServer::new(deployment);
        let result = server
            .get_attempt_changes(Parameters(GetAttemptChangesRequest {
                attempt_id: workspace.id,
                force: Some(false),
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
        let structured = result
            .structured_content
            .clone()
            .expect("structured_content should be present");
        assert_eq!(
            structured["code"].as_str(),
            Some(MCP_CODE_BLOCKED_GUARDRAILS)
        );
        assert!(structured["hint"].as_str().is_some());
        assert_eq!(
            structured["details"]["blocked_reason"].as_str(),
            Some("threshold_exceeded")
        );

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn approvals_elicitation_auto_approves_when_supported() {
        let temp_root = std::env::temp_dir().join(format!("vk-mcp-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _guard = TestEnvGuard::new(&temp_root, "sqlite::memory:".to_string());

        let deployment = DeploymentImpl::new().await.unwrap();
        let pool = &deployment.db().pool;

        let capabilities = rmcp::model::ClientCapabilities {
            elicitation: Some(rmcp::model::ElicitationCapability {
                form: Some(rmcp::model::FormElicitationCapability {
                    schema_validation: Some(true),
                }),
                url: None,
            }),
            ..Default::default()
        };

        let client_info = rmcp::model::Implementation {
            name: "vk-test-client".to_string(),
            title: None,
            version: "0.0.1".to_string(),
            description: None,
            icons: None,
            website_url: None,
        };

        let client = TestElicitationClient::new(
            rmcp::model::ClientInfo {
                meta: None,
                protocol_version: ProtocolVersion::V_2025_03_26,
                capabilities,
                client_info,
            },
            json!({ "decision": "approved", "denial_reason": null }),
        );

        let server = TaskServer::new(deployment.clone());

        let (server_io, client_io) = tokio::io::duplex(1024 * 1024);
        let (server_running, client_running) =
            tokio::join!(server.serve(server_io), client.clone().serve(client_io));
        let server_running = server_running.unwrap();
        let client_running = client_running.unwrap();

        assert!(
            server_running
                .supported_elicitation_modes()
                .contains(&rmcp::service::ElicitationMode::Form)
        );
        assert!(
            server_running
                .service()
                .approvals_elicitation_started
                .load(Ordering::SeqCst)
        );
        assert!(server_running.service().peer.read().unwrap().is_some());

        let project_id = Uuid::new_v4();
        Project::create(
            pool,
            &db::models::project::CreateProject {
                name: "Test project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        Task::create(
            pool,
            &CreateTask::from_title_description(project_id, "Test task".to_string(), None),
            task_id,
        )
        .await
        .unwrap();

        let attempt_id = Uuid::new_v4();
        Workspace::create(
            pool,
            &CreateWorkspace {
                branch: "main".to_string(),
                agent_working_dir: None,
            },
            attempt_id,
            task_id,
        )
        .await
        .unwrap();

        let session_id = Uuid::new_v4();
        Session::create(
            pool,
            &CreateSession {
                executor: Some("test".to_string()),
            },
            session_id,
            attempt_id,
        )
        .await
        .unwrap();

        let execution_process_id = Uuid::new_v4();
        ExecutionProcess::create(
            pool,
            &CreateExecutionProcess {
                session_id,
                executor_action: ExecutorAction {
                    typ: ExecutorActionType::ScriptRequest(
                        executors_protocol::actions::script::ScriptRequest {
                            script: "echo hi".to_string(),
                            language:
                                executors_protocol::actions::script::ScriptRequestLanguage::Bash,
                            context: ScriptContext::SetupScript,
                            working_dir: None,
                        },
                    ),
                    next_action: None,
                },
                run_reason: ExecutionProcessRunReason::CodingAgent,
            },
            execution_process_id,
            &[],
        )
        .await
        .unwrap();

        let request = utils_core::approvals::ApprovalRequest::from_create(
            utils_core::approvals::CreateApprovalRequest {
                tool_name: "test_tool".to_string(),
                tool_input: json!({ "x": 1 }),
                tool_call_id: "call-1".to_string(),
            },
            execution_process_id,
        );

        let mut created_rx = deployment.approvals().subscribe_created();
        let (approval, waiter) = deployment
            .approvals()
            .create_with_waiter(pool, request)
            .await
            .unwrap();
        let created = tokio::time::timeout(Duration::from_millis(200), created_rx.recv())
            .await
            .expect("approval should emit created event")
            .expect("created event receive should succeed");
        assert_eq!(created.id, approval.id);

        let status = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("approval waiter should resolve")
            .clone();
        assert!(matches!(
            status,
            utils_core::approvals::ApprovalStatus::Approved
        ));
        assert_eq!(client.call_count(), 1);

        let approval_uuid = Uuid::parse_str(&approval.id).unwrap();
        let persisted = approval_model::get_by_id(pool, approval_uuid)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            persisted.status,
            utils_core::approvals::ApprovalStatus::Approved
        ));
        assert_eq!(
            persisted.responded_by_client_id.as_deref(),
            Some("mcp:vk-test-client@0.0.1")
        );

        let _ = server_running.cancel().await;
        let _ = client_running.cancel().await;
        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn approvals_elicitation_auto_denies_when_supported() {
        let temp_root = std::env::temp_dir().join(format!("vk-mcp-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _guard = TestEnvGuard::new(&temp_root, "sqlite::memory:".to_string());

        let deployment = DeploymentImpl::new().await.unwrap();
        let pool = &deployment.db().pool;

        let client = TestElicitationClient::new(
            rmcp::model::ClientInfo {
                meta: None,
                protocol_version: ProtocolVersion::V_2025_03_26,
                capabilities: rmcp::model::ClientCapabilities {
                    elicitation: Some(rmcp::model::ElicitationCapability {
                        form: Some(rmcp::model::FormElicitationCapability {
                            schema_validation: Some(true),
                        }),
                        url: None,
                    }),
                    ..Default::default()
                },
                client_info: rmcp::model::Implementation {
                    name: "vk-test-client".to_string(),
                    title: None,
                    version: "0.0.2".to_string(),
                    description: None,
                    icons: None,
                    website_url: None,
                },
            },
            json!({ "decision": "denied", "denial_reason": "no" }),
        );

        let server = TaskServer::new(deployment.clone());

        let (server_io, client_io) = tokio::io::duplex(1024 * 1024);
        let (server_running, client_running) =
            tokio::join!(server.serve(server_io), client.clone().serve(client_io));
        let server_running = server_running.unwrap();
        let client_running = client_running.unwrap();

        assert!(
            server_running
                .supported_elicitation_modes()
                .contains(&rmcp::service::ElicitationMode::Form)
        );
        assert!(
            server_running
                .service()
                .approvals_elicitation_started
                .load(Ordering::SeqCst)
        );
        assert!(server_running.service().peer.read().unwrap().is_some());

        let project_id = Uuid::new_v4();
        Project::create(
            pool,
            &db::models::project::CreateProject {
                name: "Test project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        Task::create(
            pool,
            &CreateTask::from_title_description(project_id, "Test task".to_string(), None),
            task_id,
        )
        .await
        .unwrap();

        let attempt_id = Uuid::new_v4();
        Workspace::create(
            pool,
            &CreateWorkspace {
                branch: "main".to_string(),
                agent_working_dir: None,
            },
            attempt_id,
            task_id,
        )
        .await
        .unwrap();

        let session_id = Uuid::new_v4();
        Session::create(
            pool,
            &CreateSession {
                executor: Some("test".to_string()),
            },
            session_id,
            attempt_id,
        )
        .await
        .unwrap();

        let execution_process_id = Uuid::new_v4();
        ExecutionProcess::create(
            pool,
            &CreateExecutionProcess {
                session_id,
                executor_action: ExecutorAction {
                    typ: ExecutorActionType::ScriptRequest(
                        executors_protocol::actions::script::ScriptRequest {
                            script: "echo hi".to_string(),
                            language:
                                executors_protocol::actions::script::ScriptRequestLanguage::Bash,
                            context: ScriptContext::SetupScript,
                            working_dir: None,
                        },
                    ),
                    next_action: None,
                },
                run_reason: ExecutionProcessRunReason::CodingAgent,
            },
            execution_process_id,
            &[],
        )
        .await
        .unwrap();

        let request = utils_core::approvals::ApprovalRequest::from_create(
            utils_core::approvals::CreateApprovalRequest {
                tool_name: "test_tool".to_string(),
                tool_input: json!({ "x": 1 }),
                tool_call_id: "call-1".to_string(),
            },
            execution_process_id,
        );

        let mut created_rx = deployment.approvals().subscribe_created();
        let (approval, waiter) = deployment
            .approvals()
            .create_with_waiter(pool, request)
            .await
            .unwrap();
        let created = tokio::time::timeout(Duration::from_millis(200), created_rx.recv())
            .await
            .expect("approval should emit created event")
            .expect("created event receive should succeed");
        assert_eq!(created.id, approval.id);

        let status = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("approval waiter should resolve")
            .clone();
        match status {
            utils_core::approvals::ApprovalStatus::Denied { reason } => {
                assert_eq!(reason.as_deref(), Some("no"));
            }
            other => panic!("Expected denied approval, got: {:?}", other),
        }
        assert_eq!(client.call_count(), 1);

        let approval_uuid = Uuid::parse_str(&approval.id).unwrap();
        let persisted = approval_model::get_by_id(pool, approval_uuid)
            .await
            .unwrap()
            .unwrap();
        match persisted.status {
            utils_core::approvals::ApprovalStatus::Denied { reason } => {
                assert_eq!(reason.as_deref(), Some("no"));
            }
            other => panic!("Expected denied approval, got: {:?}", other),
        }
        assert_eq!(
            persisted.responded_by_client_id.as_deref(),
            Some("mcp:vk-test-client@0.0.2")
        );

        let _ = server_running.cancel().await;
        let _ = client_running.cancel().await;
        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn approvals_elicitation_is_skipped_when_client_does_not_declare_capability() {
        let temp_root = std::env::temp_dir().join(format!("vk-mcp-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let _guard = TestEnvGuard::new(&temp_root, "sqlite::memory:".to_string());

        let deployment = DeploymentImpl::new().await.unwrap();
        let pool = &deployment.db().pool;

        let client = TestElicitationClient::new(
            rmcp::model::ClientInfo {
                meta: None,
                protocol_version: ProtocolVersion::V_2025_03_26,
                capabilities: rmcp::model::ClientCapabilities::default(),
                client_info: rmcp::model::Implementation {
                    name: "vk-test-client".to_string(),
                    title: None,
                    version: "0.0.3".to_string(),
                    description: None,
                    icons: None,
                    website_url: None,
                },
            },
            json!({ "decision": "approved", "denial_reason": null }),
        );

        let server = TaskServer::new(deployment.clone());

        let (server_io, client_io) = tokio::io::duplex(1024 * 1024);
        let (server_running, client_running) =
            tokio::join!(server.serve(server_io), client.clone().serve(client_io));
        let server_running = server_running.unwrap();
        let client_running = client_running.unwrap();

        let project_id = Uuid::new_v4();
        Project::create(
            pool,
            &db::models::project::CreateProject {
                name: "Test project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        Task::create(
            pool,
            &CreateTask::from_title_description(project_id, "Test task".to_string(), None),
            task_id,
        )
        .await
        .unwrap();

        let attempt_id = Uuid::new_v4();
        Workspace::create(
            pool,
            &CreateWorkspace {
                branch: "main".to_string(),
                agent_working_dir: None,
            },
            attempt_id,
            task_id,
        )
        .await
        .unwrap();

        let session_id = Uuid::new_v4();
        Session::create(
            pool,
            &CreateSession {
                executor: Some("test".to_string()),
            },
            session_id,
            attempt_id,
        )
        .await
        .unwrap();

        let execution_process_id = Uuid::new_v4();
        ExecutionProcess::create(
            pool,
            &CreateExecutionProcess {
                session_id,
                executor_action: ExecutorAction {
                    typ: ExecutorActionType::ScriptRequest(
                        executors_protocol::actions::script::ScriptRequest {
                            script: "echo hi".to_string(),
                            language:
                                executors_protocol::actions::script::ScriptRequestLanguage::Bash,
                            context: ScriptContext::SetupScript,
                            working_dir: None,
                        },
                    ),
                    next_action: None,
                },
                run_reason: ExecutionProcessRunReason::CodingAgent,
            },
            execution_process_id,
            &[],
        )
        .await
        .unwrap();

        let request = utils_core::approvals::ApprovalRequest::from_create(
            utils_core::approvals::CreateApprovalRequest {
                tool_name: "test_tool".to_string(),
                tool_input: json!({ "x": 1 }),
                tool_call_id: "call-1".to_string(),
            },
            execution_process_id,
        );

        let (_approval, waiter) = deployment
            .approvals()
            .create_with_waiter(pool, request)
            .await
            .unwrap();

        let timed_out = tokio::time::timeout(Duration::from_millis(200), waiter).await;
        assert!(timed_out.is_err(), "approval should stay pending");
        assert_eq!(client.call_count(), 0);

        let _ = server_running.cancel().await;
        let _ = client_running.cancel().await;
        let _ = std::fs::remove_dir_all(&temp_root);
    }
}
