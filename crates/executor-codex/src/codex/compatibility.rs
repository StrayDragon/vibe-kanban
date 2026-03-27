use std::{collections::HashMap, io, path::Path, sync::LazyLock};

use executors_core::{
    agent_command::{AgentCommandKey, agent_command_resolver, command_identity_for_agent},
    command::{CmdOverrides, CommandBuilder},
    executors::ExecutorError,
};
use executors_protocol::BaseCodingAgent;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use ts_rs::TS;

const V2_SCHEMA_BUNDLE_FILENAME: &str = "codex_app_server_protocol.v2.schemas.json";
const EXPECTED_V2_SCHEMA_SHA256: &str = env!("VK_CODEX_EXPECTED_V2_SCHEMA_SHA256");
const EXPECTED_CODEX_CLI_VERSION: Option<&str> = option_env!("VK_CODEX_EXPECTED_CODEX_CLI_VERSION");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CodexProtocolCompatibilityStatus {
    Compatible,
    Incompatible,
    NotInstalled,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
pub struct CodexProtocolCompatibility {
    pub status: CodexProtocolCompatibilityStatus,
    pub expected_v2_schema_sha256: String,
    pub runtime_v2_schema_sha256: Option<String>,
    pub codex_cli_version: Option<String>,
    pub base_command: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    program: std::path::PathBuf,
    base_args: Vec<String>,
    additional_params: Vec<String>,
    codex_cli_version: Option<String>,
}

#[derive(Default)]
struct CompatCache {
    entries: RwLock<HashMap<CacheKey, CodexProtocolCompatibility>>,
}

impl CompatCache {
    async fn get_or_compute<F, Fut>(
        &self,
        key: CacheKey,
        refresh: bool,
        compute: F,
    ) -> CodexProtocolCompatibility
    where
        F: FnOnce(CacheKey) -> Fut,
        Fut: std::future::Future<Output = CodexProtocolCompatibility>,
    {
        if !refresh && let Some(existing) = self.entries.read().await.get(&key).cloned() {
            return existing;
        }

        let computed = compute(key.clone()).await;
        self.entries.write().await.insert(key, computed.clone());
        computed
    }
}

static COMPAT_CACHE: LazyLock<CompatCache> = LazyLock::new(CompatCache::default);

pub async fn check_codex_protocol_compatibility(
    cmd: &CmdOverrides,
    oss_enabled: bool,
    refresh: bool,
) -> CodexProtocolCompatibility {
    let resolved = agent_command_resolver()
        .resolve_with_overrides(
            AgentCommandKey::Agent(BaseCodingAgent::Codex),
            command_identity_for_agent(BaseCodingAgent::Codex),
            cmd,
        )
        .await;

    let base_command = resolved.base_command;

    let additional_params = cmd.additional_params.clone().unwrap_or_default();

    let resolved_cli = match resolve_command(&base_command, oss_enabled).await {
        Ok(cli) => cli,
        Err(ExecutorError::ExecutableNotFound { .. }) => {
            return CodexProtocolCompatibility {
                status: CodexProtocolCompatibilityStatus::NotInstalled,
                expected_v2_schema_sha256: EXPECTED_V2_SCHEMA_SHA256.to_string(),
                runtime_v2_schema_sha256: None,
                codex_cli_version: None,
                base_command,
                message: None,
            };
        }
        Err(err) => {
            let raw_error = err.to_string();
            let mut compat = CodexProtocolCompatibility {
                status: CodexProtocolCompatibilityStatus::Unknown,
                expected_v2_schema_sha256: EXPECTED_V2_SCHEMA_SHA256.to_string(),
                runtime_v2_schema_sha256: None,
                codex_cli_version: None,
                base_command,
                message: None,
            };
            compat.message = Some(unknown_compatibility_error_message(
                &compat,
                Some(&raw_error),
            ));
            return compat;
        }
    };

    let codex_cli_version = match get_codex_version(&resolved_cli.0, &resolved_cli.1).await {
        Ok(version) => version,
        Err(err) => {
            tracing::debug!("failed to detect codex version: {err}");
            None
        }
    };

    let key = CacheKey {
        program: resolved_cli.0.clone(),
        base_args: resolved_cli.1.clone(),
        additional_params: additional_params.clone(),
        codex_cli_version: codex_cli_version.clone(),
    };

    COMPAT_CACHE
        .get_or_compute(key, refresh, |key| async move {
            match compute_compatibility(
                &key.program,
                &key.base_args,
                &key.additional_params,
                codex_cli_version.clone(),
                base_command.clone(),
            )
            .await
            {
                Ok(result) => result,
                Err(err) => {
                    let raw_error = err.to_string();
                    let mut compat = CodexProtocolCompatibility {
                        status: CodexProtocolCompatibilityStatus::Unknown,
                        expected_v2_schema_sha256: EXPECTED_V2_SCHEMA_SHA256.to_string(),
                        runtime_v2_schema_sha256: None,
                        codex_cli_version,
                        base_command: base_command.clone(),
                        message: None,
                    };
                    compat.message = Some(unknown_compatibility_error_message(
                        &compat,
                        Some(&raw_error),
                    ));
                    compat
                }
            }
        })
        .await
}

async fn compute_compatibility(
    program: &std::path::Path,
    base_args: &[String],
    additional_params: &[String],
    codex_cli_version: Option<String>,
    base_command: String,
) -> Result<CodexProtocolCompatibility, ExecutorError> {
    let runtime_v2_schema_sha256 =
        run_schema_fingerprint(program, base_args, additional_params).await?;

    let mut compat = CodexProtocolCompatibility {
        status: CodexProtocolCompatibilityStatus::Compatible,
        expected_v2_schema_sha256: EXPECTED_V2_SCHEMA_SHA256.to_string(),
        runtime_v2_schema_sha256: Some(runtime_v2_schema_sha256),
        codex_cli_version,
        base_command,
        message: None,
    };

    let runtime = compat
        .runtime_v2_schema_sha256
        .as_deref()
        .unwrap_or_default();
    if runtime != EXPECTED_V2_SCHEMA_SHA256 {
        if should_allow_patch_drift(
            EXPECTED_CODEX_CLI_VERSION,
            compat.codex_cli_version.as_deref(),
        ) {
            compat.message = Some(compatibility_patch_drift_warning_message(&compat));
        } else {
            compat.status = CodexProtocolCompatibilityStatus::Incompatible;
            compat.message = Some(compatibility_error_message(&compat));
        }
    }

    Ok(compat)
}

async fn get_codex_version(
    program: &std::path::Path,
    args: &[String],
) -> Result<Option<String>, ExecutorError> {
    let output = tokio::process::Command::new(program)
        .args(args)
        .arg("--version")
        .output()
        .await
        .map_err(ExecutorError::Io)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ExecutorError::Io(io::Error::other(format!(
            "codex --version failed: {stderr}"
        ))));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(extract_first_semver(&stdout))
}

