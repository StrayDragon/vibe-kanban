use chrono::{DateTime, Utc};
use db::{
    models::milestone::{MilestoneNodeBaseStrategy, MilestoneNodeKind, MilestoneNodeLayout},
    types::MilestoneAutomationMode,
};
use executors_protocol::ExecutorProfileId;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashSet;
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MilestonePlanExtractionKind {
    Fenced,
    Embedded,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MilestonePlanDetectionStatus {
    Found,
    NotFound,
    Invalid,
    Unsupported,
}

/// Structured result for detecting the latest milestone planning payload from a guide output.
///
/// This is intentionally narrow and read-only so clients can present actionable guidance:
/// - not_found: ask the guide to emit the fenced plan block
/// - invalid: ask the guide to re-emit valid JSON
/// - unsupported: ask the guide to emit schema_version=1
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MilestonePlanDetectionResult {
    pub status: MilestonePlanDetectionStatus,
    pub plan: Option<MilestonePlanV1>,
    pub extracted_from: Option<MilestonePlanExtractionKind>,
    pub source_turn_id: Option<Uuid>,
    #[ts(type = "number | null")]
    pub source_entry_index: Option<i64>,
    pub error: Option<String>,
}

fn looks_like_milestone_plan_json(candidate: &str) -> bool {
    // Keep this lightweight and conservative to avoid flagging arbitrary JSON as "invalid plan".
    let s = candidate;
    s.contains("\"schema_version\"") && s.contains("\"nodes\"")
}

fn extract_last_fenced_json(input: &str) -> Option<String> {
    // Prefer the canonical fence info string. Keep a small compatibility surface for older outputs.
    // NOTE: We intentionally do not match generic ```json fences to reduce false positives.
    let patterns = [
        format!(r"(?is)```{}\s*([\s\S]*?)```", regex::escape(MILESTONE_PLAN_FENCE_INFO_V1)),
        r"(?is)```milestone-plan\s*([\s\S]*?)```".to_string(),
    ];

    for pattern in patterns {
        let re = Regex::new(&pattern).ok()?;
        let mut last: Option<String> = None;
        for caps in re.captures_iter(input) {
            if let Some(m) = caps.get(1) {
                let candidate = m.as_str().trim();
                if !candidate.is_empty() {
                    last = Some(candidate.to_string());
                }
            }
        }
        if last.is_some() {
            return last;
        }
    }

    None
}

fn extract_embedded_json_object(input: &str) -> Option<String> {
    let first_brace = input.find('{')?;
    let text = &input[first_brace..];

    let mut brace_positions: Vec<usize> = Vec::new();
    for (idx, ch) in text.char_indices() {
        if ch == '{' {
            brace_positions.push(first_brace + idx);
        }
    }

    for start in brace_positions {
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escape_next = false;

        for (idx, ch) in input[start..].char_indices() {
            let abs = start + idx;

            if in_string {
                if escape_next {
                    escape_next = false;
                    continue;
                }
                match ch {
                    '\\' => {
                        escape_next = true;
                    }
                    '"' => {
                        in_string = false;
                    }
                    _ => {}
                }
                continue;
            }

            match ch {
                '"' => {
                    in_string = true;
                }
                '{' => {
                    depth += 1;
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let candidate = input[start..=abs].trim();
                        return if candidate.is_empty() {
                            None
                        } else {
                            Some(candidate.to_string())
                        };
                    }
                }
                _ => {}
            }
        }
    }

    None
}

