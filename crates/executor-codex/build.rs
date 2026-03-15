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
}