fn extract_first_semver(text: &str) -> Option<String> {
    static RE: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"(?P<v>\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?)").expect("semver regex")
    });

    RE.captures(text)
        .and_then(|caps| caps.name("v"))
        .map(|m| m.as_str().to_string())
}

async fn resolve_command(
    base_command: &str,
    oss_enabled: bool,
) -> Result<(std::path::PathBuf, Vec<String>), ExecutorError> {
    let mut builder = CommandBuilder::new(base_command);
    if oss_enabled {
        builder = builder.extend_params(["--oss"]);
    }
    let parts = builder.build_initial()?;
    let (program, args) = parts.into_resolved().await?;
    Ok((program, args))
}

async fn run_schema_fingerprint(
    program: &std::path::Path,
    base_args: &[String],
    additional_params: &[String],
) -> Result<String, ExecutorError> {
    let schema_dir = tempfile::tempdir().map_err(|err| ExecutorError::Io(io::Error::other(err)))?;
    let out_dir = schema_dir.path().to_string_lossy().to_string();

    let output = tokio::process::Command::new(program)
        .args(base_args)
        .arg("app-server")
        .args(additional_params)
        .args(["generate-json-schema", "--out"])
        .arg(out_dir)
        .output()
        .await
        .map_err(ExecutorError::Io)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(ExecutorError::Io(io::Error::other(format!(
            "codex app-server generate-json-schema failed (exit={:?}): stdout={stdout} stderr={stderr}",
            output.status.code()
        ))));
    }

    let bundle_path = schema_dir.path().join(V2_SCHEMA_BUNDLE_FILENAME);
    let bytes = tokio::fs::read(&bundle_path)
        .await
        .map_err(|err| ExecutorError::Io(io::Error::other(format!("{bundle_path:?}: {err}"))))?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|err| {
        ExecutorError::Io(io::Error::other(format!(
            "failed to parse {bundle_path:?} as json: {err}"
        )))
    })?;
    let canonical_bytes = serde_json::to_vec(&canonicalize_json(value)).map_err(|err| {
        ExecutorError::Io(io::Error::other(format!(
            "failed to serialize canonical schema json: {err}"
        )))
    })?;
    Ok(format!("{:x}", Sha256::digest(&canonical_bytes)))
}

