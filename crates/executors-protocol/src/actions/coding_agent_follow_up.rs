use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{agent::BaseCodingAgent, profile::ExecutorProfileId};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
pub struct CodingAgentFollowUpRequest {
    pub prompt: String,
    pub session_id: String,
    /// Executor profile specification
    pub executor_profile_id: ExecutorProfileId,
    /// Optional relative path to execute the agent in (relative to container_ref).
    /// If None, uses the container_ref directory directly.
    #[serde(default)]
    pub working_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_paths: Option<HashMap<String, PathBuf>>,
}

impl CodingAgentFollowUpRequest {
    pub fn base_executor(&self) -> BaseCodingAgent {
        self.executor_profile_id.executor
    }
}
