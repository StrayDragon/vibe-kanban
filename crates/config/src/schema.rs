use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
};

use anyhow::Error;
use executors::profile::ExecutorConfigs;
use executors_protocol::{BaseCodingAgent, ExecutorProfileId};
use serde::{Deserialize, Serialize};
use strum_macros::EnumString;
use ts_rs::TS;
use utils_assets::SoundAssets;
use utils_core::cache_dir;

use super::editor::EditorConfig;

pub const CURRENT_CONFIG_VERSION: &str = "v10";

fn default_executor_profile() -> ExecutorProfileId {
    ExecutorProfileId::new(BaseCodingAgent::ClaudeCode)
}

fn default_git_branch_prefix() -> String {
    "vk".to_string()
}

fn default_pr_auto_description_enabled() -> bool {
    true
}

fn default_git_no_verify() -> bool {
    false
}

fn default_diff_preview_guard() -> DiffPreviewGuardPreset {
    DiffPreviewGuardPreset::Balanced
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, Default, schemars::JsonSchema)]
#[ts(use_ts_enum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiffPreviewGuardPreset {
    Safe,
    #[default]
    Balanced,
    Relaxed,
    Off,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, Default, schemars::JsonSchema)]
pub struct ShowcaseState {
    #[serde(default)]
    pub seen_features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, EnumString, Default, schemars::JsonSchema)]
#[ts(use_ts_enum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS, Default, schemars::JsonSchema)]
#[ts(export)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UiLanguage {
    #[default]
    Browser, // Detect from browser
    En,     // Force English
    ZhHans, // Force Simplified Chinese
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(default)]
pub struct NotificationConfig {
    #[serde(alias = "soundEnabled")]
    #[schemars(description = "是否启用声音提示。")]
    pub sound_enabled: bool,
    #[serde(alias = "pushEnabled")]
    #[schemars(description = "是否启用桌面推送通知。")]
    pub push_enabled: bool,
    #[serde(alias = "soundFile")]
    #[schemars(description = "声音文件预设。")]
    pub sound_file: SoundFile,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            sound_enabled: true,
            push_enabled: true,
            sound_file: SoundFile::CowMooing,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(default)]
pub struct GitHubConfig {
    #[schemars(description = "GitHub PAT（推荐通过 secret.env 并使用 ${GITHUB_PAT} 注入）。")]
    pub pat: Option<String>,
    #[serde(alias = "oauthToken")]
    #[schemars(description = "GitHub OAuth Token（推荐通过 secret.env 注入）。")]
    pub oauth_token: Option<String>,
    #[schemars(description = "GitHub 用户名（可选）。")]
    pub username: Option<String>,
    #[serde(alias = "primaryEmail")]
    #[schemars(description = "GitHub 主邮箱（可选）。")]
    pub primary_email: Option<String>,
    #[serde(alias = "defaultPrBase")]
    #[schemars(description = "默认 PR base 分支（默认 main）。")]
    pub default_pr_base: Option<String>,
}

impl Default for GitHubConfig {
    fn default() -> Self {
        Self {
            pat: None,
            oauth_token: None,
            username: None,
            primary_email: None,
            default_pr_base: Some("main".to_string()),
        }
    }
}

impl GitHubConfig {
    pub fn token(&self) -> Option<String> {
        self.pat
            .as_deref()
            .or(self.oauth_token.as_deref())
            .map(|s| s.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, EnumString, schemars::JsonSchema)]
#[ts(use_ts_enum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum SoundFile {
    AbstractSound1,
    AbstractSound2,
    AbstractSound3,
    AbstractSound4,
    CowMooing,
    PhoneVibration,
    Rooster,
}

