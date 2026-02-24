use std::path::PathBuf;

use anyhow::Error;
use executors::{executors::BaseCodingAgent, profile::ExecutorProfileId};
use serde::{Deserialize, Serialize};
use strum_macros::EnumString;
use ts_rs::TS;
use utils::{assets::SoundAssets, cache_dir};

use super::editor::EditorConfig;

pub const CURRENT_CONFIG_VERSION: &str = "v10";

fn default_executor_profile() -> ExecutorProfileId {
    ExecutorProfileId::new(BaseCodingAgent::ClaudeCode)
}

fn default_git_branch_prefix() -> String {
    "vk".to_string()
}

fn default_pr_auto_description_enabled() -> bool {
    true
}

fn default_git_no_verify() -> bool {
    false
}

fn default_diff_preview_guard() -> DiffPreviewGuardPreset {
    DiffPreviewGuardPreset::Balanced
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, Default)]
#[ts(use_ts_enum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiffPreviewGuardPreset {
    Safe,
    #[default]
    Balanced,
    Relaxed,
    Off,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, Default)]
pub struct ShowcaseState {
    #[serde(default)]
    pub seen_features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, EnumString, Default)]
#[ts(use_ts_enum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS, Default)]
#[ts(export)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UiLanguage {
    #[default]
    Browser, // Detect from browser
    En,     // Force English
    ZhHans, // Force Simplified Chinese
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct NotificationConfig {
    #[serde(alias = "soundEnabled")]
    pub sound_enabled: bool,
    #[serde(alias = "pushEnabled")]
    pub push_enabled: bool,
    #[serde(alias = "soundFile")]
    pub sound_file: SoundFile,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            sound_enabled: true,
            push_enabled: true,
            sound_file: SoundFile::CowMooing,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct GitHubConfig {
    pub pat: Option<String>,
    #[serde(alias = "oauthToken")]
    pub oauth_token: Option<String>,
    pub username: Option<String>,
    #[serde(alias = "primaryEmail")]
    pub primary_email: Option<String>,
    #[serde(alias = "defaultPrBase")]
    pub default_pr_base: Option<String>,
}

impl Default for GitHubConfig {
    fn default() -> Self {
        Self {
            pat: None,
            oauth_token: None,
            username: None,
            primary_email: None,
            default_pr_base: Some("main".to_string()),
        }
    }
}

impl GitHubConfig {
    pub fn token(&self) -> Option<String> {
        self.pat
            .as_deref()
            .or(self.oauth_token.as_deref())
            .map(|s| s.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, EnumString)]
#[ts(use_ts_enum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum SoundFile {
    AbstractSound1,
    AbstractSound2,
    AbstractSound3,
    AbstractSound4,
    CowMooing,
    PhoneVibration,
    Rooster,
}

impl SoundFile {
    pub fn to_filename(&self) -> &'static str {
        match self {
            SoundFile::AbstractSound1 => "abstract-sound1.wav",
            SoundFile::AbstractSound2 => "abstract-sound2.wav",
            SoundFile::AbstractSound3 => "abstract-sound3.wav",
            SoundFile::AbstractSound4 => "abstract-sound4.wav",
            SoundFile::CowMooing => "cow-mooing.wav",
            SoundFile::PhoneVibration => "phone-vibration.wav",
            SoundFile::Rooster => "rooster.wav",
        }
    }

    pub async fn serve(&self) -> Result<rust_embed::EmbeddedFile, Error> {
        match SoundAssets::get(self.to_filename()) {
            Some(content) => Ok(content),
            None => {
                tracing::error!("Sound file not found: {}", self.to_filename());
                Err(anyhow::anyhow!(
                    "Sound file not found: {}",
                    self.to_filename()
                ))
            }
        }
    }

    pub async fn get_path(&self) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        use std::io::Write;

        let filename = self.to_filename();
        let cache_dir = cache_dir();
        let cached_path = cache_dir.join(format!("sound-{filename}"));

        if std::fs::metadata(&cached_path)
            .map(|metadata| metadata.len() > 0)
            .unwrap_or(false)
        {
            return Ok(cached_path);
        }

        let sound_data = SoundAssets::get(filename)
            .ok_or_else(|| format!("Embedded sound file not found: {filename}"))?
            .data;

        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| format!("Failed to create cache directory: {e}"))?;

        let mut file = std::fs::File::create(&cached_path)
            .map_err(|e| format!("Failed to create cached sound file: {e}"))?;

        file.write_all(&sound_data)
            .map_err(|e| format!("Failed to write sound data to cached file: {e}"))?;

        drop(file);

        Ok(cached_path)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[ts(use_ts_enum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AccessControlMode {
    #[default]
    Disabled,
    Token,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct AccessControlConfig {
    pub mode: AccessControlMode,
    pub token: Option<String>,
    #[serde(alias = "allowLocalhostBypass")]
    pub allow_localhost_bypass: bool,
}

impl Default for AccessControlConfig {
    fn default() -> Self {
        Self {
            mode: AccessControlMode::Disabled,
            token: None,
            allow_localhost_bypass: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct Config {
    #[serde(alias = "configVersion")]
    pub config_version: String,
    pub theme: ThemeMode,
    #[serde(alias = "executorProfile")]
    pub executor_profile: ExecutorProfileId,
    #[serde(alias = "disclaimerAcknowledged")]
    pub disclaimer_acknowledged: bool,
    #[serde(alias = "onboardingAcknowledged")]
    pub onboarding_acknowledged: bool,
    pub notifications: NotificationConfig,
    pub editor: EditorConfig,
    pub github: GitHubConfig,
    #[serde(alias = "workspaceDir")]
    pub workspace_dir: Option<String>,
    #[serde(alias = "lastAppVersion")]
    pub last_app_version: Option<String>,
    #[serde(alias = "showReleaseNotes")]
    pub show_release_notes: bool,
    pub language: UiLanguage,
    #[serde(alias = "gitBranchPrefix")]
    pub git_branch_prefix: String,
    #[serde(alias = "gitNoVerify")]
    pub git_no_verify: bool,
    pub showcases: ShowcaseState,
    #[serde(alias = "prAutoDescriptionEnabled")]
    pub pr_auto_description_enabled: bool,
    #[serde(alias = "prAutoDescriptionPrompt")]
    pub pr_auto_description_prompt: Option<String>,
    #[serde(alias = "llmanClaudeCodePath")]
    pub llman_claude_code_path: Option<String>,
    #[serde(alias = "diffPreviewGuard")]
    pub diff_preview_guard: DiffPreviewGuardPreset,
    #[serde(alias = "accessControl")]
    pub access_control: AccessControlConfig,
}

impl Config {
    pub fn from_raw(raw_config: &str) -> Self {
        match serde_json::from_str::<Config>(raw_config) {
            Ok(config) => config.normalized(),
            Err(e) => {
                tracing::warn!(
                    "Failed to parse config (line {}, column {}): {}, using default",
                    e.line(),
                    e.column(),
                    e
                );
                Self::default()
            }
        }
    }

    pub fn normalized(mut self) -> Self {
        self.config_version = CURRENT_CONFIG_VERSION.to_string();

        if !utils::git::is_valid_branch_prefix(&self.git_branch_prefix) {
            tracing::warn!(
                "Invalid git branch prefix '{}', resetting to default",
                self.git_branch_prefix
            );
            self.git_branch_prefix = default_git_branch_prefix();
        }

        if matches!(
            self.executor_profile.variant.as_deref(),
            Some(variant) if variant.trim().is_empty()
        ) {
            self.executor_profile.variant = None;
        }

        if matches!(
            self.llman_claude_code_path.as_deref(),
            Some(path) if path.trim().is_empty()
        ) {
            self.llman_claude_code_path = None;
        }

        if matches!(
            self.workspace_dir.as_deref(),
            Some(dir) if dir.trim().is_empty()
        ) {
            self.workspace_dir = None;
        }

        if matches!(
            self.access_control.token.as_deref(),
            Some(token) if token.trim().is_empty()
        ) {
            self.access_control.token = None;
        }

        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: CURRENT_CONFIG_VERSION.to_string(),
            theme: ThemeMode::System,
            executor_profile: default_executor_profile(),
            disclaimer_acknowledged: false,
            onboarding_acknowledged: false,
            notifications: NotificationConfig::default(),
            editor: EditorConfig::default(),
            github: GitHubConfig::default(),
            workspace_dir: None,
            last_app_version: None,
            show_release_notes: false,
            language: UiLanguage::default(),
            git_branch_prefix: default_git_branch_prefix(),
            git_no_verify: default_git_no_verify(),
            showcases: ShowcaseState::default(),
            pr_auto_description_enabled: default_pr_auto_description_enabled(),
            pr_auto_description_prompt: None,
            llman_claude_code_path: None,
            diff_preview_guard: default_diff_preview_guard(),
            access_control: AccessControlConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_apply_for_empty_config() {
        let config = Config::from_raw("{}");

        assert_eq!(config.config_version, CURRENT_CONFIG_VERSION);
        assert!(matches!(config.theme, ThemeMode::System));
        assert_eq!(
            config.executor_profile,
            ExecutorProfileId::new(BaseCodingAgent::ClaudeCode)
        );
        assert_eq!(config.git_branch_prefix, default_git_branch_prefix());
        assert!(!config.disclaimer_acknowledged);
    }

    #[test]
    fn invalid_json_falls_back_to_default() {
        let config = Config::from_raw("{invalid json");

        assert_eq!(config.config_version, CURRENT_CONFIG_VERSION);
        assert_eq!(config.git_branch_prefix, default_git_branch_prefix());
    }

    #[test]
    fn aliases_and_normalization_are_applied() {
        let raw = r#"{
            "configVersion": "v1",
            "gitBranchPrefix": "feature",
            "executorProfile": { "executor": "CLAUDE_CODE" }
        }"#;

        let config = Config::from_raw(raw);

        assert_eq!(config.config_version, CURRENT_CONFIG_VERSION);
        assert_eq!(config.git_branch_prefix, "feature");
    }

    #[test]
    fn invalid_branch_prefix_is_reset() {
        let raw = r#"{ "gitBranchPrefix": "foo/bar" }"#;
        let config = Config::from_raw(raw);

        assert_eq!(config.git_branch_prefix, default_git_branch_prefix());
    }
}
