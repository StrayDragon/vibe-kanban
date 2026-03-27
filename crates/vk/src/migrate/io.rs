use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;

pub fn timestamp_slug() -> String {
    Utc::now().format("%Y%m%dT%H%M%SZ").to_string()
}

pub fn sqlite_path_from_url(database_url: &str) -> Option<std::path::PathBuf> {
    let trimmed = database_url.trim();
    if !trimmed.starts_with("sqlite:") {
        return None;
    }
    let mut rest = &trimmed["sqlite:".len()..];
    if rest.starts_with("//") {
        rest = &rest[2..];
    }
    let path_part = rest.split('?').next().unwrap_or(rest);
    if path_part.is_empty() || path_part == ":memory:" {
        return None;
    }
    Some(std::path::PathBuf::from(path_part))
}

pub fn atomic_write_out(path: &Path, contents: &str) -> Result<()> {
    atomic_write_out_with_unix_mode(path, contents, None)
}

pub fn atomic_write_out_secure(path: &Path, contents: &str) -> Result<()> {
    atomic_write_out_with_unix_mode(path, contents, Some(0o600))
}

fn atomic_write_out_with_unix_mode(
    path: &Path,
    contents: &str,
    unix_mode: Option<u32>,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create output directory {}",
                parent.to_string_lossy()
            )
        })?;
    }

    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "vk-export.yaml".to_string());
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let tmp_path = path.with_file_name(format!("{file_name}.tmp-{}-{unique}", std::process::id()));

    #[cfg(unix)]
    if let Some(mode) = unix_mode {
        use std::{io::Write, os::unix::fs::OpenOptionsExt};

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&tmp_path)
            .with_context(|| {
                format!(
                    "Failed to create temporary output file {}",
                    tmp_path.to_string_lossy()
                )
            })?;
        file.write_all(contents.as_bytes()).with_context(|| {
            format!(
                "Failed to write temporary output file {}",
                tmp_path.to_string_lossy()
            )
        })?;
    } else {
        std::fs::write(&tmp_path, contents).with_context(|| {
            format!(
                "Failed to write temporary output file {}",
                tmp_path.to_string_lossy()
            )
        })?;
    }

    #[cfg(not(unix))]
    {
        let _ = unix_mode;
        std::fs::write(&tmp_path, contents).with_context(|| {
            format!(
                "Failed to write temporary output file {}",
                tmp_path.to_string_lossy()
            )
        })?;
    }

    if let Err(err) = std::fs::rename(&tmp_path, path) {
        if path.exists() {
            let _ = std::fs::remove_file(path);
            std::fs::rename(&tmp_path, path).with_context(|| {
                format!("Failed to replace output file {}", path.to_string_lossy())
            })?;
        } else {
            return Err(err.into());
        }
    }

    #[cfg(unix)]
    if let Some(mode) = unix_mode {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn atomic_write_out_secure_sets_mode_0600() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("vk-io-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("secret.env.migrated.test");

        atomic_write_out_secure(&path, "FOO=bar\n").unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
