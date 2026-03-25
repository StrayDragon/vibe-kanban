use std::{
    collections::HashMap,
    sync::{LazyLock, RwLock},
};

use convert_case::{Case, Casing};
use executors_protocol::{BaseCodingAgent, ExecutorProfileId};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::executors::{AvailabilityInfo, CodingAgent, StandardCodingAgentExecutor};

/// Return the canonical form for variant keys.
/// – "DEFAULT" is kept as-is  
/// – everything else is converted to SCREAMING_SNAKE_CASE
pub fn canonical_variant_key<S: AsRef<str>>(raw: S) -> String {
    let key = raw.as_ref();
    if key.eq_ignore_ascii_case("DEFAULT") {
        "DEFAULT".to_string()
    } else {
        // Convert to SCREAMING_SNAKE_CASE by first going to snake_case then uppercase
        key.to_case(Case::Snake).to_case(Case::ScreamingSnake)
    }
}

#[derive(Error, Debug)]
pub enum ProfileError {
    #[error("Built-in executor '{executor}' cannot be deleted")]
    CannotDeleteExecutor { executor: BaseCodingAgent },

    #[error("Built-in configuration '{executor}:{variant}' cannot be deleted")]
    CannotDeleteBuiltInConfig {
        executor: BaseCodingAgent,
        variant: String,
    },

    #[error("Validation error: {0}")]
    Validation(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error("No available executor profile")]
    NoAvailableExecutorProfile,
}

static EXECUTOR_PROFILES_CACHE: LazyLock<RwLock<ExecutorConfigs>> = LazyLock::new(|| {
    let mut defaults = ExecutorConfigs::from_defaults();
    defaults.canonicalise();
    RwLock::new(defaults)
});

// New format default profiles (v3 - flattened)
const DEFAULT_PROFILES_JSON: &str = include_str!("../default_profiles.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, schemars::JsonSchema)]
pub struct ExecutorConfig {
    #[serde(flatten)]
    pub configurations: HashMap<String, CodingAgent>,
}

impl ExecutorConfig {
    /// Get variant configuration by name, or None if not found
    pub fn get_variant(&self, variant: &str) -> Option<&CodingAgent> {
        self.configurations.get(variant)
    }

    /// Get the default configuration for this executor
    pub fn get_default(&self) -> Option<&CodingAgent> {
        self.configurations.get("DEFAULT")
    }

    /// Create a new executor profile with just a default configuration
    pub fn new_with_default(default_config: CodingAgent) -> Self {
        let mut configurations = HashMap::new();
        configurations.insert("DEFAULT".to_string(), default_config);
        Self { configurations }
    }

    /// Add or update a variant configuration
    pub fn set_variant(
        &mut self,
        variant_name: String,
        config: CodingAgent,
    ) -> Result<(), &'static str> {
        let key = canonical_variant_key(&variant_name);
        if key == "DEFAULT" {
            return Err(
                "Cannot override 'DEFAULT' variant using set_variant, use set_default instead",
            );
        }
        self.configurations.insert(key, config);
        Ok(())
    }

    /// Set the default configuration
    pub fn set_default(&mut self, config: CodingAgent) {
        self.configurations.insert("DEFAULT".to_string(), config);
    }

    /// Get all variant names (excluding "DEFAULT")
    pub fn variant_names(&self) -> Vec<&String> {
        self.configurations
            .keys()
            .filter(|k| *k != "DEFAULT")
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, schemars::JsonSchema)]
pub struct ExecutorConfigs {
    pub executors: HashMap<BaseCodingAgent, ExecutorConfig>,
}

impl ExecutorConfigs {
    /// Normalise all variant keys in-place
    fn canonicalise(&mut self) {
        for profile in self.executors.values_mut() {
            let mut replacements = Vec::new();
            for key in profile.configurations.keys().cloned().collect::<Vec<_>>() {
                let canon = canonical_variant_key(&key);
                if canon != key {
                    replacements.push((key, canon));
                }
            }
            for (old, new) in replacements {
                if let Some(cfg) = profile.configurations.remove(&old) {
                    // If both lowercase and canonical forms existed, keep canonical one
                    profile.configurations.entry(new).or_insert(cfg);
                }
            }
        }
    }

