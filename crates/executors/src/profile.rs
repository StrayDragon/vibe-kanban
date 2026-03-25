use std::{
    collections::HashMap,
    fs,
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

static EXECUTOR_PROFILES_CACHE: LazyLock<RwLock<ExecutorConfigs>> =
    LazyLock::new(|| RwLock::new(ExecutorConfigs::load()));

// New format default profiles (v3 - flattened)
const DEFAULT_PROFILES_JSON: &str = include_str!("../default_profiles.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
pub struct ExecutorConfigs {
    pub executors: HashMap<BaseCodingAgent, ExecutorConfig>,
}

/// On-disk overrides format.
///
/// We persist only diffs from embedded defaults, plus an explicit list of
/// "deleted" built-in variants (tombstones) so users can remove preset
/// configurations from the merged view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ExecutorProfilesOverrides {
    /// Built-in variant keys removed by the user, by executor.
    ///
    /// Stored separately because the overrides file cannot represent deletions
    /// by omission (defaults would re-introduce them on merge).
    #[serde(default)]
    #[serde(rename = "__deleted_variants")]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    deleted_variants: HashMap<BaseCodingAgent, Vec<String>>,

    /// Variant config overrides and user-added variants.
    #[serde(default)]
    executors: HashMap<BaseCodingAgent, ExecutorConfig>,
}

impl ExecutorProfilesOverrides {
    fn canonicalise(&mut self) {
        // Canonicalise executor variant keys
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
                    profile.configurations.entry(new).or_insert(cfg);
                }
            }
        }

        // Canonicalise tombstones, drop DEFAULT (required), and de-dup
        for variants in self.deleted_variants.values_mut() {
            let mut canon = variants
                .iter()
                .map(canonical_variant_key)
                .filter(|k| k != "DEFAULT")
                .collect::<Vec<_>>();
            canon.sort();
            canon.dedup();
            *variants = canon;
        }
        self.deleted_variants.retain(|_, v| !v.is_empty());
    }

    fn apply_deletions_to_defaults(&self, defaults: &mut ExecutorConfigs) {
        for (executor_key, variants) in &self.deleted_variants {
            let Some(profile) = defaults.executors.get_mut(executor_key) else {
                continue;
            };
            for variant in variants {
                profile.configurations.remove(variant);
            }
        }
    }
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

    /// Reload executor profiles cache
    pub fn reload() {
        let mut cache = EXECUTOR_PROFILES_CACHE.write().unwrap();
        *cache = Self::load();
    }

    fn migrate_profiles_json(raw: &str) -> Result<Option<String>, serde_json::Error> {
        fn rename_key(
            obj: &mut serde_json::Map<String, serde_json::Value>,
            from: &str,
            to: &str,
        ) -> bool {
            let Some(value) = obj.remove(from) else {
                return false;
            };
            obj.insert(to.to_string(), value);
            true
        }

        let mut value: serde_json::Value = serde_json::from_str(raw)?;
        let mut changed = false;

        let Some(executors) = value
            .get_mut("executors")
            .and_then(|executors| executors.as_object_mut())
        else {
            return Ok(None);
        };

        changed |= rename_key(executors, "CURSOR", "CURSOR_AGENT");

        for executor_config in executors.values_mut() {
            let Some(variants) = executor_config.as_object_mut() else {
                continue;
            };
            for agent_value in variants.values_mut() {
                let Some(agent_obj) = agent_value.as_object_mut() else {
                    continue;
                };
                if rename_key(agent_obj, "CURSOR", "CURSOR_AGENT") {
                    changed = true;
                }
            }
        }

        if !changed {
            return Ok(None);
        }

        Ok(Some(serde_json::to_string_pretty(&value)?))
    }

    /// Load executor profiles from file or defaults
    pub fn load() -> Self {
        let profiles_path = utils_assets::profiles_path();

        // Load defaults first
        let mut defaults = Self::from_defaults();
        defaults.canonicalise();

        // Try to load user overrides
        let content = match fs::read_to_string(&profiles_path) {
            Ok(content) => content,
            Err(_) => {
                tracing::info!("No user profiles.json found, using defaults only");
                return defaults;
            }
        };

        // Parse user overrides
        let mut user_overrides = match serde_json::from_str::<ExecutorProfilesOverrides>(&content) {
            Ok(user_overrides) => user_overrides,
            Err(parse_err) => match Self::migrate_profiles_json(&content) {
                Ok(Some(migrated)) => {
                    match serde_json::from_str::<ExecutorProfilesOverrides>(&migrated) {
                        Ok(mut migrated_overrides) => {
                            tracing::info!("Migrated legacy profiles.json to current format");
                            migrated_overrides.canonicalise();
                            if let Ok(serialized) =
                                serde_json::to_string_pretty(&migrated_overrides)
                                && let Err(err) = fs::write(&profiles_path, serialized)
                            {
                                tracing::error!("Failed to write migrated profiles.json: {}", err);
                            }
                            migrated_overrides
                        }
                        Err(err) => {
                            tracing::error!(
                                "Failed to parse migrated profiles.json: {}, using defaults only",
                                err
                            );
                            return defaults;
                        }
                    }
                }
                Ok(None) => {
                    tracing::error!(
                        "Failed to parse user profiles.json: {}, using defaults only",
                        parse_err
                    );
                    return defaults;
                }
                Err(err) => {
                    tracing::error!(
                        "Failed to parse user profiles.json: {}, migration failed: {}, using defaults only",
                        parse_err,
                        err
                    );
                    return defaults;
                }
            },
        };

        tracing::info!("Loaded user profile overrides from profiles.json");
        user_overrides.canonicalise();

        // Apply tombstones before merging overrides
        user_overrides.apply_deletions_to_defaults(&mut defaults);
        Self::merge_with_defaults(
            defaults,
            Self {
                executors: user_overrides.executors,
            },
        )
    }

    /// Save user profile overrides to file (only saves what differs from defaults)
    pub fn save_overrides(&self) -> Result<(), ProfileError> {
        let profiles_path = utils_assets::profiles_path();
        let mut defaults = Self::from_defaults();
        defaults.canonicalise();

        // Canonicalise current config before computing overrides
        let mut self_clone = self.clone();
        self_clone.canonicalise();

        // Compute differences from defaults
        let overrides = Self::compute_overrides(&defaults, &self_clone)?;

        // Validate the merged result would be valid
        let merged = Self::merge_with_overrides(defaults, &overrides);
        Self::validate_merged(&merged)?;

        // Write overrides directly to file
        let content = serde_json::to_string_pretty(&overrides)?;
        fs::write(&profiles_path, content)?;

        tracing::info!("Saved profile overrides to {:?}", profiles_path);
        Ok(())
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

    /// Compute what overrides are needed to transform defaults into current config
    fn compute_overrides(
        defaults: &Self,
        current: &Self,
    ) -> Result<ExecutorProfilesOverrides, ProfileError> {
        let mut overrides = Self {
            executors: HashMap::new(),
        };
        let mut deleted_variants: HashMap<BaseCodingAgent, Vec<String>> = HashMap::new();

        // Fast scan for any illegal deletions BEFORE allocating/cloning
        for (executor_key, default_profile) in &defaults.executors {
            // Check if executor was removed entirely
            if !current.executors.contains_key(executor_key) {
                return Err(ProfileError::CannotDeleteExecutor {
                    executor: *executor_key,
                });
            }

            let current_profile = &current.executors[executor_key];

            // Record removed built-in configurations as tombstones.
            // (DEFAULT remains required; deleting it is still rejected.)
            for config_name in default_profile.configurations.keys() {
                if !current_profile.configurations.contains_key(config_name) {
                    if config_name == "DEFAULT" {
                        return Err(ProfileError::CannotDeleteBuiltInConfig {
                            executor: *executor_key,
                            variant: config_name.clone(),
                        });
                    }
                    deleted_variants
                        .entry(*executor_key)
                        .or_default()
                        .push(config_name.clone());
                }
            }
        }

        for (executor_key, current_profile) in &current.executors {
            if let Some(default_profile) = defaults.executors.get(executor_key) {
                let mut override_configurations = HashMap::new();

                // Check each configuration in current profile
                for (config_name, current_config) in &current_profile.configurations {
                    if let Some(default_config) = default_profile.configurations.get(config_name) {
                        // Only include if different from default
                        if current_config != default_config {
                            override_configurations
                                .insert(config_name.clone(), current_config.clone());
                        }
                    } else {
                        // New configuration, always include
                        override_configurations.insert(config_name.clone(), current_config.clone());
                    }
                }

                // Only include executor if there are actual differences
                if !override_configurations.is_empty() {
                    overrides.executors.insert(
                        *executor_key,
                        ExecutorConfig {
                            configurations: override_configurations,
                        },
                    );
                }
            } else {
                // New executor, include completely
                overrides
                    .executors
                    .insert(*executor_key, current_profile.clone());
            }
        }

        // Canonicalise and de-dup deleted variants for stable output
        for variants in deleted_variants.values_mut() {
            variants.sort();
            variants.dedup();
        }
        deleted_variants.retain(|_, v| !v.is_empty());

        Ok(ExecutorProfilesOverrides {
            deleted_variants,
            executors: overrides.executors,
        })
    }

    fn merge_with_overrides(mut defaults: Self, overrides: &ExecutorProfilesOverrides) -> Self {
        overrides.apply_deletions_to_defaults(&mut defaults);
        Self::merge_with_defaults(
            defaults,
            Self {
                executors: overrides.executors.clone(),
            },
        )
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
    fn compute_overrides_records_builtin_variant_deletions() {
        let mut defaults = ExecutorConfigs::from_defaults();
        defaults.canonicalise();

        let mut current = defaults.clone();
        let claude = current
            .executors
            .get_mut(&BaseCodingAgent::ClaudeCode)
            .expect("CLAUDE_CODE profile");
        assert!(claude.configurations.contains_key("PLAN"));
        claude.configurations.remove("PLAN");

        let overrides =
            ExecutorConfigs::compute_overrides(&defaults, &current).expect("compute overrides");
        let deleted = overrides
            .deleted_variants
            .get(&BaseCodingAgent::ClaudeCode)
            .cloned()
            .unwrap_or_default();
        assert_eq!(deleted, vec!["PLAN".to_string()]);
        assert!(overrides.executors.is_empty());

        let merged = ExecutorConfigs::merge_with_overrides(defaults, &overrides);
        assert_eq!(merged, current);
    }

    #[test]
    fn compute_overrides_supports_builtin_deletion_and_custom_addition() {
        let mut defaults = ExecutorConfigs::from_defaults();
        defaults.canonicalise();

        let mut current = defaults.clone();
        let claude = current
            .executors
            .get_mut(&BaseCodingAgent::ClaudeCode)
            .expect("CLAUDE_CODE profile");

        claude
            .configurations
            .remove("OPUS")
            .expect("OPUS exists in defaults");
        let default_cfg = claude
            .configurations
            .get("DEFAULT")
            .cloned()
            .expect("DEFAULT exists");
        claude
            .configurations
            .insert("CUSTOM".to_string(), default_cfg);

        let overrides =
            ExecutorConfigs::compute_overrides(&defaults, &current).expect("compute overrides");
        let deleted = overrides
            .deleted_variants
            .get(&BaseCodingAgent::ClaudeCode)
            .cloned()
            .unwrap_or_default();
        assert_eq!(deleted, vec!["OPUS".to_string()]);

        let claude_overrides = overrides
            .executors
            .get(&BaseCodingAgent::ClaudeCode)
            .expect("CLAUDE_CODE overrides");
        assert!(claude_overrides.configurations.contains_key("CUSTOM"));
        assert!(!claude_overrides.configurations.contains_key("DEFAULT"));

        let merged = ExecutorConfigs::merge_with_overrides(defaults, &overrides);
        assert_eq!(merged, current);
    }

    #[test]
    fn compute_overrides_rejects_deleting_default_variant() {
        let mut defaults = ExecutorConfigs::from_defaults();
        defaults.canonicalise();

        let mut current = defaults.clone();
        let claude = current
            .executors
            .get_mut(&BaseCodingAgent::ClaudeCode)
            .expect("CLAUDE_CODE profile");
        claude.configurations.remove("DEFAULT");

        let err = ExecutorConfigs::compute_overrides(&defaults, &current).unwrap_err();
        assert!(matches!(
            err,
            ProfileError::CannotDeleteBuiltInConfig { .. }
        ));
    }

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