pub fn compatibility_error_message(compat: &CodexProtocolCompatibility) -> String {
    let mut lines = Vec::new();
    lines.push("Codex protocol is incompatible with this Vibe Kanban build.".to_string());
    lines.push(format!("Base command: {}", compat.base_command));
    if let Some(version) = compat.codex_cli_version.as_deref() {
        lines.push(format!("Detected codex-cli version: {version}"));
    }
    if let Some(version) = EXPECTED_CODEX_CLI_VERSION {
        lines.push(format!("Expected codex-cli version: {version}"));
        if let Some(runtime) = compat.codex_cli_version.as_deref()
            && let Some(ordering) = compare_semver_core(runtime, version)
        {
            match ordering {
                std::cmp::Ordering::Less => lines.push(
                    "Detected codex-cli is older than expected (upgrade codex-cli).".to_string(),
                ),
                std::cmp::Ordering::Greater => lines.push(
                    "Detected codex-cli is newer than expected (upgrade Vibe Kanban or downgrade codex-cli)."
                        .to_string(),
                ),
                std::cmp::Ordering::Equal => {}
            }
        }
    }
    if let Some(runtime) = compat.runtime_v2_schema_sha256.as_deref() {
        lines.push(format!("Runtime protocol fingerprint: {runtime}"));
    }
    lines.push(format!(
        "Expected protocol fingerprint: {}",
        compat.expected_v2_schema_sha256
    ));
    lines.push(
        "Fix: upgrade Vibe Kanban, or align your local codex-cli to a compatible version."
            .to_string(),
    );
    lines.join("\n")
}

fn unknown_compatibility_error_message(
    compat: &CodexProtocolCompatibility,
    raw_error: Option<&str>,
) -> String {
    let mut lines = Vec::new();
    lines.push("Unable to verify Codex protocol compatibility (status: unknown).".to_string());
    lines.push("Failing fast to avoid runtime decode crashes.".to_string());
    lines.push(format!("Base command: {}", compat.base_command));
    if let Some(version) = compat.codex_cli_version.as_deref() {
        lines.push(format!("Detected codex-cli version: {version}"));
    }
    if let Some(version) = EXPECTED_CODEX_CLI_VERSION {
        lines.push(format!("Expected codex-cli version: {version}"));
    }
    if let Some(runtime) = compat.runtime_v2_schema_sha256.as_deref() {
        lines.push(format!("Runtime protocol fingerprint: {runtime}"));
    }
    lines.push(format!(
        "Expected protocol fingerprint: {}",
        compat.expected_v2_schema_sha256
    ));
    if let Some(message) = raw_error {
        lines.push(format!("Error: {message}"));
    }
    lines.push(
        "Fix: upgrade Vibe Kanban, or align your local codex-cli to a compatible version."
            .to_string(),
    );
    lines.join("\n")
}