impl SoundFile {
    pub fn to_filename(&self) -> &'static str {
        match self {
            SoundFile::AbstractSound1 => "abstract-sound1.wav",
            SoundFile::AbstractSound2 => "abstract-sound2.wav",
            SoundFile::AbstractSound3 => "abstract-sound3.wav",
            SoundFile::AbstractSound4 => "abstract-sound4.wav",
            SoundFile::CowMooing => "cow-mooing.wav",
            SoundFile::PhoneVibration => "phone-vibration.wav",
            SoundFile::Rooster => "rooster.wav",
        }
    }

    pub async fn serve(&self) -> Result<rust_embed::EmbeddedFile, Error> {
        match SoundAssets::get(self.to_filename()) {
            Some(content) => Ok(content),
            None => {
                tracing::error!("Sound file not found: {}", self.to_filename());
                Err(anyhow::anyhow!(
                    "Sound file not found: {}",
                    self.to_filename()
                ))
            }
        }
    }

    pub async fn get_path(&self) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        use std::io::Write;

        let filename = self.to_filename();
        let cache_dir = cache_dir();
        let cached_path = cache_dir.join(format!("sound-{filename}"));

        if std::fs::metadata(&cached_path)
            .map(|metadata| metadata.len() > 0)
            .unwrap_or(false)
        {
            return Ok(cached_path);
        }

        let sound_data = SoundAssets::get(filename)
            .ok_or_else(|| format!("Embedded sound file not found: {filename}"))?
            .data;

        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| format!("Failed to create cache directory: {e}"))?;

        let mut file = std::fs::File::create(&cached_path)
            .map_err(|e| format!("Failed to create cached sound file: {e}"))?;

        file.write_all(&sound_data)
            .map_err(|e| format!("Failed to write sound data to cached file: {e}"))?;

        drop(file);

        Ok(cached_path)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, Default, schemars::JsonSchema)]
#[ts(use_ts_enum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AccessControlMode {
    #[default]
    Disabled,
    Token,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(default)]
pub struct AccessControlConfig {
    #[schemars(description = "访问控制模式。DISABLED 表示不启用；TOKEN 表示要求提供 token。")]
    pub mode: AccessControlMode,
    #[schemars(description = "访问 token（当 mode=TOKEN 时必填；推荐通过 secret.env 注入）。")]
    pub token: Option<String>,
    #[serde(alias = "allowLocalhostBypass")]
    #[schemars(description = "是否允许 localhost 绕过 token 校验（仅当 mode=TOKEN 时有意义）。")]
    pub allow_localhost_bypass: bool,
}

impl Default for AccessControlConfig {
    fn default() -> Self {
        Self {
            mode: AccessControlMode::Disabled,
            token: None,
            allow_localhost_bypass: true,
        }
    }
}

fn default_scheduler_max_concurrent() -> i32 {
    1
}

fn default_scheduler_max_retries() -> i32 {
    3
}

fn default_default_continuation_turns() -> i32 {
    0
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, Default, schemars::JsonSchema)]
#[ts(use_ts_enum)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceLifecycleHookFailurePolicy {
    BlockStart,
    #[default]
    WarnOnly,
    BlockCleanup,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[ts(use_ts_enum)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceLifecycleHookRunMode {
    OncePerWorkspace,
    EveryPrepare,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, schemars::JsonSchema)]
pub struct WorkspaceLifecycleHookConfig {
    #[schemars(description = "Hook 命令（单一命令，不支持 shell 操作符拼接）。")]
    pub command: String,
    #[schemars(description = "工作目录（相对 workspace root）。")]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub failure_policy: WorkspaceLifecycleHookFailurePolicy,
    #[schemars(description = "执行模式（after_prepare 必填；before_cleanup 不支持）。")]
    pub run_mode: Option<WorkspaceLifecycleHookRunMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, schemars::JsonSchema)]
