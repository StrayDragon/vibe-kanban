use std::path::{Path, PathBuf};

use codex_app_server_protocol::{DynamicToolCallOutputContentItem, DynamicToolSpec};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

// Codex tool names must match `^[a-zA-Z0-9_-]+$` (no dots).
pub const VK_TOOL_GET_ATTEMPT_STATUS: &str = "vk_get_attempt_status";
pub const VK_TOOL_TAIL_ATTEMPT_LOGS: &str = "vk_tail_attempt_logs";
pub const VK_TOOL_GET_ATTEMPT_CHANGES: &str = "vk_get_attempt_changes";
#[cfg(test)]
pub const VK_TOOL_TEST_MUTATING: &str = "vk__test_mutating";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VkDynamicToolKind {
    ReadOnly,
    Mutating,
}

#[derive(Debug, Clone)]
pub struct VkDynamicToolContext {
    pub workspace_root: PathBuf,
    pub project_name: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub attempt_id: Option<String>,
    pub workspace_branch: Option<String>,
}

impl VkDynamicToolContext {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            project_name: None,
            project_id: None,
            task_id: None,
            attempt_id: None,
            workspace_branch: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VkDynamicToolDefinition {
    pub spec: DynamicToolSpec,
    pub kind: VkDynamicToolKind,
}

#[derive(Debug, Clone, Default)]
pub struct VkDynamicToolRegistry {
    tools: Vec<VkDynamicToolDefinition>,
}

impl VkDynamicToolRegistry {
    pub fn vk_default() -> Self {
        #[allow(unused_mut)]
        let mut tools = vec![
            VkDynamicToolDefinition {
                spec: DynamicToolSpec {
                    name: VK_TOOL_GET_ATTEMPT_STATUS.to_string(),
                    description: "Get a human-readable summary of the current VK attempt."
                        .to_string(),
                    input_schema: schema_attempt_id_only(),
                    defer_loading: false,
                },
                kind: VkDynamicToolKind::ReadOnly,
            },
            VkDynamicToolDefinition {
                spec: DynamicToolSpec {
                    name: VK_TOOL_TAIL_ATTEMPT_LOGS.to_string(),
                    description: "Tail recent executor protocol logs for the current VK attempt."
                        .to_string(),
                    input_schema: schema_tail_attempt_logs(),
                    defer_loading: false,
                },
                kind: VkDynamicToolKind::ReadOnly,
            },
            VkDynamicToolDefinition {
                spec: DynamicToolSpec {
                    name: VK_TOOL_GET_ATTEMPT_CHANGES.to_string(),
                    description: "Summarize git changes in the current VK attempt workspace."
                        .to_string(),
                    input_schema: schema_get_attempt_changes(),
                    defer_loading: false,
                },
                kind: VkDynamicToolKind::ReadOnly,
            },
        ];

        #[cfg(test)]
        tools.push(VkDynamicToolDefinition {
            spec: DynamicToolSpec {
                name: VK_TOOL_TEST_MUTATING.to_string(),
                description: "Test-only mutating tool (requires approval).".to_string(),
                input_schema: schema_attempt_id_only(),
                defer_loading: false,
            },
            kind: VkDynamicToolKind::Mutating,
        });

        Self { tools }
    }

    pub fn specs(&self) -> Vec<DynamicToolSpec> {
        let mut specs = self
            .tools
            .iter()
            .map(|t| t.spec.clone())
            .collect::<Vec<_>>();
        specs.sort_by(|a, b| a.name.cmp(&b.name));
        specs
    }

    pub fn kind(&self, name: &str) -> Option<VkDynamicToolKind> {
        self.tools
            .iter()
            .find(|t| t.spec.name == name)
            .map(|t| t.kind)
    }

    pub fn is_supported(&self, name: &str) -> bool {
        self.kind(name).is_some()
    }

    pub fn summarize_args(&self, name: &str, arguments: &Value) -> Option<String> {
        match name {
            VK_TOOL_GET_ATTEMPT_STATUS => {
                serde_json::from_value::<AttemptIdOnlyArgs>(arguments.clone())
                    .ok()
                    .map(|args| {
                        format!(
                            "attempt_id={}",
                            args.attempt_id.unwrap_or("<current>".to_string())
                        )
                    })
            }
            VK_TOOL_TAIL_ATTEMPT_LOGS => {
                serde_json::from_value::<TailAttemptLogsArgs>(arguments.clone())
                    .ok()
                    .map(|args| {
                        format!(
                            "attempt_id={} max_lines={}",
                            args.attempt_id.unwrap_or("<current>".to_string()),
                            args.max_lines.unwrap_or(default_max_lines())
                        )
                    })
            }
            VK_TOOL_GET_ATTEMPT_CHANGES => {
                serde_json::from_value::<GetAttemptChangesArgs>(arguments.clone())
                    .ok()
                    .map(|args| {
                        format!(
                            "attempt_id={} max_files={}",
                            args.attempt_id.unwrap_or("<current>".to_string()),
                            args.max_files.unwrap_or(default_max_files())
                        )
                    })
            }
            _ => None,
        }
    }

