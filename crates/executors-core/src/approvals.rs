use std::fmt;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use utils_core::approvals::ApprovalStatus;

/// Errors emitted by executor approval services.
#[derive(Debug, Error)]
pub enum ExecutorApprovalError {
    #[error("executor approval session not registered")]
    SessionNotRegistered,
    #[error("executor approval request failed: {0}")]
    RequestFailed(String),
    #[error("executor approval service unavailable")]
    ServiceUnavailable,
}

impl ExecutorApprovalError {
    pub fn request_failed<E: fmt::Display>(err: E) -> Self {
        Self::RequestFailed(err.to_string())
    }
}

/// Abstraction for executor approval backends.
#[async_trait]
pub trait ExecutorApprovalService: Send + Sync {
    /// Requests approval for a tool invocation and waits for the final decision.
    async fn request_tool_approval(
        &self,
        tool_name: &str,
        tool_input: Value,
        tool_call_id: &str,
    ) -> Result<ApprovalStatus, ExecutorApprovalError>;

    /// Optional hook: executor is waiting for a human decision (non-tool-approval flow).
    ///
    /// Default is a no-op so executors can call this without requiring every backend
    /// to implement task status transitions.
    async fn notify_task_needs_review(&self) -> Result<(), ExecutorApprovalError> {
        Ok(())
    }

    /// Optional hook: executor resumed from a human-decision state.
    ///
    /// Default is a no-op so executors can call this without requiring every backend
    /// to implement task status transitions.
    async fn notify_task_resumed(&self) -> Result<(), ExecutorApprovalError> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct NoopExecutorApprovalService;

#[async_trait]
impl ExecutorApprovalService for NoopExecutorApprovalService {
    async fn request_tool_approval(
        &self,
        _tool_name: &str,
        _tool_input: Value,
        _tool_call_id: &str,
    ) -> Result<ApprovalStatus, ExecutorApprovalError> {
        Ok(ApprovalStatus::Approved)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallMetadata {
    pub tool_call_id: String,
}