    /// Get cached executor profiles
    pub fn get_cached() -> ExecutorConfigs {
        EXECUTOR_PROFILES_CACHE.read().unwrap().clone()
    }

    pub fn set_cached(configs: ExecutorConfigs) {
        let mut cache = EXECUTOR_PROFILES_CACHE.write().unwrap();
        *cache = configs;
    }

    /// Builds the runtime profiles by merging optional overrides onto the embedded defaults.
    ///
    /// - Variant keys are canonicalised (`default` → `DEFAULT`, `plan` → `PLAN`, …).
    /// - The merged result is validated (e.g., `DEFAULT` must exist and match the executor key).
    pub fn from_defaults_merged_with_overrides(
        overrides: Option<&ExecutorConfigs>,
    ) -> Result<Self, ProfileError> {
        let mut defaults = Self::from_defaults();
        defaults.canonicalise();

        let Some(overrides) = overrides else {
            Self::validate_merged(&defaults)?;
            return Ok(defaults);
        };

        let mut overrides = overrides.clone();
        overrides.canonicalise();

        let merged = Self::merge_with_defaults(defaults, overrides);
        Self::validate_merged(&merged)?;
        Ok(merged)
    }

    /// Deep merge defaults with user overrides
    fn merge_with_defaults(mut defaults: Self, overrides: Self) -> Self {
        for (executor_key, override_profile) in overrides.executors {
            match defaults.executors.get_mut(&executor_key) {
                Some(default_profile) => {
                    // Merge configurations (user configs override defaults, new ones are added)
                    for (config_name, config) in override_profile.configurations {
                        default_profile.configurations.insert(config_name, config);
                    }
                }
                None => {
                    // New executor, add completely
                    defaults.executors.insert(executor_key, override_profile);
                }
            }
        }
        defaults
    }

    /// Validate that merged profiles are consistent and valid
    fn validate_merged(merged: &Self) -> Result<(), ProfileError> {
        for (executor_key, profile) in &merged.executors {
            // Ensure default configuration exists
            let default_config = profile.configurations.get("DEFAULT").ok_or_else(|| {
                ProfileError::Validation(format!(
                    "Executor '{executor_key}' is missing required 'default' configuration"
                ))
            })?;

            // Validate that the default agent type matches the executor key
            if default_config.base_agent() != *executor_key {
                return Err(ProfileError::Validation(format!(
                    "Executor key '{executor_key}' does not match the agent variant '{default_config}'"
                )));
            }

            // Ensure configuration names don't conflict with reserved words
            for config_name in profile.configurations.keys() {
                if config_name.starts_with("__") {
                    return Err(ProfileError::Validation(format!(
                        "Configuration name '{config_name}' is reserved (starts with '__')"
                    )));
                }
            }

            // Validate auto-retry config for each variant
            for config in profile.configurations.values() {
                config
                    .validate_auto_retry()
                    .map_err(ProfileError::Validation)?;
            }
        }
        Ok(())
    }

    /// Load from the new v3 defaults
    pub fn from_defaults() -> Self {
        serde_json::from_str(DEFAULT_PROFILES_JSON).unwrap_or_else(|e| {
            tracing::error!("Failed to parse embedded default_profiles.json: {}", e);
            panic!("Default profiles v3 JSON is invalid")
        })
    }

    pub fn get_coding_agent(&self, executor_profile_id: &ExecutorProfileId) -> Option<CodingAgent> {
        self.executors
            .get(&executor_profile_id.executor)
            .and_then(|executor| {
                executor.get_variant(
                    &executor_profile_id
                        .variant
                        .clone()
                        .unwrap_or("DEFAULT".to_string()),
                )
            })
            .cloned()
    }

    pub fn require_coding_agent(
        &self,
        executor_profile_id: &ExecutorProfileId,
    ) -> Result<CodingAgent, ProfileError> {
        let Some(executor) = self.executors.get(&executor_profile_id.executor) else {
            let mut supported = self
                .executors
                .keys()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            supported.sort();
            return Err(ProfileError::Validation(format!(
                "Unsupported executor '{}'. Supported executors: {}",
                executor_profile_id.executor,
                supported.join(", ")
            )));
        };

        let variant = executor_profile_id
            .variant
            .clone()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "DEFAULT".to_string());
        let variant = canonical_variant_key(variant);