pub fn compatibility_blocking_error_message(compat: &CodexProtocolCompatibility) -> String {
    match compat.status {
        CodexProtocolCompatibilityStatus::Compatible => "Codex protocol is compatible.".to_string(),
        CodexProtocolCompatibilityStatus::Incompatible => compatibility_error_message(compat),
        CodexProtocolCompatibilityStatus::NotInstalled => format!(
            "Codex CLI is not installed or not available in PATH (base command: {}).",
            compat.base_command
        ),
        CodexProtocolCompatibilityStatus::Unknown => compat
            .message
            .clone()
            .unwrap_or_else(|| unknown_compatibility_error_message(compat, None)),
    }
}

pub fn v2_schema_bundle_path(dir: &Path) -> std::path::PathBuf {
    dir.join(V2_SCHEMA_BUNDLE_FILENAME)
}

fn should_allow_patch_drift(expected: Option<&str>, runtime: Option<&str>) -> bool {
    let Some(expected) = expected else {
        return false;
    };
    let Some(runtime) = runtime else {
        return false;
    };

    // Pre-release versions are allowed to drift rapidly and may not be
    // semver-compatible with the stable channel. Stay strict unless the
    // fingerprints match.
    if expected.contains('-') || runtime.contains('-') {
        return false;
    }

    let Some(expected_mm) = semver_major_minor(expected) else {
        return false;
    };
    let Some(runtime_mm) = semver_major_minor(runtime) else {
        return false;
    };

    expected_mm == runtime_mm
}

fn semver_major_minor(version: &str) -> Option<(u64, u64)> {
    let core = version.split(['-', '+']).next().unwrap_or(version);
    let mut parts = core.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    let _patch = parts.next()?.parse::<u64>().ok()?;
    Some((major, minor))
}

fn compare_semver_core(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let parse = |v: &str| -> Option<(u64, u64, u64)> {
        let core = v.split(['-', '+']).next().unwrap_or(v);
        let mut parts = core.split('.');
        let major = parts.next()?.parse::<u64>().ok()?;
        let minor = parts.next()?.parse::<u64>().ok()?;
        let patch = parts.next()?.parse::<u64>().ok()?;
        Some((major, minor, patch))
    };

    let a = parse(a)?;
    let b = parse(b)?;
    Some(a.cmp(&b))
}

