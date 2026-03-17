use std::{env, fs, path::PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries = map.into_iter().collect::<Vec<_>>();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let mut out = serde_json::Map::new();
            for (key, value) in entries {
                out.insert(key, canonicalize_json(value));
            }
            Value::Object(out)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect()),
        other => other,
    }
}

fn parse_toml_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let stripped = trimmed.strip_prefix('"')?.strip_suffix('"')?;
    Some(stripped.to_string())
}

fn find_workspace_cargo_lock(start_dir: &PathBuf) -> Option<PathBuf> {
    for ancestor in start_dir.ancestors() {
        let candidate = ancestor.join("Cargo.lock");
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn parse_expected_codex_cli_version_from_lock(lock: &str) -> Option<String> {
    let mut current_name: Option<String> = None;
    let mut current_version: Option<String> = None;
    let mut current_source: Option<String> = None;

    let mut seen_matching_name = None;

    let flush = |name: &Option<String>,
                 version: &Option<String>,
                 source: &Option<String>|
     -> Option<String> {
        if name.as_deref() != Some("codex-app-server-protocol") {
            return None;
        }
        if source
            .as_deref()
            .is_some_and(|s| s.contains("git+https://github.com/openai/codex.git"))
        {
            return version.clone();
        }
        None
    };

    for line in lock.lines() {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            if let Some(version) = flush(&current_name, &current_version, &current_source) {
                return Some(version);
            }
            if current_name.as_deref() == Some("codex-app-server-protocol") {
                seen_matching_name = current_version.clone();
            }

            current_name = None;
            current_version = None;
            current_source = None;
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("name = ") {
            current_name = parse_toml_string(rest);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("version = ") {
            current_version = parse_toml_string(rest);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("source = ") {
            current_source = parse_toml_string(rest);
            continue;
        }
    }

    if let Some(version) = flush(&current_name, &current_version, &current_source) {
        return Some(version);
    }

    seen_matching_name
}

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let schema_dir = out_dir.join("vk_codex_app_server_protocol_schema");

    // Ensure a clean directory to avoid stale artifacts.
    let _ = fs::remove_dir_all(&schema_dir);
    fs::create_dir_all(&schema_dir).expect("create schema dir");

    codex_app_server_protocol::generate_json(&schema_dir)
        .expect("generate codex app-server protocol json schema");

    let bundle_path = schema_dir.join("codex_app_server_protocol.v2.schemas.json");
    let bytes = fs::read(&bundle_path).expect("read v2 schema bundle");
    let value: Value = serde_json::from_slice(&bytes).expect("parse v2 schema bundle json");
    let canonical_bytes =
        serde_json::to_vec(&canonicalize_json(value)).expect("serialize canonical v2 schema");
    let hash = format!("{:x}", Sha256::digest(&canonical_bytes));

    println!("cargo:rustc-env=VK_CODEX_EXPECTED_V2_SCHEMA_SHA256={hash}");

    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let manifest_dir = PathBuf::from(manifest_dir);
        if let Some(lock_path) = find_workspace_cargo_lock(&manifest_dir)
            && let Ok(lock) = fs::read_to_string(&lock_path)
            && let Some(version) = parse_expected_codex_cli_version_from_lock(&lock)
        {
            println!("cargo:rustc-env=VK_CODEX_EXPECTED_CODEX_CLI_VERSION={version}");
        }
    }
}
