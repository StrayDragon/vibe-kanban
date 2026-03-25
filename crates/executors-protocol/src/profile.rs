use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::agent::BaseCodingAgent;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, TS, schemars::JsonSchema)]
pub struct ExecutorProfileId {
    /// The executor type (e.g., "CLAUDE_CODE", "AMP")
    pub executor: BaseCodingAgent,
    /// Optional variant name (e.g., "PLAN", "ROUTER")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

impl ExecutorProfileId {
    pub fn new(executor: BaseCodingAgent) -> Self {
        Self {
            executor,
            variant: None,
        }
    }

    pub fn with_variant(executor: BaseCodingAgent, variant: String) -> Self {
        Self {
            executor,
            variant: Some(variant),
        }
    }

    pub fn cache_key(&self) -> String {
        match &self.variant {
            Some(variant) => format!("{}:{}", self.executor, variant),
            None => self.executor.to_string(),
        }
    }
}

impl fmt::Display for ExecutorProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.variant {
            Some(variant) => write!(f, "{}:{}", self.executor, variant),
            None => write!(f, "{}", self.executor),
        }
    }
}