pub struct ProjectRepoConfig {
    #[schemars(description = "Git 仓库绝对路径。")]
    pub path: String,
    #[schemars(description = "显示名称（可选，仅用于 UI 展示）。")]
    pub display_name: Option<String>,
    #[schemars(description = "可选：在 coding agent 前运行的 setup 脚本（单一命令）。")]
    pub setup_script: Option<String>,
    #[schemars(description = "可选：在 workspace 清理前运行的 cleanup 脚本（单一命令）。")]
    pub cleanup_script: Option<String>,
    #[schemars(description = "可选：复制文件规则（legacy 字段；建议逐步移除）。")]
    pub copy_files: Option<String>,
    #[serde(default)]
    #[schemars(
        description = "当项目包含多个 repos 且这些 repos 有 setup_script 时，是否并行执行。"
    )]
    pub parallel_setup_script: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, Default, schemars::JsonSchema)]
#[ts(use_ts_enum)]
#[serde(rename_all = "snake_case")]
pub enum ProjectMcpExecutorPolicyMode {
    #[default]
    InheritAll,
    AllowList,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, schemars::JsonSchema)]
pub struct ProjectConfig {
    #[schemars(description = "项目稳定 id（UUID，必须显式写入，且全局唯一）。")]
    pub id: Option<uuid::Uuid>,
    #[schemars(description = "项目名称（用于 UI 展示）。")]
    pub name: String,
    #[schemars(description = "项目 repos（至少一个）。")]
    #[serde(default)]
    pub repos: Vec<ProjectRepoConfig>,
    #[schemars(description = "可选：项目 dev server 脚本（单一命令）。")]
    pub dev_script: Option<String>,
    #[schemars(description = "可选：dev script 工作目录（相对 workspace root）。")]
    pub dev_script_working_dir: Option<String>,
    #[schemars(description = "可选：默认 agent 工作目录（相对 workspace root）。")]
    pub default_agent_working_dir: Option<String>,
    #[schemars(
        description = "项目级 git hooks 跳过策略。\n\n- null/未设置：继承全局 `git_no_verify`\n- true/false：覆盖全局设置"
    )]
    pub git_no_verify_override: Option<bool>,
    #[serde(default = "default_scheduler_max_concurrent")]
    pub scheduler_max_concurrent: i32,
    #[serde(default = "default_scheduler_max_retries")]
    pub scheduler_max_retries: i32,
    #[serde(default = "default_default_continuation_turns")]
    pub default_continuation_turns: i32,
    #[serde(default)]
    pub mcp_auto_executor_policy_mode: ProjectMcpExecutorPolicyMode,
    #[serde(default)]
    #[schemars(
        description = "当 mode=allow_list 时生效：允许的 executor profiles（executor + variant）。"
    )]
    pub mcp_auto_executor_policy_allow_list: Vec<ExecutorProfileId>,
    pub after_prepare_hook: Option<WorkspaceLifecycleHookConfig>,
    pub before_cleanup_hook: Option<WorkspaceLifecycleHookConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(default)]
