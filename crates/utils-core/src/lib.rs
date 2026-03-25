use std::{env, sync::OnceLock};

use directories::ProjectDirs;

pub mod approvals;
pub mod browser;
pub mod diff;
pub mod log_entries;
pub mod notifications;
pub mod path;
pub mod port_file;
pub mod response;
pub mod shell;
pub mod stream_lines;
pub mod text;
pub mod tokio;
pub mod version;

const VK_CONFIG_DIR_ENV: &str = "VK_CONFIG_DIR";

/// Cache for WSL2 detection result
static WSL2_CACHE: OnceLock<bool> = OnceLock::new();

/// Check if running in WSL2 (cached)
pub fn is_wsl2() -> bool {
    *WSL2_CACHE.get_or_init(|| {
        // Check for WSL environment variables
        if std::env::var("WSL_DISTRO_NAME").is_ok() || std::env::var("WSLENV").is_ok() {
            tracing::debug!("WSL2 detected via environment variables");
            return true;
        }

        // Check /proc/version for WSL2 signature
        if let Ok(version) = std::fs::read_to_string("/proc/version")
            && (version.contains("WSL2") || version.contains("microsoft"))
        {
            tracing::debug!("WSL2 detected via /proc/version");
            return true;
        }

        tracing::debug!("WSL2 not detected");
        false
    })
}

pub fn cache_dir() -> std::path::PathBuf {
    let proj = if cfg!(debug_assertions) {
        ProjectDirs::from("ai", "bloop-dev", env!("CARGO_PKG_NAME"))
            .expect("OS didn't give us a home directory")
    } else {
        ProjectDirs::from("ai", "bloop", env!("CARGO_PKG_NAME"))
            .expect("OS didn't give us a home directory")
    };

    // ✔ macOS → ~/Library/Caches/MyApp
    // ✔ Linux → ~/.cache/myapp (respects XDG_CACHE_HOME)
    // ✔ Windows → %LOCALAPPDATA%\\Example\\MyApp
    proj.cache_dir().to_path_buf()
}

/// VK config directory.
///
/// - Override: `VK_CONFIG_DIR=/path/to/dir`
/// - Unix (Linux/macOS): `${XDG_CONFIG_HOME:-$HOME/.config}/vk`
/// - Windows: `%APPDATA%\\vk` (via `dirs::config_dir()`), falling back to temp dir
pub fn vk_config_dir() -> std::path::PathBuf {
    fn ensure_dir_exists(path: std::path::PathBuf) -> std::path::PathBuf {
        if let Err(err) = std::fs::create_dir_all(&path) {
            panic!(
                "Failed to create VK config directory '{}': {err}",
                path.display()
            );
        }
        path
    }

    if let Ok(override_dir) = env::var(VK_CONFIG_DIR_ENV) {
        let override_dir = override_dir.trim();
        if !override_dir.is_empty() {
            return ensure_dir_exists(std::path::PathBuf::from(override_dir));
        }
    }

    if cfg!(windows) {
        let base = dirs::config_dir().unwrap_or_else(std::env::temp_dir);
        return ensure_dir_exists(base.join("vk"));
    }

    let base = env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .unwrap_or_else(std::env::temp_dir);
    ensure_dir_exists(base.join("vk"))
}

pub fn vk_config_yaml_path() -> std::path::PathBuf {
    vk_config_dir().join("config.yaml")
}

pub fn vk_secret_env_path() -> std::path::PathBuf {
    vk_config_dir().join("secret.env")
}

pub fn vk_config_schema_path() -> std::path::PathBuf {
    vk_config_dir().join("config.schema.json")
}

#[cfg(test)]
mod vk_config_dir_tests {
    use std::sync::{Mutex, OnceLock};

    use super::*;

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn env_lock() -> &'static Mutex<()> {
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn vk_config_dir_uses_override_and_creates_dir() {
        let _guard = env_lock().lock().unwrap();

        let prev = std::env::var_os(VK_CONFIG_DIR_ENV);
        let tmp = std::env::temp_dir().join(format!("vk-test-{}", uuid::Uuid::new_v4()));
        unsafe {
            std::env::set_var(VK_CONFIG_DIR_ENV, tmp.to_string_lossy().to_string());
        }

        let resolved = vk_config_dir();
        assert_eq!(resolved, tmp);
        assert!(resolved.is_dir());

        match prev {
            Some(value) => unsafe { std::env::set_var(VK_CONFIG_DIR_ENV, value) },
            None => unsafe { std::env::remove_var(VK_CONFIG_DIR_ENV) },
        }
    }
}
