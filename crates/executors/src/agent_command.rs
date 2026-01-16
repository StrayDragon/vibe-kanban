use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::{
    process::Command,
    sync::{Mutex, Notify, RwLock},
};
use ts_rs::TS;
use workspace_utils::shell::resolve_executable_path;

use crate::{
    command::CmdOverrides,
    executors::BaseCodingAgent,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(use_ts_enum)]
pub enum AgentCommandSource {
    PnpmGlobal,
    NpmGlobal,
    NpxLatest,
    SystemBinary,
    Override,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(use_ts_enum)]
pub enum AgentCommandStatus {
    Checking,
    Ready,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
pub struct AgentCommandResolution {
    pub source: AgentCommandSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub status: AgentCommandStatus,
    #[serde(default)]
    pub fallback_to_latest: bool,
}

impl AgentCommandResolution {
    fn checking() -> Self {
        Self {
            source: AgentCommandSource::Unknown,
            version: None,
            status: AgentCommandStatus::Checking,
            fallback_to_latest: false,
        }
    }

    fn ready(resolved: &ResolvedCommand) -> Self {
        Self {
            source: resolved.source.clone(),
            version: resolved.version.clone(),
            status: AgentCommandStatus::Ready,
            fallback_to_latest: resolved.fallback_to_latest,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedCommand {
    pub base_command: String,
    pub source: AgentCommandSource,
    pub version: Option<String>,
    pub fallback_to_latest: bool,
}

impl ResolvedCommand {
    fn override_command(command: &str) -> Self {
        Self {
            base_command: command.to_string(),
            source: AgentCommandSource::Override,
            version: None,
            fallback_to_latest: false,
        }
    }

    fn system_binary(binary: &str, base_args: &[&str]) -> Self {
        Self {
            base_command: build_command_string(binary, base_args),
            source: AgentCommandSource::SystemBinary,
            version: None,
            fallback_to_latest: false,
        }
    }

    fn from_binary(
        source: AgentCommandSource,
        version: String,
        binary_path: &Path,
        base_args: &[&str],
    ) -> Self {
        let base_command = build_command_string(&binary_path.to_string_lossy(), base_args);
        Self {
            base_command,
            source,
            version: Some(version),
            fallback_to_latest: false,
        }
    }

    fn npx_latest(package: &str, base_args: &[&str]) -> Self {
        let mut args = Vec::with_capacity(base_args.len() + 2);
        args.push("-y".to_string());
        args.push(format!("{}@latest", package));
        args.extend(base_args.iter().map(|arg| (*arg).to_string()));
        Self {
            base_command: build_command_string_owned("npx", &args),
            source: AgentCommandSource::NpxLatest,
            version: None,
            fallback_to_latest: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum AgentCommandKey {
    Agent(BaseCodingAgent),
    ClaudeRouter,
}

#[derive(Debug, Clone, Copy)]
pub enum CommandIdentity {
    NodePackage {
        package: &'static str,
        preferred_binary: Option<&'static str>,
        base_args: &'static [&'static str],
    },
    SystemBinary {
        binary: &'static str,
        base_args: &'static [&'static str],
    },
}

pub fn command_identity_for_agent(agent: BaseCodingAgent) -> CommandIdentity {
    match agent {
        BaseCodingAgent::ClaudeCode => CommandIdentity::NodePackage {
            package: "@anthropic-ai/claude-code",
            preferred_binary: Some("claude"),
            base_args: &[],
        },
        BaseCodingAgent::Amp => CommandIdentity::NodePackage {
            package: "@sourcegraph/amp",
            preferred_binary: Some("amp"),
            base_args: &[],
        },
        BaseCodingAgent::Gemini => CommandIdentity::NodePackage {
            package: "@google/gemini-cli",
            preferred_binary: Some("gemini"),
            base_args: &[],
        },
        BaseCodingAgent::Codex => CommandIdentity::NodePackage {
            package: "@openai/codex",
            preferred_binary: Some("codex"),
            base_args: &[],
        },
        BaseCodingAgent::Opencode => CommandIdentity::NodePackage {
            package: "opencode-ai",
            preferred_binary: Some("opencode"),
            base_args: &[],
        },
        BaseCodingAgent::QwenCode => CommandIdentity::NodePackage {
            package: "@qwen-code/qwen-code",
            preferred_binary: Some("qwen"),
            base_args: &[],
        },
        BaseCodingAgent::Copilot => CommandIdentity::NodePackage {
            package: "@github/copilot",
            preferred_binary: Some("copilot"),
            base_args: &[],
        },
        BaseCodingAgent::CursorAgent => CommandIdentity::SystemBinary {
            binary: "cursor-agent",
            base_args: &[],
        },
        BaseCodingAgent::Droid => CommandIdentity::SystemBinary {
            binary: "droid",
            base_args: &["exec"],
        },
        BaseCodingAgent::FakeAgent => CommandIdentity::SystemBinary {
            binary: "fake-agent",
            base_args: &[],
        },
    }
}

pub fn claude_router_identity() -> CommandIdentity {
    CommandIdentity::NodePackage {
        package: "@musistudio/claude-code-router",
        preferred_binary: Some("claude-code-router"),
        base_args: &["code"],
    }
}

static AGENT_COMMAND_RESOLVER: LazyLock<AgentCommandResolver> =
    LazyLock::new(AgentCommandResolver::new);

pub fn agent_command_resolver() -> &'static AgentCommandResolver {
    &AGENT_COMMAND_RESOLVER
}

#[derive(Clone)]
pub struct AgentCommandResolver {
    state: Arc<ResolverState>,
}

struct ResolverState {
    cache: RwLock<HashMap<AgentCommandKey, ResolvedCommand>>,
    in_flight: Mutex<HashSet<AgentCommandKey>>,
    notifiers: Mutex<HashMap<AgentCommandKey, Arc<Notify>>>,
    global_packages: RwLock<Option<GlobalPackages>>,
}

impl AgentCommandResolver {
    fn new() -> Self {
        Self {
            state: Arc::new(ResolverState {
                cache: RwLock::new(HashMap::new()),
                in_flight: Mutex::new(HashSet::new()),
                notifiers: Mutex::new(HashMap::new()),
                global_packages: RwLock::new(None),
            }),
        }
    }

    pub fn warm_cache(&self) {
        for agent in all_agents() {
            let resolver = self.clone();
            tokio::spawn(async move {
                let key = AgentCommandKey::Agent(agent);
                let identity = command_identity_for_agent(agent);
                let _ = resolver.resolve_cached(key, identity).await;
            });
        }
    }

    pub async fn resolve_with_overrides(
        &self,
        key: AgentCommandKey,
        identity: CommandIdentity,
        overrides: &CmdOverrides,
    ) -> ResolvedCommand {
        if let Some(base) = overrides.base_command_override.as_ref() {
            return ResolvedCommand::override_command(base);
        }

        self.resolve_cached(key, identity).await
    }

    pub async fn snapshot(&self) -> HashMap<String, AgentCommandResolution> {
        let cache = self.state.cache.read().await;
        let mut map = HashMap::new();
        for agent in all_agents() {
            let key = AgentCommandKey::Agent(agent);
            let resolution = cache
                .get(&key)
                .map(AgentCommandResolution::ready)
                .unwrap_or_else(AgentCommandResolution::checking);
            map.insert(agent.to_string(), resolution);
        }
        map
    }

    async fn resolve_cached(
        &self,
        key: AgentCommandKey,
        identity: CommandIdentity,
    ) -> ResolvedCommand {
        if let Some(existing) = self.state.cache.read().await.get(&key).cloned() {
            return existing;
        }

        let notify = {
            let mut notifiers = self.state.notifiers.lock().await;
            notifiers
                .entry(key)
                .or_insert_with(|| Arc::new(Notify::new()))
                .clone()
        };

        let should_resolve = {
            let mut in_flight = self.state.in_flight.lock().await;
            in_flight.insert(key)
        };

        if should_resolve {
            let resolved = self.resolve_identity(identity).await;
            self.state.cache.write().await.insert(key, resolved.clone());
            let mut in_flight = self.state.in_flight.lock().await;
            in_flight.remove(&key);
            notify.notify_waiters();
            return resolved;
        }

        notify.notified().await;
        if let Some(existing) = self.state.cache.read().await.get(&key).cloned() {
            return existing;
        }

        self.resolve_identity(identity).await
    }

    async fn resolve_identity(&self, identity: CommandIdentity) -> ResolvedCommand {
        match identity {
            CommandIdentity::SystemBinary { binary, base_args } => {
                ResolvedCommand::system_binary(binary, base_args)
            }
            CommandIdentity::NodePackage {
                package,
                preferred_binary,
                base_args,
            } => {
                let global_packages = self.load_global_packages().await;
                resolve_node_package(
                    package,
                    preferred_binary,
                    base_args,
                    global_packages.pnpm.as_ref(),
                    global_packages.npm.as_ref(),
                )
                .await
            }
        }
    }

    async fn load_global_packages(&self) -> GlobalPackages {
        if let Some(existing) = self.state.global_packages.read().await.clone() {
            return existing;
        }

        let mut write = self.state.global_packages.write().await;
        if let Some(existing) = write.clone() {
            return existing;
        }

        let loaded = GlobalPackages::load().await;
        *write = Some(loaded.clone());
        loaded
    }
}

#[derive(Debug, Clone, Default)]
struct GlobalPackages {
    pnpm: Option<GlobalPackageSnapshot>,
    npm: Option<GlobalPackageSnapshot>,
}

impl GlobalPackages {
    async fn load() -> Self {
        let pnpm = load_pnpm_global_packages().await;
        let npm = load_npm_global_packages().await;
        Self { pnpm, npm }
    }
}

#[derive(Debug, Clone)]
struct GlobalPackageSnapshot {
    packages: HashMap<String, GlobalPackageEntry>,
    root_path: Option<PathBuf>,
    bin_dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct GlobalPackageEntry {
    version: String,
    path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct ResolvedBinary {
    path: PathBuf,
    version: String,
    source: AgentCommandSource,
}

async fn resolve_node_package(
    package: &str,
    preferred_binary: Option<&str>,
    base_args: &[&str],
    pnpm: Option<&GlobalPackageSnapshot>,
    npm: Option<&GlobalPackageSnapshot>,
) -> ResolvedCommand {
    let pnpm_binary = resolve_global_binary(
        package,
        preferred_binary,
        pnpm,
        AgentCommandSource::PnpmGlobal,
    )
    .await;
    let npm_binary = resolve_global_binary(
        package,
        preferred_binary,
        npm,
        AgentCommandSource::NpmGlobal,
    )
    .await;

    select_resolution(package, base_args, pnpm_binary, npm_binary)
}

fn select_resolution(
    package: &str,
    base_args: &[&str],
    pnpm_binary: Option<ResolvedBinary>,
    npm_binary: Option<ResolvedBinary>,
) -> ResolvedCommand {
    if let Some(binary) = pnpm_binary {
        return ResolvedCommand::from_binary(
            binary.source,
            binary.version,
            &binary.path,
            base_args,
        );
    }

    if let Some(binary) = npm_binary {
        return ResolvedCommand::from_binary(
            binary.source,
            binary.version,
            &binary.path,
            base_args,
        );
    }

    ResolvedCommand::npx_latest(package, base_args)
}

async fn resolve_global_binary(
    package: &str,
    preferred_binary: Option<&str>,
    snapshot: Option<&GlobalPackageSnapshot>,
    source: AgentCommandSource,
) -> Option<ResolvedBinary> {
    let snapshot = snapshot?;
    let entry = snapshot.packages.get(package)?;
    let bin_dir = snapshot.bin_dir.as_deref();
    let package_path = resolve_package_path(snapshot, package, entry);
    let candidates = binary_candidates(package, preferred_binary, package_path.as_deref()).await;

    for candidate in candidates {
        if let Some(path) = resolve_binary_path(&candidate, bin_dir) {
            return Some(ResolvedBinary {
                path,
                version: entry.version.clone(),
                source: source.clone(),
            });
        }
    }

    None
}

async fn binary_candidates(
    package: &str,
    preferred_binary: Option<&str>,
    package_path: Option<&Path>,
) -> Vec<String> {
    let mut candidates = Vec::new();

    if let Some(preferred) = preferred_binary {
        candidates.push(preferred.to_string());
    }

    if let Some(bin_names) = match package_path {
        Some(package_path) => read_package_bin_names(package, package_path).await,
        None => None,
    } {
        for name in bin_names {
            if !candidates.contains(&name) {
                candidates.push(name);
            }
        }
    }

    if candidates.is_empty() {
        candidates.push(default_binary_name(package));
    } else {
        let derived = default_binary_name(package);
        if !candidates.contains(&derived) {
            candidates.push(derived);
        }
    }

    candidates
}

async fn read_package_bin_names(package: &str, package_path: &Path) -> Option<Vec<String>> {
    let content = tokio::fs::read_to_string(package_path.join("package.json"))
        .await
        .ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let bin = json.get("bin")?;

    match bin {
        serde_json::Value::String(_) => {
            let pkg_name = json
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or(package);
            Some(vec![default_binary_name(pkg_name)])
        }
        serde_json::Value::Object(map) => {
            if map.is_empty() {
                None
            } else {
                let mut keys = map.keys().cloned().collect::<Vec<_>>();
                keys.sort();
                Some(keys)
            }
        }
        _ => None,
    }
}

fn resolve_package_path(
    snapshot: &GlobalPackageSnapshot,
    package: &str,
    entry: &GlobalPackageEntry,
) -> Option<PathBuf> {
    if let Some(path) = &entry.path {
        return Some(path.clone());
    }

    let root = snapshot.root_path.as_ref()?;
    let mut path = if root.file_name().and_then(|p| p.to_str()) == Some("node_modules") {
        root.to_path_buf()
    } else {
        root.join("node_modules")
    };
    if package.starts_with('@') {
        let (scope, name) = package.split_once('/')?;
        path = path.join(scope).join(name);
    } else {
        path = path.join(package);
    }
    Some(path)
}

fn default_binary_name(package: &str) -> String {
    package
        .rsplit('/')
        .next()
        .unwrap_or(package)
        .to_string()
}

fn resolve_binary_path(candidate: &str, bin_dir: Option<&Path>) -> Option<PathBuf> {
    let bin_dir = bin_dir?;
    let path = bin_dir.join(candidate);
    if path.is_file() {
        return Some(path);
    }

    if cfg!(windows) {
        for ext in ["cmd", "exe", "bat", "ps1"] {
            let path = bin_dir.join(format!("{}.{}", candidate, ext));
            if path.is_file() {
                return Some(path);
            }
        }
    }

    None
}

fn build_command_string(program: &str, args: &[&str]) -> String {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(program);
    parts.extend_from_slice(args);
    join_command_parts(&parts)
}

fn build_command_string_owned(program: &str, args: &[String]) -> String {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(program);
    for arg in args {
        parts.push(arg);
    }
    join_command_parts(&parts)
}

fn join_command_parts(parts: &[&str]) -> String {
    #[cfg(windows)]
    {
        parts
            .iter()
            .map(|part| quote_windows_arg(part))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[cfg(not(windows))]
    {
        shlex::try_join(parts.iter().copied()).unwrap_or_else(|_| parts.join(" "))
    }
}

#[cfg(windows)]
fn quote_windows_arg(arg: &str) -> String {
    if arg.contains(' ') || arg.contains('\t') || arg.contains('"') {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        arg.to_string()
    }
}

fn all_agents() -> Vec<BaseCodingAgent> {
    vec![
        BaseCodingAgent::ClaudeCode,
        BaseCodingAgent::Amp,
        BaseCodingAgent::Gemini,
        BaseCodingAgent::Codex,
        BaseCodingAgent::FakeAgent,
        BaseCodingAgent::Opencode,
        BaseCodingAgent::CursorAgent,
        BaseCodingAgent::QwenCode,
        BaseCodingAgent::Copilot,
        BaseCodingAgent::Droid,
    ]
}

async fn load_pnpm_global_packages() -> Option<GlobalPackageSnapshot> {
    let pnpm_path = resolve_executable_path("pnpm").await?;
    let list_output = run_command(&pnpm_path, &["list", "-g", "--json"]).await?;
    let mut snapshot = parse_pnpm_list_json(&list_output)?;
    snapshot.bin_dir = run_command(&pnpm_path, &["bin", "-g"])
        .await
        .map(PathBuf::from);
    Some(snapshot)
}

async fn load_npm_global_packages() -> Option<GlobalPackageSnapshot> {
    let npm_path = resolve_executable_path("npm").await?;
    let list_output = run_command(&npm_path, &["list", "-g", "--json"]).await?;
    let mut snapshot = parse_npm_list_json(&list_output)?;
    snapshot.bin_dir = run_command(&npm_path, &["bin", "-g"])
        .await
        .map(PathBuf::from);
    Some(snapshot)
}

async fn run_command(program: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().await.ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        None
    } else {
        Some(stdout)
    }
}

fn parse_pnpm_list_json(raw: &str) -> Option<GlobalPackageSnapshot> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let root = match &value {
        serde_json::Value::Array(arr) => arr.first()?,
        serde_json::Value::Object(_) => &value,
        _ => return None,
    };

    parse_dependencies_snapshot(root)
}

fn parse_npm_list_json(raw: &str) -> Option<GlobalPackageSnapshot> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    parse_dependencies_snapshot(&value)
}

fn parse_dependencies_snapshot(root: &serde_json::Value) -> Option<GlobalPackageSnapshot> {
    let root_path = root
        .get("path")
        .and_then(|value| value.as_str())
        .map(PathBuf::from);

    let mut packages = HashMap::new();
    let dependencies = root.get("dependencies").and_then(|v| v.as_object());
    if let Some(dependencies) = dependencies {
        for (name, entry) in dependencies {
            let version = entry
                .get("version")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
            if version.is_empty() {
                continue;
            }
            let path = entry
                .get("path")
                .and_then(|value| value.as_str())
                .map(PathBuf::from);
            packages.insert(
                name.clone(),
                GlobalPackageEntry {
                    version,
                    path,
                },
            );
        }
    }

    Some(GlobalPackageSnapshot {
        packages,
        root_path,
        bin_dir: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_resolution_prefers_pnpm() {
        let pnpm = Some(ResolvedBinary {
            path: PathBuf::from("/pnpm/bin/agent"),
            version: "1.2.3".to_string(),
            source: AgentCommandSource::PnpmGlobal,
        });
        let npm = Some(ResolvedBinary {
            path: PathBuf::from("/npm/bin/agent"),
            version: "9.9.9".to_string(),
            source: AgentCommandSource::NpmGlobal,
        });

        let resolved = select_resolution("pkg", &[], pnpm, npm);
        assert_eq!(resolved.source, AgentCommandSource::PnpmGlobal);
        assert_eq!(resolved.version.as_deref(), Some("1.2.3"));
    }

    #[test]
    fn select_resolution_uses_npm_when_pnpm_missing() {
        let npm = Some(ResolvedBinary {
            path: PathBuf::from("/npm/bin/agent"),
            version: "2.0.0".to_string(),
            source: AgentCommandSource::NpmGlobal,
        });

        let resolved = select_resolution("pkg", &[], None, npm);
        assert_eq!(resolved.source, AgentCommandSource::NpmGlobal);
        assert_eq!(resolved.version.as_deref(), Some("2.0.0"));
    }

    #[test]
    fn select_resolution_falls_back_to_latest() {
        let resolved = select_resolution("pkg", &[], None, None);
        assert_eq!(resolved.source, AgentCommandSource::NpxLatest);
        assert!(resolved.fallback_to_latest);
    }
}