    pub async fn execute(
        &self,
        ctx: &VkDynamicToolContext,
        name: &str,
        arguments: Value,
        recent_logs: Option<Vec<String>>,
    ) -> Result<Vec<DynamicToolCallOutputContentItem>, String> {
        match name {
            VK_TOOL_GET_ATTEMPT_STATUS => {
                let args: AttemptIdOnlyArgs = parse_args(name, arguments)?;
                let attempt_id = resolve_attempt_id(ctx, args.attempt_id)?;
                ensure_attempt_is_current(ctx, &attempt_id)?;
                get_attempt_status(ctx, &attempt_id).await
            }
            VK_TOOL_TAIL_ATTEMPT_LOGS => {
                let args: TailAttemptLogsArgs = parse_args(name, arguments)?;
                let attempt_id = resolve_attempt_id(ctx, args.attempt_id.clone())?;
                ensure_attempt_is_current(ctx, &attempt_id)?;
                tail_attempt_logs(ctx, &args, recent_logs.unwrap_or_default()).await
            }
            VK_TOOL_GET_ATTEMPT_CHANGES => {
                let args: GetAttemptChangesArgs = parse_args(name, arguments)?;
                let attempt_id = resolve_attempt_id(ctx, args.attempt_id.clone())?;
                ensure_attempt_is_current(ctx, &attempt_id)?;
                get_attempt_changes(ctx, &args).await
            }
            #[cfg(test)]
            VK_TOOL_TEST_MUTATING => {
                let args: AttemptIdOnlyArgs = parse_args(name, arguments)?;
                let attempt_id = resolve_attempt_id(ctx, args.attempt_id)?;
                ensure_attempt_is_current(ctx, &attempt_id)?;
                Ok(vec![DynamicToolCallOutputContentItem::InputText {
                    text: "ok".to_string(),
                }])
            }
            _ => Err(format!("Unsupported tool: {name}")),
        }
    }
}

fn default_max_lines() -> u32 {
    200
}

fn default_max_files() -> u32 {
    50
}

fn schema_attempt_id_only() -> Value {
    json!({
        "type": "object",
        "properties": {
            "attempt_id": {
                "type": "string",
                "description": "VK attempt/workspace id (UUID). If omitted, VK will use the current attempt."
            }
        },
        "additionalProperties": false,
    })
}

fn schema_tail_attempt_logs() -> Value {
    json!({
        "type": "object",
        "properties": {
            "attempt_id": {
                "type": "string",
                "description": "VK attempt/workspace id (UUID). If omitted, VK will use the current attempt."
            },
            "max_lines": {
                "type": "integer",
                "minimum": 1,
                "maximum": 500,
                "description": "Maximum number of recent log lines to return."
            }
        },
        "additionalProperties": false,
    })
}

fn schema_get_attempt_changes() -> Value {
    json!({
        "type": "object",
        "properties": {
            "attempt_id": {
                "type": "string",
                "description": "VK attempt/workspace id (UUID). If omitted, VK will use the current attempt."
            },
            "max_files": {
                "type": "integer",
                "minimum": 1,
                "maximum": 500,
                "description": "Maximum number of file entries to include."
            }
        },
        "additionalProperties": false,
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AttemptIdOnlyArgs {
    attempt_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TailAttemptLogsArgs {
    attempt_id: Option<String>,
    #[serde(default)]
    max_lines: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GetAttemptChangesArgs {
    attempt_id: Option<String>,
    #[serde(default)]
    max_files: Option<u32>,
}

fn parse_args<T: for<'de> Deserialize<'de>>(tool: &str, arguments: Value) -> Result<T, String> {
    serde_json::from_value(arguments).map_err(|err| {
        format!("Invalid arguments for `{tool}`: {err}. Inputs must match the tool's JSON schema.")
    })
}

fn resolve_attempt_id(
    ctx: &VkDynamicToolContext,
    attempt_id: Option<String>,
) -> Result<String, String> {
    attempt_id
        .or_else(|| ctx.attempt_id.clone())
        .ok_or_else(|| {
            "attempt_id is required (or set VK_WORKSPACE_ID in the environment).".to_string()
        })
}

fn ensure_attempt_is_current(ctx: &VkDynamicToolContext, attempt_id: &str) -> Result<(), String> {
    let parsed =
        Uuid::parse_str(attempt_id).map_err(|_| "attempt_id must be a UUID string".to_string())?;

    if let Some(current) = ctx.attempt_id.as_deref() {
        let current_parsed = Uuid::parse_str(current)
            .map_err(|_| "VK_WORKSPACE_ID must be a UUID string".to_string())?;
        if current_parsed != parsed {
            return Err(format!(
                "This tool can only access the current attempt. Current attempt_id: {current}. Requested: {attempt_id}."
            ));
        }
    }
    Ok(())
}

async fn get_attempt_status(
    ctx: &VkDynamicToolContext,
    attempt_id: &str,
) -> Result<Vec<DynamicToolCallOutputContentItem>, String> {
    let mut lines = Vec::new();
    lines.push(format!("attempt_id: {attempt_id}"));
    if let Some(task_id) = ctx.task_id.as_deref() {
        lines.push(format!("task_id: {task_id}"));
    }
    if let Some(project_id) = ctx.project_id.as_deref() {
        lines.push(format!("project_id: {project_id}"));
    }
    if let Some(project_name) = ctx.project_name.as_deref() {
        lines.push(format!("project_name: {project_name}"));
    }
    if let Some(branch) = ctx.workspace_branch.as_deref() {
        lines.push(format!("workspace_branch: {branch}"));
    }
    lines.push(format!("workspace_root: {}", ctx.workspace_root.display()));

    if let Ok(branch) = run_cmd(
        &ctx.workspace_root,
        "git",
        &["rev-parse", "--abbrev-ref", "HEAD"],
    )
    .await
    {
        let branch = branch.trim();
        if !branch.is_empty() {
            lines.push(format!("git_branch: {branch}"));
        }
    }

    if let Ok(status) = run_cmd(&ctx.workspace_root, "git", &["status", "--porcelain=v1"]).await {
        let entries = status.lines().filter(|l| !l.trim().is_empty()).count();
        lines.push(format!("git_status_entries: {entries}"));
    }

    Ok(vec![DynamicToolCallOutputContentItem::InputText {
        text: lines.join("\n"),
    }])
}

async fn tail_attempt_logs(
    _ctx: &VkDynamicToolContext,
    args: &TailAttemptLogsArgs,
    recent_logs: Vec<String>,
) -> Result<Vec<DynamicToolCallOutputContentItem>, String> {
    let max_lines = args.max_lines.unwrap_or(default_max_lines()).clamp(1, 500) as usize;

    let start = recent_logs.len().saturating_sub(max_lines);
    let slice = &recent_logs[start..];
    let text = if slice.is_empty() {
        "No recent logs captured yet.".to_string()
    } else {
        slice.join("\n")
    };

    Ok(vec![DynamicToolCallOutputContentItem::InputText { text }])
}

async fn get_attempt_changes(
    ctx: &VkDynamicToolContext,
    args: &GetAttemptChangesArgs,
) -> Result<Vec<DynamicToolCallOutputContentItem>, String> {
    let max_files = args.max_files.unwrap_or(default_max_files()).clamp(1, 500) as usize;

    let status = run_cmd(&ctx.workspace_root, "git", &["status", "--porcelain=v1"])
        .await
        .map_err(|err| format!("Failed to run git status: {err}"))?;

    let mut entries = status
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.to_string())
        .collect::<Vec<_>>();
    entries.sort();

    let total = entries.len();
    let listed = entries.iter().take(max_files).cloned().collect::<Vec<_>>();

    let diff_stat = run_cmd(&ctx.workspace_root, "git", &["diff", "--stat"])
        .await
        .ok();

    let mut text = String::new();
    text.push_str(&format!(
        "workspace_root: {}\nchanged_entries: {total}\n",
        ctx.workspace_root.display()
    ));
    if let Some(stat) = diff_stat {
        let stat = stat.trim();
        if !stat.is_empty() {
            text.push_str("\n# git diff --stat\n");
            text.push_str(stat);
            text.push('\n');
        }
    }
    if !listed.is_empty() {
        text.push_str("\n# git status --porcelain=v1\n");
        for line in listed {
            text.push_str(&line);
            text.push('\n');
        }
        if total > max_files {
            text.push_str(&format!("… and {} more\n", total - max_files));
        }
    }

    Ok(vec![DynamicToolCallOutputContentItem::InputText { text }])
}

async fn run_cmd(dir: &Path, program: &str, args: &[&str]) -> Result<String, String> {
    let output = tokio::process::Command::new(program)
        .current_dir(dir)
        .args(args)
        .output()
        .await
        .map_err(|err| err.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "{program} {:?} failed (exit={:?}): stdout={stdout} stderr={stderr}",
            args,
            output.status.code()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    #[tokio::test]
    async fn registry_rejects_unknown_tool() {
        let reg = VkDynamicToolRegistry::vk_default();
        let ctx = VkDynamicToolContext::new(std::env::temp_dir());

        let err = reg
            .execute(&ctx, "vk.unknown", json!({}), None)
            .await
            .expect_err("unknown tool");
        assert!(err.contains("Unsupported tool"));
    }

    #[tokio::test]
    async fn registry_rejects_invalid_args_shape() {
        let reg = VkDynamicToolRegistry::vk_default();
        let mut ctx = VkDynamicToolContext::new(std::env::temp_dir());
        ctx.attempt_id = Some("3b3a4b8a-3b55-4f8b-9af6-0fef7f7c0b4b".to_string());

        let err = reg
            .execute(
                &ctx,
                VK_TOOL_GET_ATTEMPT_STATUS,
                json!({"attempt_id": 123}),
                None,
            )
            .await
            .expect_err("invalid args");
        assert!(err.contains("Invalid arguments"));
    }

    #[tokio::test]
    async fn registry_rejects_unknown_fields() {
        let reg = VkDynamicToolRegistry::vk_default();
        let mut ctx = VkDynamicToolContext::new(std::env::temp_dir());
        ctx.attempt_id = Some("3b3a4b8a-3b55-4f8b-9af6-0fef7f7c0b4b".to_string());

        let err = reg
            .execute(
                &ctx,
                VK_TOOL_GET_ATTEMPT_STATUS,
                json!({"attempt_id": ctx.attempt_id.clone().unwrap(), "extra": true}),
                None,
            )
            .await
            .expect_err("unknown fields");
        assert!(err.contains("Invalid arguments"));
    }

    #[tokio::test]
    async fn get_attempt_status_succeeds_with_current_attempt() {
        let reg = VkDynamicToolRegistry::vk_default();
        let mut ctx = VkDynamicToolContext::new(std::env::temp_dir());
        ctx.attempt_id = Some("3b3a4b8a-3b55-4f8b-9af6-0fef7f7c0b4b".to_string());

        let items = reg
            .execute(&ctx, VK_TOOL_GET_ATTEMPT_STATUS, json!({}), None)
            .await
            .expect("execute");

        assert!(matches!(
            items.first(),
            Some(DynamicToolCallOutputContentItem::InputText { text })
                if text.contains("attempt_id:")
        ));
    }

    #[tokio::test]
    async fn tail_attempt_logs_succeeds_and_returns_text() {
        let reg = VkDynamicToolRegistry::vk_default();
        let mut ctx = VkDynamicToolContext::new(std::env::temp_dir());
        ctx.attempt_id = Some("3b3a4b8a-3b55-4f8b-9af6-0fef7f7c0b4b".to_string());

        let items = reg
            .execute(
                &ctx,
                VK_TOOL_TAIL_ATTEMPT_LOGS,
                json!({ "max_lines": 2 }),
                Some(vec![
                    "line1".to_string(),
                    "line2".to_string(),
                    "line3".to_string(),
                ]),
            )
            .await
            .expect("execute");

        assert!(matches!(
            items.first(),
            Some(DynamicToolCallOutputContentItem::InputText { text })
                if text.contains("line2") && text.contains("line3")
        ));
    }

    #[tokio::test]
    async fn get_attempt_changes_succeeds_in_git_repo() {
        let reg = VkDynamicToolRegistry::vk_default();
        let dir = tempfile::tempdir().expect("tempdir");
        Command::new("git")
            .current_dir(dir.path())
            .args(["init", "-q"])
            .status()
            .expect("git init");
        std::fs::write(dir.path().join("file.txt"), "hi").expect("write file");

        let mut ctx = VkDynamicToolContext::new(dir.path().to_path_buf());
        ctx.attempt_id = Some("3b3a4b8a-3b55-4f8b-9af6-0fef7f7c0b4b".to_string());

        let items = reg
            .execute(
                &ctx,
                VK_TOOL_GET_ATTEMPT_CHANGES,
                json!({ "max_files": 10 }),
                None,
            )
            .await
            .expect("execute");

        assert!(matches!(
            items.first(),
            Some(DynamicToolCallOutputContentItem::InputText { text })
                if text.contains("changed_entries: 1")
        ));
    }

    #[tokio::test]
    async fn registry_requires_attempt_id_when_not_in_vk_attempt() {
        let reg = VkDynamicToolRegistry::vk_default();
        let ctx = VkDynamicToolContext::new(std::env::temp_dir());

        let err = reg
            .execute(&ctx, VK_TOOL_GET_ATTEMPT_STATUS, json!({}), None)
            .await
            .expect_err("missing attempt_id");

        assert!(err.contains("attempt_id is required"));
    }
}
