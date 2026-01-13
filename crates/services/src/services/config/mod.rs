use std::path::PathBuf;

use thiserror::Error;

pub mod editor;
mod schema;

pub use editor::{EditorConfig, EditorOpenError, EditorType};
pub use schema::{
    Config, DiffPreviewGuardPreset, GitHubConfig, NotificationConfig, ShowcaseState, SoundFile,
    ThemeMode, UiLanguage, CURRENT_CONFIG_VERSION,
};

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("Validation error: {0}")]
    ValidationError(String),
}

/// Will always return config, falling back to defaults on missing/invalid files.
pub async fn load_config_from_file(config_path: &PathBuf) -> Config {
    match std::fs::read_to_string(config_path) {
        Ok(raw_config) => Config::from_raw(&raw_config),
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                tracing::info!("No config file found, creating one");
            } else {
                tracing::warn!("Failed to read config file: {}", err);
            }
            Config::default()
        }
    }
}

/// Saves the config to the given path
pub async fn save_config_to_file(
    config: &Config,
    config_path: &PathBuf,
) -> Result<(), ConfigError> {
    let normalized = config.clone().normalized();
    let raw_config = serde_json::to_string_pretty(&normalized)?;
    std::fs::write(config_path, raw_config)?;
    Ok(())
}
