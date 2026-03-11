use chrono::{DateTime, Utc};
use db::{
    models::milestone::{MilestoneNodeBaseStrategy, MilestoneNodeKind, MilestoneNodeLayout},
    types::MilestoneAutomationMode,
};
use executors_protocol::ExecutorProfileId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

pub const MILESTONE_PLAN_SCHEMA_VERSION_V1: i32 = 1;

// Canonical fenced block label for agent outputs. The UI may detect this as a
// reliable plan carrier without having to guess.
pub const MILESTONE_PLAN_FENCE_INFO_V1: &str = "milestone-plan-v1";

/// A versioned, machine-parseable planning payload that can be validated,
/// previewed, and applied to a milestone.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MilestonePlanV1 {
    pub schema_version: i32,
    #[serde(default)]
    pub milestone: MilestonePlanMilestonePatchV1,
    pub nodes: Vec<MilestonePlanNodeV1>,
    #[serde(default)]
    pub edges: Vec<MilestonePlanEdgeV1>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct MilestonePlanMilestonePatchV1 {
    pub objective: Option<String>,
    pub definition_of_done: Option<String>,
    /// Mirrors `UpdateMilestone.default_executor_profile_id` semantics:
    /// - None: no change
    /// - Some(Some(id)): set
    /// - Some(None): clear
    pub default_executor_profile_id: Option<Option<ExecutorProfileId>>,
    pub automation_mode: Option<MilestoneAutomationMode>,
    pub baseline_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MilestonePlanCreateTaskV1 {
    pub title: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MilestonePlanNodeV1 {
    pub id: String,
    #[serde(default)]
    pub kind: MilestoneNodeKind,
    #[serde(default)]
    pub phase: i32,
    #[serde(default)]
    pub executor_profile_id: Option<ExecutorProfileId>,
    #[serde(default)]
    pub base_strategy: MilestoneNodeBaseStrategy,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub requires_approval: Option<bool>,
    #[serde(default)]
    pub layout: Option<MilestoneNodeLayout>,
    pub task_id: Option<Uuid>,
    pub create_task: Option<MilestonePlanCreateTaskV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MilestonePlanEdgeV1 {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub data_flow: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MilestonePlanMetadataField {
    Objective,
    DefinitionOfDone,
    DefaultExecutorProfile,
    AutomationMode,
    BaselineRef,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct MilestonePlanPreviewMetadataChange {
    pub field: MilestonePlanMetadataField,
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct MilestonePlanPreviewTaskToCreate {
    pub node_id: String,
    pub title: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct MilestonePlanPreviewTaskLink {
    pub node_id: String,
    pub task_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Hash)]
pub struct MilestonePlanEdgeKeyV1 {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub data_flow: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct MilestonePlanPreviewNodeDiff {
    pub existing: Vec<String>,
    pub planned: Vec<String>,
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct MilestonePlanPreviewEdgeDiff {
    pub existing: Vec<MilestonePlanEdgeKeyV1>,
    pub planned: Vec<MilestonePlanEdgeKeyV1>,
    pub added: Vec<MilestonePlanEdgeKeyV1>,
    pub removed: Vec<MilestonePlanEdgeKeyV1>,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct MilestonePlanPreviewResponse {
    pub metadata_changes: Vec<MilestonePlanPreviewMetadataChange>,
    pub tasks_to_create: Vec<MilestonePlanPreviewTaskToCreate>,
    pub task_links: Vec<MilestonePlanPreviewTaskLink>,
    pub node_diff: MilestonePlanPreviewNodeDiff,
    pub edge_diff: MilestonePlanPreviewEdgeDiff,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MilestonePlanApplyResponse {
    pub milestone: db::models::milestone::Milestone,
    pub created_tasks: Vec<db::models::task::Task>,
    pub applied_at: DateTime<Utc>,
}

pub const MILESTONE_PLANNING_PROMPT_TEMPLATE: &str = r#"You are a Milestone Planning Guide.

Your goal is to turn the milestone objective and definition-of-done into an executable milestone graph (nodes + edges) that VK can preview and apply.

Rules:
- You MUST emit exactly one canonical plan payload (MilestonePlanV1) in a fenced JSON block.
- Use this exact fence info string: milestone-plan-v1
- The JSON MUST validate as MilestonePlanV1 with schema_version = 1.
- The JSON MUST be strict JSON (no comments, no trailing commas).
- Node ids must be unique.
- Edges must form a DAG (no cycles, no self-edges).
- Prefer small nodes with crisp titles, and add at least one checkpoint for a human review gate.

Output format (required):

```milestone-plan-v1
<JSON>
```

Schema (MilestonePlanV1, schema_version = 1):

{
  "schema_version": 1,
  "milestone": {
    "objective": string|null,
    "definition_of_done": string|null,
    "default_executor_profile_id": { "executor": "...", "variant": "..."|null } | null,
    "automation_mode": "manual" | "auto" | null,
    "baseline_ref": string | null
  },
  "nodes": [
    {
      "id": string,
      "kind": "task" | "checkpoint",
      "phase": number,
      "executor_profile_id": { "executor": "...", "variant": "..."|null } | null,
      "base_strategy": "topology" | "baseline",
      "instructions": string | null,
      "requires_approval": boolean | null,
      "layout": { "x": number, "y": number } | null,
      "task_id": string | null,
      "create_task": { "title": string, "description": string|null } | null
    }
  ],
  "edges": [
    { "from": string, "to": string, "data_flow": string|null }
  ]
}

Additional constraints:
- You MUST set milestone.objective and milestone.definition_of_done to non-empty strings.
- Each node MUST either reference an existing task (task_id) OR create a new task (create_task). Prefer create_task for planning.
- For checkpoint nodes: kind="checkpoint" and requires_approval=true.
- Keep the plan practical: 3-8 nodes total.

After the fenced plan block, you may optionally add up to 5 lines of explanation."#;
