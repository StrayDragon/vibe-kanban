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
}

impl TestEnvGuard {
    pub fn new(temp_root: &Path, db_url: String) -> Self {
        let lock = test_lock().lock().unwrap_or_else(|err| err.into_inner());
        let prev_database_url = std::env::var("DATABASE_URL").ok();
        let prev_asset_dir = std::env::var("VIBE_ASSET_DIR").ok();

        // SAFETY: tests using TestEnvGuard are serialized by test_lock.
        unsafe {
            std::env::set_var("VIBE_ASSET_DIR", temp_root);
            std::env::set_var("DATABASE_URL", db_url);
        }

        Self {
            _lock: lock,
            prev_database_url,
            prev_asset_dir,
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
        }
    }
}
