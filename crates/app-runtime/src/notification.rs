use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use config::{Config, NotificationConfig, SoundFile};
use tokio::sync::RwLock;
use utils_core::notifications::Notifier;

#[derive(Debug, Clone)]
pub struct NotificationService {
    config: Arc<RwLock<Config>>,
}

static WSL_ROOT_PATH_CACHE: OnceLock<Option<String>> = OnceLock::new();

impl NotificationService {
    pub fn new(config: Arc<RwLock<Config>>) -> Self {
        Self { config }
    }

    async fn send_notification(config: &NotificationConfig, title: &str, message: &str) {
        if config.sound_enabled {
            Self::play_sound_notification(&config.sound_file).await;
        }

        if config.push_enabled {
            Self::send_push_notification(title, message).await;
        }
    }

    async fn play_sound_notification(sound_file: &SoundFile) {
        let file_path = match sound_file.get_path().await {
            Ok(path) => path,
            Err(error) => {
                tracing::error!("Failed to create cached sound file: {}", error);
                return;
            }
        };

        if cfg!(target_os = "macos") {
            let _ = tokio::process::Command::new("afplay")
                .arg(&file_path)
                .spawn();
        } else if cfg!(target_os = "linux") && !utils_core::is_wsl2() {
            let played = tokio::process::Command::new("paplay")
                .arg(&file_path)
                .spawn()
                .is_ok()
                || tokio::process::Command::new("aplay")
                    .arg(&file_path)
                    .spawn()
                    .is_ok();
            if !played {
                let _ = tokio::process::Command::new("echo")
                    .arg("-e")
                    .arg("\\a")
                    .spawn();
            }
        } else if cfg!(target_os = "windows")
            || (cfg!(target_os = "linux") && utils_core::is_wsl2())
        {
            let file_path = if utils_core::is_wsl2() {
                if let Some(windows_path) = Self::wsl_to_windows_path(&file_path).await {
                    windows_path
                } else {
                    file_path.to_string_lossy().to_string()
                }
            } else {
                file_path.to_string_lossy().to_string()
            };

            let _ = tokio::process::Command::new("powershell.exe")
                .arg("-c")
                .arg(format!(
                    r#"(New-Object Media.SoundPlayer \"{file_path}\").PlaySync()"#
                ))
                .spawn();
        }
    }

    async fn send_push_notification(title: &str, message: &str) {
        if cfg!(target_os = "macos") {
            Self::send_macos_notification(title, message).await;
        } else if cfg!(target_os = "linux") && !utils_core::is_wsl2() {
            Self::send_linux_notification(title, message).await;
        } else if cfg!(target_os = "windows")
            || (cfg!(target_os = "linux") && utils_core::is_wsl2())
        {
            Self::send_windows_notification(title, message).await;
        }
    }

    async fn send_macos_notification(title: &str, message: &str) {
        let script = format!(
            r#"display notification \"{message}\" with title \"{title}\" sound name \"Glass\""#,
            message = message.replace('"', r#"\""#),
            title = title.replace('"', r#"\""#)
        );

        let _ = tokio::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .spawn();
    }

    async fn send_linux_notification(title: &str, message: &str) {
        use notify_rust::Notification;

        let title = title.to_string();
        let message = message.to_string();

        let handle = tokio::task::spawn_blocking(move || {
            if let Err(error) = Notification::new()
                .summary(&title)
                .body(&message)
                .timeout(10000)
                .show()
            {
                tracing::error!("Failed to send Linux notification: {}", error);
            }
        });
        drop(handle);
    }

    async fn send_windows_notification(title: &str, message: &str) {
        let script_path = match utils_assets::get_powershell_script().await {
            Ok(path) => path,
            Err(error) => {
                tracing::error!("Failed to get PowerShell script: {}", error);
                return;
            }
        };

        let script_path_str = if utils_core::is_wsl2() {
            if let Some(windows_path) = Self::wsl_to_windows_path(&script_path).await {
                windows_path
            } else {
                script_path.to_string_lossy().to_string()
            }
        } else {
            script_path.to_string_lossy().to_string()
        };

        let _ = tokio::process::Command::new("powershell.exe")
            .arg("-NoProfile")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-File")
            .arg(script_path_str)
            .arg("-Title")
            .arg(title)
            .arg("-Message")
            .arg(message)
            .spawn();
    }

    async fn get_wsl_root_path() -> Option<String> {
        if let Some(cached) = WSL_ROOT_PATH_CACHE.get() {
            return cached.clone();
        }

        match tokio::process::Command::new("powershell.exe")
            .arg("-c")
            .arg("(Get-Location).Path -replace '^.*::', ''")
            .current_dir("/")
            .output()
            .await
        {
            Ok(output) => match String::from_utf8(output.stdout) {
                Ok(pwd_str) => {
                    let pwd = pwd_str.trim();
                    tracing::info!("WSL root path detected: {}", pwd);
                    let _ = WSL_ROOT_PATH_CACHE.set(Some(pwd.to_string()));
                    return Some(pwd.to_string());
                }
                Err(error) => {
                    tracing::error!("Failed to parse PowerShell pwd output as UTF-8: {}", error);
                }
            },
            Err(error) => {
                tracing::error!("Failed to execute PowerShell pwd command: {}", error);
            }
        }

        let _ = WSL_ROOT_PATH_CACHE.set(None);
        None
    }

    async fn wsl_to_windows_path(wsl_path: &std::path::Path) -> Option<String> {
        let path_str = wsl_path.to_string_lossy();

        if !path_str.starts_with('/') {
            tracing::debug!("Using relative path as-is: {}", path_str);
            return Some(path_str.to_string());
        }

        if let Some(wsl_root) = Self::get_wsl_root_path().await {
            let windows_path = format!("{wsl_root}{path_str}");
            tracing::debug!("WSL path converted: {} -> {}", path_str, windows_path);
            Some(windows_path)
        } else {
            tracing::error!(
                "Failed to determine WSL root path for conversion: {}",
                path_str
            );
            None
        }
    }
}

#[async_trait]
impl Notifier for NotificationService {
    async fn notify(&self, title: &str, message: &str) {
        let config = self.config.read().await.notifications.clone();
        Self::send_notification(&config, title, message).await;
    }
}
