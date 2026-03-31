use std::{env, sync::OnceLock};

use directories::{BaseDirs, ProjectDirs};

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
/// - Windows: `%APPDATA%\\vk` (via `directories::BaseDirs::config_dir()`), falling back to temp dir
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

    let base = BaseDirs::new()
        .map(|dirs| dirs.config_dir().to_path_buf())
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

pub fn vk_projects_yaml_path() -> std::path::PathBuf {
    vk_config_dir().join("projects.yaml")
}

pub fn vk_projects_schema_path() -> std::path::PathBuf {
    vk_config_dir().join("projects.schema.json")
}

pub fn vk_projects_dir() -> std::path::PathBuf {
    vk_config_dir().join("projects.d")
}

pub fn vk_projects_ui_yaml_path() -> std::path::PathBuf {
    vk_config_dir().join("projects.ui.yaml")
}

#[cfg(test)]
mod vk_config_dir_tests {
    use std::sync::{Mutex, OnceLock};

    use super::*;

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn env_lock() -> &'static Mutex<()> {
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvVarGuard {
        key: &'static str,
        prev: Option<std::ffi::OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &std::path::Path) -> Self {
            let prev = std::env::var_os(key);
            // SAFETY: tests using EnvVarGuard are serialized by env_lock().
            unsafe {
                std::env::set_var(key, value.as_os_str());
            }
            Self { key, prev }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            // SAFETY: tests using EnvVarGuard are serialized by env_lock().
            unsafe {
                match &self.prev {
                    Some(value) => std::env::set_var(self.key, value),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }

    #[test]
    fn vk_config_dir_uses_override_and_creates_dir() {
        let _guard = env_lock().lock().unwrap();

        let tmp = std::env::temp_dir().join(format!("vk-test-{}", uuid::Uuid::new_v4()));
        let _env = EnvVarGuard::set(VK_CONFIG_DIR_ENV, &tmp);

        let resolved = vk_config_dir();
        assert_eq!(resolved, tmp);
        assert!(resolved.is_dir());
    }
}