pub struct Config {
    #[serde(alias = "configVersion")]
    #[schemars(description = "配置版本号（通常无需手动修改；启动时会归一化为当前版本）。")]
    pub config_version: String,
    pub theme: ThemeMode,
    #[serde(alias = "executorProfile")]
    #[schemars(description = "默认 executor profile（executor + 可选 variant）。")]
    pub executor_profile: ExecutorProfileId,
    #[serde(default)]
    #[serde(alias = "executorProfiles")]
    #[schemars(
        description = "Executor profiles 覆盖（可选，按需配置）。\n\n该字段会与内置 defaults 合并后作为运行时可用 profiles。\n当某个 executor/variant 在本次构建中不可用时，引用会导致配置校验失败。"
    )]
    pub executor_profiles: Option<ExecutorConfigs>,
    #[serde(alias = "disclaimerAcknowledged")]
    pub disclaimer_acknowledged: bool,
    #[serde(alias = "onboardingAcknowledged")]
    pub onboarding_acknowledged: bool,
    pub notifications: NotificationConfig,
    pub editor: EditorConfig,
    pub github: GitHubConfig,
    #[serde(alias = "workspaceDir")]
    pub workspace_dir: Option<String>,
    #[serde(alias = "lastAppVersion")]
    pub last_app_version: Option<String>,
    #[serde(alias = "showReleaseNotes")]
    pub show_release_notes: bool,
    pub language: UiLanguage,
    #[serde(alias = "gitBranchPrefix")]
    pub git_branch_prefix: String,
    #[serde(alias = "gitNoVerify")]
    #[schemars(
        description = "全局 git hooks 跳过策略（默认值）。当 projects 中存在 git_no_verify_override=true/false 时，项目级优先；null/未设置则继承此全局值。"
    )]
    pub git_no_verify: bool,
    pub showcases: ShowcaseState,
    #[serde(alias = "prAutoDescriptionEnabled")]
    pub pr_auto_description_enabled: bool,
    #[serde(alias = "prAutoDescriptionPrompt")]
    pub pr_auto_description_prompt: Option<String>,
    #[serde(alias = "llmanClaudeCodePath")]
    pub llman_claude_code_path: Option<String>,
    #[serde(alias = "diffPreviewGuard")]
    pub diff_preview_guard: DiffPreviewGuardPreset,
    #[serde(alias = "accessControl")]
    pub access_control: AccessControlConfig,
    #[serde(default)]
    #[schemars(
        description = "Projects 与 repos 配置（file-first）。\n\n- projects 的 `id` 必须显式提供且全局唯一\n- repo `path` 必须为绝对路径\n- 写入/修改方式：编辑 `config.yaml` 后调用 `POST /api/config/reload`"
    )]
    pub projects: Vec<ProjectConfig>,
}

impl Config {
    pub fn normalized(mut self) -> Self {
        self.config_version = CURRENT_CONFIG_VERSION.to_string();

        if !utils_git::is_valid_branch_prefix(&self.git_branch_prefix) {
            tracing::warn!(
                "Invalid git branch prefix '{}', resetting to default",
                self.git_branch_prefix
            );
            self.git_branch_prefix = default_git_branch_prefix();
        }

        if matches!(
            self.executor_profile.variant.as_deref(),
            Some(variant) if variant.trim().is_empty()
        ) {
            self.executor_profile.variant = None;
        }

        if matches!(
            self.llman_claude_code_path.as_deref(),
            Some(path) if path.trim().is_empty()
        ) {
            self.llman_claude_code_path = None;
        }

        if matches!(
            self.workspace_dir.as_deref(),
            Some(dir) if dir.trim().is_empty()
        ) {
            self.workspace_dir = None;
        }

        if matches!(
            self.access_control.token.as_deref(),
            Some(token) if token.trim().is_empty()
        ) {
            self.access_control.token = None;
        }

        self
    }

