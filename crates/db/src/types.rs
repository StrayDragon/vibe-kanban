use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};
use ts_rs::TS;

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
    EnumString,
    Display,
    Default,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum TaskStatus {
    #[default]
    #[sea_orm(string_value = "todo")]
    Todo,
    #[sea_orm(string_value = "inprogress")]
    InProgress,
    #[sea_orm(string_value = "inreview")]
    InReview,
    #[sea_orm(string_value = "done")]
    Done,
    #[sea_orm(string_value = "cancelled")]
    Cancelled,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
    EnumString,
    Display,
    Default,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum MilestoneAutomationMode {
    #[default]
    #[sea_orm(string_value = "manual")]
    Manual,
    #[sea_orm(string_value = "auto")]
    Auto,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
    EnumString,
    Display,
    Default,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum TaskCreatedByKind {
    #[default]
    #[sea_orm(string_value = "human_ui")]
    HumanUi,
    #[sea_orm(string_value = "mcp")]
    Mcp,
    #[sea_orm(string_value = "scheduler")]
    Scheduler,
    #[sea_orm(string_value = "agent_followup")]
    AgentFollowup,
    #[sea_orm(string_value = "milestone_planner")]
    MilestonePlanner,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
    EnumString,
    Display,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum WorkspaceLifecycleHookFailurePolicy {
    #[sea_orm(string_value = "block_start")]
    BlockStart,
    #[sea_orm(string_value = "warn_only")]
    WarnOnly,
    #[sea_orm(string_value = "block_cleanup")]
    BlockCleanup,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
    EnumString,
    Display,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum WorkspaceLifecycleHookRunMode {
    #[sea_orm(string_value = "once_per_workspace")]
    OncePerWorkspace,
    #[sea_orm(string_value = "every_prepare")]
    EveryPrepare,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
    EnumString,
    Display,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum WorkspaceLifecycleHookPhase {
    #[sea_orm(string_value = "after_prepare")]
    AfterPrepare,
    #[sea_orm(string_value = "before_cleanup")]
    BeforeCleanup,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
    EnumString,
    Display,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum WorkspaceLifecycleHookStatus {
    #[sea_orm(string_value = "succeeded")]
    Succeeded,
    #[sea_orm(string_value = "failed")]
    Failed,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
    EnumString,
    Display,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum TaskDispatchController {
    #[sea_orm(string_value = "manual")]
    Manual,
    #[sea_orm(string_value = "scheduler")]
    Scheduler,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
    EnumString,
    Display,
    Default,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum TaskDispatchStatus {
    #[default]
    #[sea_orm(string_value = "idle")]
    Idle,
    #[sea_orm(string_value = "claimed")]
    Claimed,
    #[sea_orm(string_value = "running")]
    Running,
    #[sea_orm(string_value = "retry_scheduled")]
    RetryScheduled,
    #[sea_orm(string_value = "awaiting_human_review")]
    AwaitingHumanReview,
    #[sea_orm(string_value = "blocked")]
    Blocked,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
    EnumString,
    Display,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum VkNextAction {
    #[sea_orm(string_value = "continue")]
    Continue,
    #[sea_orm(string_value = "review")]
    Review,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
    EnumString,
    Display,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum TaskContinuationStopReasonCode {
    #[sea_orm(string_value = "disabled")]
    Disabled,
    #[sea_orm(string_value = "budget_exhausted")]
    BudgetExhausted,
    #[sea_orm(string_value = "approval_pending")]
    ApprovalPending,
    #[sea_orm(string_value = "vk_next_review")]
    VkNextReview,
    #[sea_orm(string_value = "vk_next_missing")]
    VkNextMissing,
    #[sea_orm(string_value = "vk_next_invalid")]
    VkNextInvalid,
    #[sea_orm(string_value = "human_queued_follow_up")]
    HumanQueuedFollowUp,
    #[sea_orm(string_value = "task_not_actionable")]
    TaskNotActionable,
    #[sea_orm(string_value = "start_failed")]
    StartFailed,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
    EnumString,
    Display,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum TaskControlTransferReasonCode {
    #[sea_orm(string_value = "human_takeover")]
    HumanTakeover,
    #[sea_orm(string_value = "human_resume")]
    HumanResume,
    #[sea_orm(string_value = "awaiting_human_review")]
    AwaitingHumanReview,
    #[sea_orm(string_value = "policy_rejected_profile")]
    PolicyRejectedProfile,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
    EnumString,
    Display,
    Default,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum TaskKind {
    #[default]
    #[sea_orm(string_value = "default")]
    Default,
    #[sea_orm(string_value = "milestone")]
    Milestone,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, TS)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "lowercase")]
#[ts(use_ts_enum)]
pub enum ExecutionProcessStatus {
    #[sea_orm(string_value = "running")]
    Running,
    #[sea_orm(string_value = "completed")]
    Completed,
    #[sea_orm(string_value = "failed")]
    Failed,
    #[sea_orm(string_value = "killed")]
    Killed,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, TS)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "lowercase")]
pub enum ExecutionProcessRunReason {
    #[sea_orm(string_value = "setupscript")]
    SetupScript,
    #[sea_orm(string_value = "cleanupscript")]
    CleanupScript,
    #[sea_orm(string_value = "codingagent")]
    CodingAgent,
    #[sea_orm(string_value = "devserver")]
    DevServer,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, TS)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum MergeStatus {
    #[sea_orm(string_value = "open")]
    Open,
    #[sea_orm(string_value = "merged")]
    Merged,
    #[sea_orm(string_value = "closed")]
    Closed,
    #[sea_orm(string_value = "unknown")]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, TS)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum MergeType {
    #[sea_orm(string_value = "direct")]
    Direct,
    #[sea_orm(string_value = "pr")]
    Pr,
}
