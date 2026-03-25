use std::{
    path::Path,
    sync::{Mutex, MutexGuard, OnceLock},
};

pub fn test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub struct TestEnvGuard {
    _lock: MutexGuard<'static, ()>,
    prev_database_url: Option<String>,
    prev_asset_dir: Option<String>,
    prev_disable_background_tasks: Option<String>,
    prev_vk_config_dir: Option<String>,
}

impl TestEnvGuard {
    pub fn new(temp_root: &Path, db_url: String) -> Self {
        let lock = test_lock().lock().unwrap_or_else(|err| err.into_inner());
        let prev_database_url = std::env::var("DATABASE_URL").ok();
        let prev_asset_dir = std::env::var("VIBE_ASSET_DIR").ok();
        let prev_disable_background_tasks = std::env::var("VIBE_DISABLE_BACKGROUND_TASKS").ok();
        let prev_vk_config_dir = std::env::var("VK_CONFIG_DIR").ok();

        let vk_config_dir = temp_root.join("vk-config");
        std::fs::create_dir_all(&vk_config_dir).unwrap();

        // SAFETY: tests using TestEnvGuard are serialized by test_lock.
        unsafe {
            std::env::set_var("VIBE_ASSET_DIR", temp_root);
            std::env::set_var("DATABASE_URL", db_url);
            // Disable background workers (event outbox loop, orphan cleanup, etc.) to avoid
            // cross-task SQLite write contention in tests.
            std::env::set_var("VIBE_DISABLE_BACKGROUND_TASKS", "1");
            std::env::set_var("VK_CONFIG_DIR", &vk_config_dir);
        }

        Self {
            _lock: lock,
            prev_database_url,
            prev_asset_dir,
            prev_disable_background_tasks,
            prev_vk_config_dir,
        }
    }
}

impl Drop for TestEnvGuard {
    fn drop(&mut self) {
        // SAFETY: tests using TestEnvGuard are serialized by test_lock.
        unsafe {
            match &self.prev_database_url {
                Some(value) => std::env::set_var("DATABASE_URL", value),
                None => std::env::remove_var("DATABASE_URL"),
            }
            match &self.prev_asset_dir {
                Some(value) => std::env::set_var("VIBE_ASSET_DIR", value),
                None => std::env::remove_var("VIBE_ASSET_DIR"),
            }
            match &self.prev_disable_background_tasks {
                Some(value) => std::env::set_var("VIBE_DISABLE_BACKGROUND_TASKS", value),
                None => std::env::remove_var("VIBE_DISABLE_BACKGROUND_TASKS"),
            }
            match &self.prev_vk_config_dir {
                Some(value) => std::env::set_var("VK_CONFIG_DIR", value),
                None => std::env::remove_var("VK_CONFIG_DIR"),
            }
        }
    }
}
