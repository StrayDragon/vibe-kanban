use anyhow::Result;

fn print_help() {
    println!(
        r#"vk config schema

Usage:
  vk config schema upsert
"#
    );
}

pub async fn run(args: Vec<String>) -> Result<()> {
    if args.is_empty() {
        print_help();
        return Ok(());
    }

    let first = args[0].as_str();
    if matches!(first, "--help" | "-h" | "help") {
        print_help();
        return Ok(());
    }

    match first {
        "upsert" => run_upsert(),
        other => anyhow::bail!(
            "Unknown vk config schema command: {other}. Run `vk config schema --help`."
        ),
    }
}

fn run_upsert() -> Result<()> {
    let config_schema_path = utils_core::vk_config_schema_path();
    let projects_schema_path = utils_core::vk_projects_schema_path();

    config::write_config_schema_json(&config_schema_path)?;
    config::write_projects_schema_json(&projects_schema_path)?;

    println!(
        "Wrote schemas:\n- {}\n- {}",
        config_schema_path.display(),
        projects_schema_path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
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
        fn set(key: &'static str, value: Option<&std::path::Path>) -> Self {
            let prev = std::env::var_os(key);
            // SAFETY: tests using EnvVarGuard are serialized by env_lock().
            unsafe {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
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

    struct EnvStrGuard {
        key: &'static str,
        prev: Option<std::ffi::OsString>,
    }

    impl EnvStrGuard {
        fn set(key: &'static str, value: Option<&str>) -> Self {
            let prev = std::env::var_os(key);
            // SAFETY: tests using EnvStrGuard are serialized by env_lock().
            unsafe {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
            Self { key, prev }
        }
    }

    impl Drop for EnvStrGuard {
        fn drop(&mut self) {
            // SAFETY: tests using EnvStrGuard are serialized by env_lock().
            unsafe {
                match &self.prev {
                    Some(value) => std::env::set_var(self.key, value),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }

    #[test]
    fn schema_upsert_writes_json_files_without_leaking_secrets() {
        let _guard = env_lock().lock().unwrap();

        let dir = std::env::temp_dir().join(format!("vk-schema-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let _vk_config_dir = EnvVarGuard::set("VK_CONFIG_DIR", Some(&dir));

        // Run schema generation with a known secret in the process environment to ensure the
        // output is independent of runtime config/env.
        let _github_pat = EnvStrGuard::set("GITHUB_PAT", Some("sekrit"));

        run_upsert().expect("schema upsert should succeed");

        let config_schema_path = utils_core::vk_config_schema_path();
        let projects_schema_path = utils_core::vk_projects_schema_path();

        assert!(config_schema_path.exists());
        assert!(projects_schema_path.exists());

        let config_schema_raw = std::fs::read_to_string(&config_schema_path).unwrap();
        let projects_schema_raw = std::fs::read_to_string(&projects_schema_path).unwrap();

        serde_json::from_str::<serde_json::Value>(&config_schema_raw)
            .expect("config schema should be valid JSON");
        serde_json::from_str::<serde_json::Value>(&projects_schema_raw)
            .expect("projects schema should be valid JSON");

        assert!(!config_schema_raw.contains("sekrit"));
        assert!(!projects_schema_raw.contains("sekrit"));
    }
}