        let Some(agent) = executor.get_variant(&variant) else {
            let mut variants = executor
                .configurations
                .keys()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            variants.sort();
            return Err(ProfileError::Validation(format!(
                "Unsupported executor profile '{}:{}'. Available variants: {}",
                executor_profile_id.executor,
                variant,
                variants.join(", ")
            )));
        };

        Ok(agent.clone())
    }

    pub fn get_coding_agent_or_default(
        &self,
        executor_profile_id: &ExecutorProfileId,
    ) -> CodingAgent {
        self.get_coding_agent(executor_profile_id)
            .unwrap_or_else(|| {
                let mut default_executor_profile_id = executor_profile_id.clone();
                default_executor_profile_id.variant = Some("DEFAULT".to_string());
                self.get_coding_agent(&default_executor_profile_id)
                    .expect("No default variant found")
            })
    }
    pub async fn get_recommended_executor_profile(
        &self,
    ) -> Result<ExecutorProfileId, ProfileError> {
        let mut agents_with_info: Vec<(BaseCodingAgent, AvailabilityInfo)> = Vec::new();

        for &base_agent in self.executors.keys() {
            if base_agent == BaseCodingAgent::FakeAgent {
                continue;
            }
            let profile_id = ExecutorProfileId::new(base_agent);
            if let Some(coding_agent) = self.get_coding_agent(&profile_id) {
                let info = coding_agent.get_availability_info();
                if info.is_available() {
                    agents_with_info.push((base_agent, info));
                }
            }
        }

        if agents_with_info.is_empty() {
            return Err(ProfileError::NoAvailableExecutorProfile);
        }

        agents_with_info.sort_by(|a, b| {
            use crate::executors::AvailabilityInfo;
            match (&a.1, &b.1) {
                // Both have login detected - compare timestamps (most recent first)
                (
                    AvailabilityInfo::LoginDetected {
                        last_auth_timestamp: time_a,
                    },
                    AvailabilityInfo::LoginDetected {
                        last_auth_timestamp: time_b,
                    },
                ) => time_b.cmp(time_a),
                // LoginDetected > InstallationFound
                (AvailabilityInfo::LoginDetected { .. }, AvailabilityInfo::InstallationFound) => {
                    std::cmp::Ordering::Less
                }
                (AvailabilityInfo::InstallationFound, AvailabilityInfo::LoginDetected { .. }) => {
                    std::cmp::Ordering::Greater
                }
                // LoginDetected > NotFound
                (AvailabilityInfo::LoginDetected { .. }, AvailabilityInfo::NotFound) => {
                    std::cmp::Ordering::Less
                }
                (AvailabilityInfo::NotFound, AvailabilityInfo::LoginDetected { .. }) => {
                    std::cmp::Ordering::Greater
                }
                // InstallationFound > NotFound
                (AvailabilityInfo::InstallationFound, AvailabilityInfo::NotFound) => {
                    std::cmp::Ordering::Less
                }
                (AvailabilityInfo::NotFound, AvailabilityInfo::InstallationFound) => {
                    std::cmp::Ordering::Greater
                }
                // Same state - equal
                _ => std::cmp::Ordering::Equal,
            }
        });

        let selected = agents_with_info[0].0;
        tracing::info!("Recommended executor: {}", selected);
        Ok(ExecutorProfileId::new(selected))
    }
}

pub fn to_default_variant(id: &ExecutorProfileId) -> ExecutorProfileId {
    ExecutorProfileId {
        executor: id.executor,
        variant: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn require_coding_agent_errors_for_unsupported_executor() {
        let configs = ExecutorConfigs::from_defaults();
        let id = ExecutorProfileId::new(BaseCodingAgent::Gemini);

        let err = configs.require_coding_agent(&id).unwrap_err();
        match err {
            ProfileError::Validation(message) => {
                assert!(message.contains("Unsupported executor"));
                assert!(message.contains("CLAUDE_CODE"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn require_coding_agent_errors_for_unsupported_variant() {
        let configs = ExecutorConfigs::from_defaults();
        let id =
            ExecutorProfileId::with_variant(BaseCodingAgent::ClaudeCode, "missing".to_string());

        let err = configs.require_coding_agent(&id).unwrap_err();
        match err {
            ProfileError::Validation(message) => {
                assert!(message.contains("Unsupported executor profile"));
                assert!(message.contains("DEFAULT"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