fn compatibility_patch_drift_warning_message(compat: &CodexProtocolCompatibility) -> String {
    let mut lines = Vec::new();
    lines.push(
        "Codex protocol fingerprint differs from this Vibe Kanban build, but continuing because the codex-cli MAJOR.MINOR matches."
            .to_string(),
    );
    lines.push(format!("Base command: {}", compat.base_command));
    if let Some(version) = compat.codex_cli_version.as_deref() {
        lines.push(format!("Detected codex-cli version: {version}"));
    }
    if let Some(version) = EXPECTED_CODEX_CLI_VERSION {
        lines.push(format!("Expected codex-cli version: {version}"));
    }
    if let Some(runtime) = compat.runtime_v2_schema_sha256.as_deref() {
        lines.push(format!("Runtime protocol fingerprint: {runtime}"));
    }
    lines.push(format!(
        "Expected protocol fingerprint: {}",
        compat.expected_v2_schema_sha256
    ));
    lines.push(
        "Note: if you encounter protocol decode errors, align Vibe Kanban and codex-cli versions."
            .to_string(),
    );
    lines.join("\n")
}

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

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;

    #[tokio::test]
    async fn cache_reuses_value_when_key_matches() {
        let cache = CompatCache::default();
        let key = CacheKey {
            program: std::path::PathBuf::from("/bin/codex"),
            base_args: vec![],
            additional_params: vec![],
            codex_cli_version: Some("0.0.1".to_string()),
        };
        let calls = Arc::new(AtomicUsize::new(0));

        let compute = |calls: Arc<AtomicUsize>| {
            move |key: CacheKey| {
                let calls = calls.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    CodexProtocolCompatibility {
                        status: CodexProtocolCompatibilityStatus::Compatible,
                        expected_v2_schema_sha256: "expected".to_string(),
                        runtime_v2_schema_sha256: Some("runtime".to_string()),
                        codex_cli_version: key.codex_cli_version.clone(),
                        base_command: key.program.to_string_lossy().to_string(),
                        message: None,
                    }
                }
            }
        };

        let _ = cache
            .get_or_compute(key.clone(), false, compute(calls.clone()))
            .await;
        let _ = cache
            .get_or_compute(key.clone(), false, compute(calls.clone()))
            .await;

        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn cache_misses_when_key_changes() {
        let cache = CompatCache::default();
        let calls = Arc::new(AtomicUsize::new(0));

        let compute = |calls: Arc<AtomicUsize>| {
            move |key: CacheKey| {
                let calls = calls.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    CodexProtocolCompatibility {
                        status: CodexProtocolCompatibilityStatus::Compatible,
                        expected_v2_schema_sha256: "expected".to_string(),
                        runtime_v2_schema_sha256: Some("runtime".to_string()),
                        codex_cli_version: key.codex_cli_version.clone(),
                        base_command: key.program.to_string_lossy().to_string(),
                        message: None,
                    }
                }
            }
        };

        let key1 = CacheKey {
            program: std::path::PathBuf::from("/bin/codex"),
            base_args: vec![],
            additional_params: vec![],
            codex_cli_version: Some("0.0.1".to_string()),
        };
        let key2 = CacheKey {
            program: std::path::PathBuf::from("/bin/codex"),
            base_args: vec![],
            additional_params: vec![],
            codex_cli_version: Some("0.0.2".to_string()),
        };

        let _ = cache
            .get_or_compute(key1, false, compute(calls.clone()))
            .await;
        let _ = cache
            .get_or_compute(key2, false, compute(calls.clone()))
            .await;

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn extract_first_semver_finds_version() {
        assert_eq!(
            extract_first_semver("codex-cli 0.114.0"),
            Some("0.114.0".to_string())
        );
    }

    #[test]
    fn should_allow_patch_drift_when_major_minor_match() {
        assert!(should_allow_patch_drift(Some("0.115.0"), Some("0.115.1")));
        assert!(!should_allow_patch_drift(
            Some("0.115.0"),
            Some("0.115.0-alpha.1")
        ));
        assert!(!should_allow_patch_drift(Some("0.115.0"), Some("0.116.0")));
        assert!(!should_allow_patch_drift(Some("0.115.0"), Some("1.115.0")));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn missing_schema_bundle_reports_unknown_with_diagnostics() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("tempdir");
        let fake_codex = dir.path().join("codex");

        std::fs::write(
            &fake_codex,
            r#"#!/bin/sh
set -eu

if [ "${1:-}" = "--version" ]; then
  echo "codex-cli 0.0.0-test"
  exit 0
fi

if [ "${1:-}" = "--oss" ]; then
  shift
fi

if [ "${1:-}" = "app-server" ] && [ "${2:-}" = "generate-json-schema" ]; then
  out=""
  while [ "$#" -gt 0 ]; do
    if [ "$1" = "--out" ]; then
      out="$2"
      shift 2
      continue
    fi
    shift
  done
  mkdir -p "$out"
  # Intentionally do NOT write the v2 schema bundle file to simulate drift.
  exit 0
fi

echo "unexpected args: $*" >&2
exit 1
"#,
        )
        .expect("write fake codex");

        let mut perms = std::fs::metadata(&fake_codex)
            .expect("metadata")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&fake_codex, perms).expect("chmod");

        let overrides = CmdOverrides {
            base_command_override: Some(fake_codex.to_string_lossy().to_string()),
            additional_params: None,
            env: None,
        };

        let compat = check_codex_protocol_compatibility(&overrides, false, true).await;
        assert_eq!(compat.status, CodexProtocolCompatibilityStatus::Unknown);

        let msg = compat.message.expect("diagnostics message");
        assert!(msg.contains(V2_SCHEMA_BUNDLE_FILENAME));
    }
}