pub fn detect_milestone_plan_v1_in_text(input: &str) -> MilestonePlanDetectionResult {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return MilestonePlanDetectionResult {
            status: MilestonePlanDetectionStatus::NotFound,
            plan: None,
            extracted_from: None,
            source_turn_id: None,
            source_entry_index: None,
            error: None,
        };
    }

    let mut candidates: Vec<(String, Option<MilestonePlanExtractionKind>)> = Vec::new();

    if (trimmed.starts_with('{') || trimmed.starts_with('['))
        && looks_like_milestone_plan_json(trimmed)
    {
        candidates.push((trimmed.to_string(), None));
    }

    if let Some(fenced) = extract_last_fenced_json(trimmed) {
        candidates.push((fenced, Some(MilestonePlanExtractionKind::Fenced)));
    }

    if let Some(embedded) = extract_embedded_json_object(trimmed)
        && looks_like_milestone_plan_json(&embedded)
    {
        candidates.push((embedded, Some(MilestonePlanExtractionKind::Embedded)));
    }

    if candidates.is_empty() {
        return MilestonePlanDetectionResult {
            status: MilestonePlanDetectionStatus::NotFound,
            plan: None,
            extracted_from: None,
            source_turn_id: None,
            source_entry_index: None,
            error: None,
        };
    }

    let mut seen = HashSet::<String>::new();
    let mut last_error: Option<String> = None;
    let mut last_unsupported: Option<(Option<MilestonePlanExtractionKind>, i32)> = None;

    for (json, extracted_from) in candidates {
        if !seen.insert(json.clone()) {
            continue;
        }

        let parsed_value: JsonValue = match serde_json::from_str(&json) {
            Ok(v) => v,
            Err(err) => {
                last_error = Some(format!("Invalid JSON: {}", err));
                continue;
            }
        };

        let plan: MilestonePlanV1 = match serde_json::from_value(parsed_value) {
            Ok(p) => p,
            Err(err) => {
                last_error = Some(err.to_string());
                continue;
            }
        };

        if plan.schema_version != MILESTONE_PLAN_SCHEMA_VERSION_V1 {
            last_unsupported = Some((extracted_from, plan.schema_version));
            continue;
        }

        return MilestonePlanDetectionResult {
            status: MilestonePlanDetectionStatus::Found,
            plan: Some(plan),
            extracted_from,
            source_turn_id: None,
            source_entry_index: None,
            error: None,
        };
    }

    if let Some((extracted_from, schema_version)) = last_unsupported {
        return MilestonePlanDetectionResult {
            status: MilestonePlanDetectionStatus::Unsupported,
            plan: None,
            extracted_from,
            source_turn_id: None,
            source_entry_index: None,
            error: Some(format!(
                "Unsupported schema_version {} (expected {})",
                schema_version, MILESTONE_PLAN_SCHEMA_VERSION_V1
            )),
        };
    }

    MilestonePlanDetectionResult {
        status: MilestonePlanDetectionStatus::Invalid,
        plan: None,
        extracted_from: None,
        source_turn_id: None,
        source_entry_index: None,
        error: Some(last_error.unwrap_or_else(|| "Invalid plan payload".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        detect_milestone_plan_v1_in_text, MilestonePlanDetectionStatus,
        MilestonePlanExtractionKind, MILESTONE_PLAN_SCHEMA_VERSION_V1,
    };

    fn minimal_plan_json(schema_version: i32) -> String {
        format!(
            r#"{{
  "schema_version": {schema_version},
  "milestone": {{}},
  "nodes": [],
  "edges": []
}}"#
        )
    }

    #[test]
    fn detects_fenced_plan() {
        let plan_json = minimal_plan_json(MILESTONE_PLAN_SCHEMA_VERSION_V1);
        let input = format!(
            "Here is a plan:\n\n```milestone-plan-v1\n{}\n```\n",
            plan_json
        );
        let result = detect_milestone_plan_v1_in_text(&input);
        assert!(matches!(result.status, MilestonePlanDetectionStatus::Found));
        assert_eq!(result.extracted_from, Some(MilestonePlanExtractionKind::Fenced));
        assert_eq!(
            result.plan.as_ref().map(|p| p.schema_version),
            Some(MILESTONE_PLAN_SCHEMA_VERSION_V1)
        );
    }

    #[test]
    fn detects_embedded_plan() {
        let plan_json = minimal_plan_json(MILESTONE_PLAN_SCHEMA_VERSION_V1);
        let input = format!("prefix text\n{}\ntrailing text", plan_json);
        let result = detect_milestone_plan_v1_in_text(&input);
        assert!(matches!(result.status, MilestonePlanDetectionStatus::Found));
        assert_eq!(result.extracted_from, Some(MilestonePlanExtractionKind::Embedded));
    }

    #[test]
    fn returns_not_found_when_no_candidate_exists() {
        let result = detect_milestone_plan_v1_in_text("hello world");
        assert!(matches!(result.status, MilestonePlanDetectionStatus::NotFound));
    }

    #[test]
    fn returns_invalid_for_fenced_invalid_json() {
        let input = "```milestone-plan-v1\n{ this is not json }\n```";
        let result = detect_milestone_plan_v1_in_text(input);
        assert!(matches!(result.status, MilestonePlanDetectionStatus::Invalid));
        assert!(result.error.unwrap_or_default().to_lowercase().contains("invalid"));
    }

    #[test]
    fn returns_unsupported_for_schema_mismatch() {
        let plan_json = minimal_plan_json(999);
        let input = format!("```milestone-plan-v1\n{}\n```", plan_json);
        let result = detect_milestone_plan_v1_in_text(&input);
        assert!(matches!(
            result.status,
            MilestonePlanDetectionStatus::Unsupported
        ));
    }

    #[test]
    fn braces_that_do_not_resemble_a_plan_are_not_treated_as_invalid() {
        let input = "this is not json but has braces {like this}";
        let result = detect_milestone_plan_v1_in_text(input);
        assert!(matches!(result.status, MilestonePlanDetectionStatus::NotFound));
    }
}