    pub fn validate_projects(&self, profiles: &ExecutorConfigs) -> Result<(), String> {
        fn validate_single_command_text(label: &str, script: &str) -> Result<(), String> {
            let trimmed = script.trim();
            if trimmed.is_empty() {
                return Err(format!("{label} command cannot be empty"));
            }

            let tokens = shlex::split(trimmed).ok_or_else(|| {
                format!("{label} command must be valid shell-like command text")
            })?;

            if tokens.is_empty() {
                return Err(format!("{label} command must include an executable"));
            }

            let has_forbidden = tokens.iter().any(|token| {
                matches!(
                    token.as_str(),
                    "|" | "||" | "&" | "&&" | ";" | ">" | ">>" | "<" | "<<"
                )
            });
            if has_forbidden {
                return Err(format!(
                    "{label} command must be a single command without shell operators"
                ));
            }

            Ok(())
        }

        fn validate_workspace_relative_dir(label: &str, working_dir: &str) -> Result<(), String> {
            let trimmed = working_dir.trim();
            if trimmed.is_empty() {
                return Ok(());
            }

            let path = Path::new(trimmed);
            if path.is_absolute() {
                return Err(format!(
                    "{label} working directory must be relative to the workspace root"
                ));
            }
            if path
                .components()
                .any(|component| matches!(component, Component::ParentDir))
            {
                return Err(format!(
                    "{label} working directory cannot traverse outside the workspace"
                ));
            }
            Ok(())
        }

        fn validate_workspace_hook(
            phase: &str,
            hook: &WorkspaceLifecycleHookConfig,
        ) -> Result<(), String> {
            validate_single_command_text(phase, &hook.command)?;
            if let Some(working_dir) = hook.working_dir.as_deref() {
                validate_workspace_relative_dir(phase, working_dir)?;
            }

            match phase {
                "after_prepare_hook" => {
                    match hook.failure_policy {
                        WorkspaceLifecycleHookFailurePolicy::BlockStart
                        | WorkspaceLifecycleHookFailurePolicy::WarnOnly => {}
                        WorkspaceLifecycleHookFailurePolicy::BlockCleanup => {
                            return Err(
                                "after_prepare hooks only support block_start or warn_only"
                                    .to_string(),
                            );
                        }
                    }
                    if hook.run_mode.is_none() {
                        return Err("after_prepare hooks require a run_mode".to_string());
                    }
                }
                "before_cleanup_hook" => {
                    match hook.failure_policy {
                        WorkspaceLifecycleHookFailurePolicy::WarnOnly
                        | WorkspaceLifecycleHookFailurePolicy::BlockCleanup => {}
                        WorkspaceLifecycleHookFailurePolicy::BlockStart => {
                            return Err(
                                "before_cleanup hooks only support warn_only or block_cleanup"
                                    .to_string(),
                            );
                        }
                    }
                    if hook.run_mode.is_some() {
                        return Err("before_cleanup hooks do not support run_mode".to_string());
                    }
                }
                _ => {}
            }

            Ok(())
        }

        let mut seen_ids = HashSet::new();
        for (project_index, project) in self.projects.iter().enumerate() {
            let Some(project_id) = project.id else {
                return Err(format!("projects[{project_index}] missing id (UUID)"));
            };
            if !seen_ids.insert(project_id) {
                return Err(format!("Duplicate project id: {project_id}"));
            }

            if project.name.trim().is_empty() {
                return Err(format!("projects[{project_index}].name cannot be empty"));
            }

            if project.repos.is_empty() {
                return Err(format!("projects[{project_index}].repos must not be empty"));
            }

            let mut seen_repo_paths = HashSet::new();
            for (repo_index, repo) in project.repos.iter().enumerate() {
                let path = repo.path.trim();
                if path.is_empty() {
                    return Err(format!(
                        "projects[{project_index}].repos[{repo_index}].path cannot be empty"
                    ));
                }
                if !Path::new(path).is_absolute() {
                    return Err(format!(
                        "projects[{project_index}].repos[{repo_index}].path must be an absolute path"
                    ));
                }
                if !seen_repo_paths.insert(path.to_string()) {
                    return Err(format!(
                        "projects[{project_index}].repos[{repo_index}] duplicate path: {path}"
                    ));
                }

                if let Some(script) = repo.setup_script.as_deref() {
                    validate_single_command_text(
                        &format!(
                            "projects[{project_index}].repos[{repo_index}].setup_script"
                        ),
                        script,
                    )?;
                }

                if let Some(script) = repo.cleanup_script.as_deref() {
                    validate_single_command_text(
                        &format!(
                            "projects[{project_index}].repos[{repo_index}].cleanup_script"
                        ),
                        script,
                    )?;
                }
            }

            if let Some(script) = project.dev_script.as_deref() {
                validate_single_command_text(&format!("projects[{project_index}].dev_script"), script)
                    ?;
            }

            if let Some(working_dir) = project.dev_script_working_dir.as_deref() {
                validate_workspace_relative_dir(
                    &format!("projects[{project_index}].dev_script_working_dir"),
                    working_dir,
                )?;
            }

            if let Some(working_dir) = project.default_agent_working_dir.as_deref() {
                validate_workspace_relative_dir(
                    &format!("projects[{project_index}].default_agent_working_dir"),
                    working_dir,
                )?;
            }

            if project.scheduler_max_concurrent < 1 {
                return Err(format!(
                    "projects[{project_index}].scheduler_max_concurrent must be at least 1"
                ));
            }

            if project.scheduler_max_retries < 0 {
                return Err(format!(
                    "projects[{project_index}].scheduler_max_retries must be zero or greater"
                ));
            }

            if project.default_continuation_turns < 0 {
                return Err(format!(
                    "projects[{project_index}].default_continuation_turns must be zero or greater"
                ));
            }

            if matches!(
                project.mcp_auto_executor_policy_mode,
                ProjectMcpExecutorPolicyMode::AllowList
            ) {
                if project.mcp_auto_executor_policy_allow_list.is_empty() {
                    return Err(format!(
                        "projects[{project_index}].mcp_auto_executor_policy_allow_list must not be empty when mode=allow_list"
                    ));
                }

                for (entry_index, entry) in project
                    .mcp_auto_executor_policy_allow_list
                    .iter()
                    .enumerate()
                {
                    profiles
                        .require_coding_agent(entry)
                        .map_err(|err| {
                            format!(
                                "projects[{project_index}].mcp_auto_executor_policy_allow_list[{entry_index}] invalid executor profile: {err}"
                            )
                        })?;
                }
            }

            if let Some(hook) = project.after_prepare_hook.as_ref() {
                validate_workspace_hook("after_prepare_hook", hook)?;
            }

            if let Some(hook) = project.before_cleanup_hook.as_ref() {
                validate_workspace_hook("before_cleanup_hook", hook)?;
            }
        }

        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: CURRENT_CONFIG_VERSION.to_string(),
            theme: ThemeMode::System,
            executor_profile: default_executor_profile(),
            executor_profiles: None,
            disclaimer_acknowledged: false,
            onboarding_acknowledged: false,
            notifications: NotificationConfig::default(),
            editor: EditorConfig::default(),
            github: GitHubConfig::default(),
            workspace_dir: None,
            last_app_version: None,
            show_release_notes: false,
            language: UiLanguage::default(),
            git_branch_prefix: default_git_branch_prefix(),
            git_no_verify: default_git_no_verify(),
            showcases: ShowcaseState::default(),
            pr_auto_description_enabled: default_pr_auto_description_enabled(),
            pr_auto_description_prompt: None,
            llman_claude_code_path: None,
            diff_preview_guard: default_diff_preview_guard(),
            access_control: AccessControlConfig::default(),
            projects: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_apply_for_empty_config() {
        let config = serde_yaml::from_str::<Config>("{}")
            .expect("YAML parse should succeed for empty mapping")
            .normalized();

        assert_eq!(config.config_version, CURRENT_CONFIG_VERSION);
        assert!(matches!(config.theme, ThemeMode::System));
        assert_eq!(
            config.executor_profile,
            ExecutorProfileId::new(BaseCodingAgent::ClaudeCode)
        );
        assert_eq!(config.git_branch_prefix, default_git_branch_prefix());
        assert!(!config.disclaimer_acknowledged);
    }

    #[test]
    fn aliases_and_normalization_are_applied() {
        let raw = r#"
configVersion: v1
gitBranchPrefix: feature
executorProfile:
  executor: CLAUDE_CODE
"#;

        let config = serde_yaml::from_str::<Config>(raw)
            .expect("YAML parse should succeed")
            .normalized();

        assert_eq!(config.config_version, CURRENT_CONFIG_VERSION);
        assert_eq!(config.git_branch_prefix, "feature");
    }

    #[test]
    fn invalid_branch_prefix_is_reset() {
        let raw = r#"
gitBranchPrefix: foo/bar
"#;
        let config = serde_yaml::from_str::<Config>(raw)
            .expect("YAML parse should succeed")
            .normalized();

        assert_eq!(config.git_branch_prefix, default_git_branch_prefix());
    }
}
