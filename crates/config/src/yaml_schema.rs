use std::path::Path;

use schemars::{Schema, SchemaGenerator, generate::SchemaSettings};
use thiserror::Error;

use crate::{Config, ProjectsFile};

#[derive(Debug, Error)]
pub enum ConfigSchemaError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn generate_config_schema_json() -> Result<String, ConfigSchemaError> {
    // Draft-07, inline everything (no $defs)
    let mut settings = SchemaSettings::draft07();
    settings.inline_subschemas = true;

    let generator: SchemaGenerator = settings.into_generator();
    let schema: Schema = generator.into_root_schema_for::<Config>();

    let mut schema_value: serde_json::Value = serde_json::to_value(&schema)?;
    if let Some(obj) = schema_value.as_object_mut() {
        obj.remove("title");
    }

    Ok(serde_json::to_string_pretty(&schema_value)?)
}

pub fn generate_projects_schema_json() -> Result<String, ConfigSchemaError> {
    // Draft-07, inline everything (no $defs)
    let mut settings = SchemaSettings::draft07();
    settings.inline_subschemas = true;

    let generator: SchemaGenerator = settings.into_generator();
    let schema: Schema = generator.into_root_schema_for::<ProjectsFile>();

    let mut schema_value: serde_json::Value = serde_json::to_value(&schema)?;
    if let Some(obj) = schema_value.as_object_mut() {
        obj.remove("title");
    }

    Ok(serde_json::to_string_pretty(&schema_value)?)
}

pub fn write_config_schema_json(path: &Path) -> Result<(), ConfigSchemaError> {
    let content = generate_config_schema_json()?;

    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "config.schema.json path has no parent directory",
        )
    })?;
    std::fs::create_dir_all(parent)?;

    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "config.schema.json".to_string());
    let tmp_path = path.with_file_name(format!("{file_name}.tmp-{}", std::process::id()));

    std::fs::write(&tmp_path, content)?;

    if let Err(err) = std::fs::rename(&tmp_path, path) {
        if path.exists() {
            let _ = std::fs::remove_file(path);
            std::fs::rename(&tmp_path, path)?;
            return Ok(());
        }
        return Err(err.into());
    }

    Ok(())
}

pub fn write_projects_schema_json(path: &Path) -> Result<(), ConfigSchemaError> {
    let content = generate_projects_schema_json()?;

    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "projects.schema.json path has no parent directory",
        )
    })?;
    std::fs::create_dir_all(parent)?;

    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "projects.schema.json".to_string());
    let tmp_path = path.with_file_name(format!("{file_name}.tmp-{}", std::process::id()));

    std::fs::write(&tmp_path, content)?;

    if let Err(err) = std::fs::rename(&tmp_path, path) {
        if path.exists() {
            let _ = std::fs::remove_file(path);
            std::fs::rename(&tmp_path, path)?;
            return Ok(());
        }
        return Err(err.into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use test_support::TempRoot;

    use super::*;

    #[test]
    fn schema_generation_produces_valid_json() {
        let raw = generate_config_schema_json().expect("schema generation");
        serde_json::from_str::<serde_json::Value>(&raw).expect("schema json parse");
    }

    #[test]
    fn schema_includes_field_descriptions() {
        let raw = generate_config_schema_json().expect("schema generation");
        let value: serde_json::Value = serde_json::from_str(&raw).expect("schema json parse");

        let git_desc = value
            .pointer("/properties/git_no_verify/description")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert!(git_desc.contains("git hooks"));

        let pat_desc = value
            .pointer("/properties/github/properties/pat/description")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert!(pat_desc.contains("secret.env"));
    }

    #[test]
    fn schema_write_creates_file() {
        let temp_root = TempRoot::new("vk-schema-test-");
        let path = temp_root.join("config.schema.json");
        write_config_schema_json(&path).expect("write schema");

        assert!(path.is_file());
    }

    #[test]
    fn projects_schema_write_creates_file() {
        let temp_root = TempRoot::new("vk-projects-schema-test-");
        let path = temp_root.join("projects.schema.json");
        write_projects_schema_json(&path).expect("write schema");

        assert!(path.is_file());
    }
}
