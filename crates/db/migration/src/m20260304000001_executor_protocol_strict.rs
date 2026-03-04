use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();

        migrate_execution_processes(conn).await?;
        migrate_task_groups(conn).await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

#[derive(Iden)]
enum ExecutionProcesses {
    Table,
    Id,
    ExecutorAction,
}

#[derive(Iden)]
enum TaskGroups {
    Table,
    Id,
    GraphJson,
}

fn canonical_executor_name(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("executor must not be empty".to_string());
    }

    let mut norm = trimmed.replace('-', "_").to_ascii_uppercase();
    if norm == "CURSOR" {
        norm = "CURSOR_AGENT".to_string();
    }

    const ALLOWED: [&str; 10] = [
        "CLAUDE_CODE",
        "AMP",
        "GEMINI",
        "CODEX",
        "FAKE_AGENT",
        "OPENCODE",
        "CURSOR_AGENT",
        "QWEN_CODE",
        "COPILOT",
        "DROID",
    ];
    if !ALLOWED.contains(&norm.as_str()) {
        return Err(format!("unknown executor '{raw}' (normalized to '{norm}')"));
    }

    Ok(norm)
}

fn canonical_variant(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("DEFAULT") {
        return "DEFAULT".to_string();
    }
    trimmed.replace('-', "_").to_ascii_uppercase()
}

fn migrate_executor_profile_id(
    obj: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<bool, String> {
    let mut changed = false;

    if obj.contains_key("profile")
        && !obj.contains_key("executor")
        && let Some(value) = obj.remove("profile")
    {
        obj.insert("executor".to_string(), value);
        changed = true;
    }

    if let Some(executor) = obj.get_mut("executor") {
        match executor {
            serde_json::Value::String(s) => {
                let canon = canonical_executor_name(s)?;
                if *s != canon {
                    *s = canon;
                    changed = true;
                }
            }
            serde_json::Value::Null => {}
            _ => return Err("executor_profile_id.executor must be a string".to_string()),
        }
    }

    if let Some(variant) = obj.get_mut("variant")
        && let serde_json::Value::String(s) = variant
    {
        let canon = canonical_variant(s);
        if *s != canon {
            *s = canon;
            changed = true;
        }
    }

    Ok(changed)
}

fn migrate_executor_action_type(
    obj: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<bool, String> {
    let mut changed = false;

    if obj.contains_key("profile_variant_label")
        && !obj.contains_key("executor_profile_id")
        && let Some(value) = obj.remove("profile_variant_label")
    {
        obj.insert("executor_profile_id".to_string(), value);
        changed = true;
    }

    if let Some(profile_id) = obj.get_mut("executor_profile_id") {
        match profile_id {
            serde_json::Value::Object(profile_obj) => {
                if migrate_executor_profile_id(profile_obj)? {
                    changed = true;
                }
            }
            serde_json::Value::Null => {}
            _ => return Err("executor_profile_id must be an object".to_string()),
        }
    }

    Ok(changed)
}

fn migrate_executor_action(value: &mut serde_json::Value) -> Result<bool, String> {
    let mut changed = false;
    let obj = value
        .as_object_mut()
        .ok_or_else(|| "executor_action must be a JSON object".to_string())?;

    let typ = obj
        .get_mut("typ")
        .ok_or_else(|| "executor_action.typ is missing".to_string())?;
    let typ_obj = typ
        .as_object_mut()
        .ok_or_else(|| "executor_action.typ must be an object".to_string())?;
    if migrate_executor_action_type(typ_obj)? {
        changed = true;
    }

    if let Some(next_action) = obj.get_mut("next_action") {
        if serde_json::Value::is_null(next_action) {
            // nothing
        } else if migrate_executor_action(next_action)? {
            changed = true;
        }
    }

    Ok(changed)
}

fn migrate_task_group_graph(value: &mut serde_json::Value) -> Result<bool, String> {
    let mut changed = false;
    let obj = value
        .as_object_mut()
        .ok_or_else(|| "task_groups.graph_json must be a JSON object".to_string())?;

    let Some(nodes) = obj.get_mut("nodes").and_then(|n| n.as_array_mut()) else {
        return Ok(false);
    };

    for node in nodes {
        let Some(node_obj) = node.as_object_mut() else {
            continue;
        };
        let Some(executor_profile_id) = node_obj.get_mut("executor_profile_id") else {
            continue;
        };
        match executor_profile_id {
            serde_json::Value::Object(profile_obj) => {
                if migrate_executor_profile_id(profile_obj)? {
                    changed = true;
                }
            }
            serde_json::Value::Null => {}
            _ => return Err("executor_profile_id must be an object".to_string()),
        }
    }

    Ok(changed)
}

async fn migrate_execution_processes<C: ConnectionTrait>(db: &C) -> Result<(), DbErr> {
    let select = Query::select()
        .column(ExecutionProcesses::Id)
        .column(ExecutionProcesses::ExecutorAction)
        .from(ExecutionProcesses::Table)
        .to_owned();

    let rows = db.query_all(&select).await?;

    for row in rows {
        let id: i64 = row.try_get("", "id")?;
        let mut executor_action_value: serde_json::Value = match row.try_get("", "executor_action")
        {
            Ok(v) => v,
            Err(_) => {
                let s: String = row.try_get("", "executor_action")?;
                serde_json::from_str(&s).map_err(|err| {
                    DbErr::Custom(format!(
                        "Invalid execution_processes.executor_action JSON (id={id}): {err}"
                    ))
                })?
            }
        };

        let changed = migrate_executor_action(&mut executor_action_value).map_err(DbErr::Custom)?;

        // Validate against the strict protocol type after migration.
        serde_json::from_value::<executors_protocol::actions::ExecutorAction>(
            executor_action_value.clone(),
        )
        .map_err(|err| {
            DbErr::Custom(format!(
                "execution_processes.executor_action is invalid after migration (id={id}): {err}"
            ))
        })?;

        if changed {
            let update = Query::update()
                .table(ExecutionProcesses::Table)
                .values([(
                    ExecutionProcesses::ExecutorAction,
                    Expr::val(executor_action_value),
                )])
                .and_where(Expr::col(ExecutionProcesses::Id).eq(id))
                .to_owned();
            db.execute(&update).await?;
        }
    }

    Ok(())
}

async fn migrate_task_groups<C: ConnectionTrait>(db: &C) -> Result<(), DbErr> {
    let select = Query::select()
        .column(TaskGroups::Id)
        .column(TaskGroups::GraphJson)
        .from(TaskGroups::Table)
        .to_owned();

    let rows = db.query_all(&select).await?;

    for row in rows {
        let id: i64 = row.try_get("", "id")?;
        let mut graph_value: serde_json::Value = match row.try_get("", "graph_json") {
            Ok(v) => v,
            Err(_) => {
                let s: String = row.try_get("", "graph_json")?;
                serde_json::from_str(&s).map_err(|err| {
                    DbErr::Custom(format!(
                        "Invalid task_groups.graph_json JSON (id={id}): {err}"
                    ))
                })?
            }
        };

        let changed = migrate_task_group_graph(&mut graph_value).map_err(DbErr::Custom)?;

        if changed {
            let update = Query::update()
                .table(TaskGroups::Table)
                .values([(TaskGroups::GraphJson, Expr::val(graph_value))])
                .and_where(Expr::col(TaskGroups::Id).eq(id))
                .to_owned();
            db.execute(&update).await?;
        }
    }

    Ok(())
}
