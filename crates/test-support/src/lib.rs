use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::{OsStr, OsString},
    marker::PhantomData,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Mutex, MutexGuard, OnceLock},
};

/// A global lock for tests that mutate process-global env vars.
///
/// Mutating env vars is `unsafe` in Rust 2024 because it can cause UB if another thread reads the
/// environment concurrently. Tests should serialize env mutations to avoid cross-test interference.
pub fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

thread_local! {
    static ENV_LOCK_STATE: RefCell<EnvLockState> = const {
        RefCell::new(EnvLockState { depth: 0, guard: None })
    };
}

struct EnvLockState {
    depth: usize,
    guard: Option<MutexGuard<'static, ()>>,
}

/// An RAII guard that holds the global env lock.
///
/// This lock is re-entrant within the same thread (nested guards won't deadlock) but still
/// serializes env mutation across threads.
pub struct EnvLockGuard {
    _private: (),
    _not_send_or_sync: PhantomData<Rc<()>>,
}

pub fn lock_env() -> EnvLockGuard {
    ENV_LOCK_STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.depth == 0 {
            state.guard = Some(env_lock().lock().unwrap_or_else(|err| err.into_inner()));
        }
        state.depth += 1;
    });

    EnvLockGuard {
        _private: (),
        _not_send_or_sync: PhantomData,
    }
}

impl Drop for EnvLockGuard {
    fn drop(&mut self) {
        ENV_LOCK_STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.depth = state.depth.saturating_sub(1);
            if state.depth == 0 {
                state.guard.take();
            }
        });
    }
}

/// RAII env var guard that restores modified vars on drop.
///
/// Holds the global env lock for the lifetime of the guard.
pub struct EnvVarGuard {
    _lock: EnvLockGuard,
    previous: HashMap<OsString, Option<OsString>>,
}

impl EnvVarGuard {
    pub fn new() -> Self {
        Self {
            _lock: lock_env(),
            previous: HashMap::new(),
        }
    }

    pub fn set<K, V>(key: K, value: V) -> Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        let mut guard = Self::new();
        guard.set_var(key, value);
        guard
    }

    pub fn set_optional<K>(key: K, value: Option<&str>) -> Self
    where
        K: AsRef<OsStr>,
    {
        let mut guard = Self::new();
        guard.set_optional_var(key, value);
        guard
    }

    pub fn set_var<K, V>(&mut self, key: K, value: V)
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        let key = key.as_ref().to_os_string();
        if !self.previous.contains_key(&key) {
            self.previous.insert(key.clone(), std::env::var_os(&key));
        }

        // SAFETY: env mutations are serialized by EnvLockGuard.
        unsafe {
            std::env::set_var(&key, value.as_ref());
        }
    }

    pub fn set_optional_var<K>(&mut self, key: K, value: Option<&str>)
    where
        K: AsRef<OsStr>,
    {
        match value {
            Some(value) => self.set_var(key, value),
            None => self.remove_var(key),
        }
    }

    pub fn remove_var<K>(&mut self, key: K)
    where
        K: AsRef<OsStr>,
    {
        let key = key.as_ref().to_os_string();
        if !self.previous.contains_key(&key) {
            self.previous.insert(key.clone(), std::env::var_os(&key));
        }

        // SAFETY: env mutations are serialized by EnvLockGuard.
        unsafe {
            std::env::remove_var(&key);
        }
    }
}

impl Default for EnvVarGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        // SAFETY: env mutations are serialized by EnvLockGuard.
        unsafe {
            for (key, prev) in self.previous.drain() {
                match prev {
                    Some(value) => std::env::set_var(&key, value),
                    None => std::env::remove_var(&key),
                }
            }
        }
    }
}

/// A unique temporary root directory.
///
/// Uses the OS temp directory by default and removes the directory on drop.
pub struct TempRoot {
    dir: tempfile::TempDir,
}

impl TempRoot {
    pub fn new(prefix: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(prefix)
            .tempdir()
            .expect("create temp dir");
        Self { dir }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn join(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.path().join(rel)
    }
}

/// A sqlite database for tests.
pub struct TestDb {
    db_path: PathBuf,
    url: String,
}

impl TestDb {
    pub fn sqlite_file(temp_root: &TempRoot) -> Self {
        let db_path = temp_root.join("db.sqlite");
        let url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        Self { db_path, url }
    }

    pub fn sqlite_memory() -> Self {
        Self {
            db_path: PathBuf::new(),
            url: "sqlite::memory:".to_string(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.db_path
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

/// A common env setup for backend tests.
///
/// - Sets `VIBE_ASSET_DIR`, `VK_CONFIG_DIR`, `DATABASE_URL`
/// - Disables background tasks with `VIBE_DISABLE_BACKGROUND_TASKS=1`
///
/// Holds the global env lock for the lifetime of the guard.
pub struct TestEnvGuard {
    #[allow(dead_code)]
    env: EnvVarGuard,
    vk_config_dir: PathBuf,
}

impl TestEnvGuard {
    pub fn new(temp_root: &Path, db_url: String) -> Self {
        let vk_config_dir = temp_root.join("vk-config");
        std::fs::create_dir_all(&vk_config_dir).expect("create vk config dir");

        let mut env = EnvVarGuard::new();
        env.set_var("VIBE_ASSET_DIR", temp_root.as_os_str());
        env.set_var("DATABASE_URL", db_url);
        // Disable background workers (event outbox loop, orphan cleanup, etc.) to avoid
        // cross-task SQLite write contention in tests.
        env.set_var("VIBE_DISABLE_BACKGROUND_TASKS", "1");
        env.set_var("VK_CONFIG_DIR", vk_config_dir.as_os_str());

        Self { env, vk_config_dir }
    }

    pub fn vk_config_dir(&self) -> &Path {
        &self.vk_config_dir
    }
}

/// A convenient bundle for backend tests:
/// - unique `TempRoot`
/// - file-backed sqlite `TestDb`
/// - `TestEnvGuard` that sets common env vars
pub struct TestEnv {
    temp_root: TempRoot,
    db: TestDb,
    guard: TestEnvGuard,
}

impl TestEnv {
    pub fn new(prefix: &str) -> Self {
        let temp_root = TempRoot::new(prefix);
        let db = TestDb::sqlite_file(&temp_root);
        let guard = TestEnvGuard::new(temp_root.path(), db.url().to_string());

        Self {
            temp_root,
            db,
            guard,
        }
    }

    pub fn temp_root(&self) -> &TempRoot {
        &self.temp_root
    }

    pub fn db(&self) -> &TestDb {
        &self.db
    }

    pub fn guard(&self) -> &TestEnvGuard {
        &self.guard
    }
}
