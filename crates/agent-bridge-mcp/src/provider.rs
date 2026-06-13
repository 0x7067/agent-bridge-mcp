use crate::claude_host::ClaudeHostCommand;
use crate::domain::{LaunchProfile, ProviderKind, TaskMode};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::env;

const PROVIDER_SMOKE_PROMPT: &str = "Reply with exactly: AGENT_BRIDGE_PROVIDER_SMOKE_OK";
pub const PROVIDER_SMOKE_TOKEN: &str = "AGENT_BRIDGE_PROVIDER_SMOKE_OK";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCommand {
    pub provider: ProviderKind,
    pub command_kind: Option<String>,
    pub claude_host: Option<ClaudeHostCommand>,
    pub command: String,
    pub args: Vec<String>,
    pub stdin: Option<String>,
    pub redactions: Vec<String>,
    pub cwd: String,
    pub timeout_seconds: i64,
    pub env: BTreeMap<String, String>,
    pub profile: LaunchProfile,
    pub prompt_strategy: String,
    pub profile_diagnostics: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTask<'a> {
    pub provider: ProviderKind,
    pub mode: TaskMode,
    pub prompt: &'a str,
    pub title: Option<&'a str>,
    pub cwd: &'a str,
    pub timeout_seconds: i64,
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
    pub thinking: Option<&'a str>,
    pub profile: LaunchProfile,
}

pub fn capabilities() -> Value {
    json!({
        "claude": {
            "modes": ["research", "review", "implement", "command"],
            "supportsReply": false,
            "supportsResume": false,
            "supportsWorktreeIsolation": true,
            "effort": ["low", "medium", "high", "xhigh", "max"],
            "launchProfiles": ["bridge", "bare"],
            "presentationActions": presentation_actions(),
            "outputCadence": output_cadence(ProviderKind::Claude),
            "readiness": default_readiness(),
            "reducedConfiguration": reduced_configuration(ProviderKind::Claude)
        },
        "cursor": {
            "modes": ["research", "review", "implement"],
            "supportsReply": false,
            "supportsResume": false,
            "supportsWorktreeIsolation": true,
            "launchProfiles": ["bridge", "bare"],
            "presentationActions": presentation_actions(),
            "outputCadence": output_cadence(ProviderKind::Cursor),
            "readiness": default_readiness(),
            "reducedConfiguration": reduced_configuration(ProviderKind::Cursor)
        },
        "kimi": {
            "modes": ["research", "review", "implement", "command"],
            "supportsReply": false,
            "supportsResume": false,
            "supportsWorktreeIsolation": true,
            "thinking": ["off", "minimal", "low", "medium", "high", "xhigh"],
            "launchProfiles": ["bridge", "bare"],
            "presentationActions": presentation_actions(),
            "outputCadence": output_cadence(ProviderKind::Kimi),
            "readiness": default_readiness(),
            "reducedConfiguration": reduced_configuration(ProviderKind::Kimi)
        },
        "codex": {
            "modes": ["research", "review", "implement", "command"],
            "supportsReply": false,
            "supportsResume": false,
            "supportsWorktreeIsolation": true,
            "thinking": ["low", "medium", "high", "xhigh"],
            "launchProfiles": ["bridge", "bare"],
            "presentationActions": presentation_actions(),
            "outputCadence": output_cadence(ProviderKind::Codex),
            "readiness": default_readiness(),
            "reducedConfiguration": reduced_configuration(ProviderKind::Codex)
        },
        "forge": {
            "modes": ["research", "review", "implement", "command"],
            "supportsReply": false,
            "supportsResume": false,
            "supportsWorktreeIsolation": true,
            "launchProfiles": ["bridge", "bare"],
            "presentationActions": presentation_actions(),
            "outputCadence": output_cadence(ProviderKind::Forge),
            "readiness": default_readiness(),
            "reducedConfiguration": reduced_configuration(ProviderKind::Forge)
        },
        "antigravity": {
            "modes": ["research", "review", "implement", "command"],
            "supportsReply": false,
            "supportsResume": false,
            "supportsWorktreeIsolation": true,
            "launchProfiles": ["bridge", "bare"],
            "presentationActions": presentation_actions(),
            "outputCadence": output_cadence(ProviderKind::Antigravity),
            "readiness": default_readiness(),
            "reducedConfiguration": reduced_configuration(ProviderKind::Antigravity),
            "readOnlyEnforcement": {
                "research": "prompt_enforced",
                "review": "prompt_enforced",
                "note": "Antigravity --sandbox is used for non-mutating modes, but Agent Bridge does not claim verified read-only filesystem enforcement."
            }
        }
    })
}

fn presentation_actions() -> Value {
    json!({
        "wait": "supported",
        "observe": "supported",
        "inspectStatus": "supported",
        "inspectLogs": "supported",
        "inspectTranscript": "supported",
        "inspectResult": "supported",
        "stop": "supported",
        "cleanup": "supported",
        "reply": "unsupported",
        "resume": "unsupported"
    })
}

pub fn output_cadence(provider: ProviderKind) -> Value {
    match provider {
        ProviderKind::Cursor => json!({
            "cadence": "final_json",
            "firstOutputExpected": "near_final",
            "recommendedPollMs": 30000,
            "recommendedSilentBudgetMs": 240000,
            "fallbackAfterMs": 300000,
            "advisory": true,
            "note": "Cursor JSON output may be silent until final completion."
        }),
        ProviderKind::Claude => json!({
            "cadence": "provider_dependent",
            "firstOutputExpected": "unknown",
            "recommendedPollMs": 30000,
            "recommendedSilentBudgetMs": 120000,
            "fallbackAfterMs": 180000,
            "advisory": true,
            "note": "Claude output cadence varies by launch strategy and host runner."
        }),
        ProviderKind::Kimi => json!({
            "cadence": "provider_dependent",
            "firstOutputExpected": "intermittent",
            "recommendedPollMs": 30000,
            "recommendedSilentBudgetMs": 120000,
            "fallbackAfterMs": 180000,
            "advisory": true,
            "note": "Kimi output cadence is provider-dependent."
        }),
        ProviderKind::Codex => json!({
            "cadence": "provider_dependent",
            "firstOutputExpected": "intermittent",
            "recommendedPollMs": 30000,
            "recommendedSilentBudgetMs": 120000,
            "fallbackAfterMs": 180000,
            "advisory": true,
            "note": "Codex output cadence is provider-dependent."
        }),
        ProviderKind::Forge => json!({
            "cadence": "provider_dependent",
            "firstOutputExpected": "intermittent",
            "recommendedPollMs": 30000,
            "recommendedSilentBudgetMs": 120000,
            "fallbackAfterMs": 180000,
            "advisory": true,
            "note": "Forge output cadence is provider-dependent."
        }),
        ProviderKind::Antigravity => json!({
            "cadence": "provider_dependent",
            "firstOutputExpected": "intermittent",
            "recommendedPollMs": 30000,
            "recommendedSilentBudgetMs": 120000,
            "fallbackAfterMs": 180000,
            "advisory": true,
            "note": "Antigravity print-mode output cadence is provider-dependent."
        }),
    }
}

fn default_readiness() -> Value {
    json!({
        "state": "stale",
        "startupVerified": false,
        "launchable": false,
        "probe": "not_checked"
    })
}

fn reduced_configuration(provider: ProviderKind) -> Value {
    match provider {
        ProviderKind::Claude => json!({
            "compactPrompt": "supported",
            "customSystemPrompt": "supported",
            "hooks": "best_effort",
            "skills": "best_effort",
            "configIsolation": "best_effort",
            "memorySession": "supported",
            "contextFiles": "best_effort"
        }),
        ProviderKind::Codex => json!({
            "compactPrompt": "supported",
            "customSystemPrompt": "best_effort",
            "hooks": "unsupported",
            "skills": "supported",
            "configIsolation": "supported",
            "memorySession": "supported",
            "contextFiles": "best_effort"
        }),
        ProviderKind::Forge => json!({
            "compactPrompt": "supported",
            "customSystemPrompt": "unsupported",
            "hooks": "unsupported",
            "skills": "best_effort",
            "configIsolation": "best_effort",
            "memorySession": "best_effort",
            "contextFiles": "best_effort"
        }),
        ProviderKind::Cursor => json!({
            "compactPrompt": "supported",
            "customSystemPrompt": "unsupported",
            "hooks": "unsupported",
            "skills": "best_effort",
            "configIsolation": "best_effort",
            "memorySession": "supported",
            "contextFiles": "best_effort"
        }),
        ProviderKind::Kimi => json!({
            "compactPrompt": "supported",
            "customSystemPrompt": "supported",
            "hooks": "supported",
            "skills": "supported",
            "configIsolation": "supported",
            "memorySession": "supported",
            "contextFiles": "supported"
        }),
        ProviderKind::Antigravity => json!({
            "compactPrompt": "supported",
            "customSystemPrompt": "unsupported",
            "hooks": "unsupported",
            "skills": "unsupported",
            "configIsolation": "best_effort",
            "memorySession": "best_effort",
            "contextFiles": "best_effort"
        }),
    }
}

pub fn validate_options(task: &ProviderTask<'_>) -> Result<(), String> {
    adapter_for(task.provider).validate(task)
}

pub fn version_command(provider: ProviderKind) -> ProviderCommand {
    ProviderCommand {
        provider,
        command_kind: provider_command_kind(provider),
        claude_host: None,
        command: resolve_command(provider),
        args: vec!["--version".to_string()],
        stdin: None,
        redactions: Vec::new(),
        cwd: env::current_dir()
            .unwrap_or_else(|_| ".".into())
            .display()
            .to_string(),
        timeout_seconds: 5,
        env: provider_env(provider),
        profile: LaunchProfile::Bridge,
        prompt_strategy: "version".to_string(),
        profile_diagnostics: profile_diagnostics(provider, LaunchProfile::Bridge),
    }
}

/// Builds a non-Claude smoke `ProviderCommand`, filling the boilerplate that is
/// identical across providers (minimal prompt strategy, smoke-prompt redaction,
/// empty env, no stdin) so each provider arm only supplies `command` and `args`.
fn minimal_smoke_command(
    task: &ProviderTask,
    command: String,
    args: Vec<String>,
) -> ProviderCommand {
    ProviderCommand {
        provider: task.provider,
        command_kind: None,
        claude_host: None,
        command,
        args,
        stdin: None,
        redactions: vec![PROVIDER_SMOKE_PROMPT.to_string()],
        cwd: task.cwd.to_string(),
        timeout_seconds: task.timeout_seconds,
        env: BTreeMap::new(),
        profile: task.profile,
        prompt_strategy: "minimal".to_string(),
        profile_diagnostics: profile_diagnostics(task.provider, task.profile),
    }
}

pub fn smoke_command(
    provider: ProviderKind,
    cwd: &str,
    timeout_seconds: i64,
) -> Result<(ProviderCommand, &'static str), String> {
    let task = ProviderTask {
        provider,
        mode: TaskMode::Research,
        prompt: PROVIDER_SMOKE_PROMPT,
        title: None,
        cwd,
        timeout_seconds,
        model: None,
        effort: None,
        thinking: None,
        profile: LaunchProfile::Bridge,
    };
    validate_options(&task)?;
    let command = match provider {
        ProviderKind::Claude => build_claude_command(&task, PROVIDER_SMOKE_PROMPT.to_string()),
        ProviderKind::Cursor => minimal_smoke_command(
            &task,
            env_or("CURSOR_AGENT_BIN", "cursor-agent"),
            [
                vec![
                    "-p".to_string(),
                    "--output-format".to_string(),
                    "json".to_string(),
                    "--workspace".to_string(),
                    task.cwd.to_string(),
                ],
                cursor_mode_flags(task.mode),
                vec![
                    "--trust".to_string(),
                    "--".to_string(),
                    PROVIDER_SMOKE_PROMPT.to_string(),
                ],
            ]
            .concat(),
        ),
        ProviderKind::Kimi => minimal_smoke_command(
            &task,
            env_or("PI_BIN", "pi"),
            vec![
                "-p".to_string(),
                "--no-session".to_string(),
                "--no-context-files".to_string(),
                "--tools".to_string(),
                kimi_tools(task.mode).to_string(),
                PROVIDER_SMOKE_PROMPT.to_string(),
            ],
        ),
        ProviderKind::Codex => minimal_smoke_command(
            &task,
            env_or("CODEX_BIN", "codex"),
            vec![
                "exec".to_string(),
                "--cd".to_string(),
                task.cwd.to_string(),
                "--skip-git-repo-check".to_string(),
                "--json".to_string(),
                "--sandbox".to_string(),
                codex_sandbox(task.mode).to_string(),
                "--config".to_string(),
                "shell_environment_policy.inherit=\"all\"".to_string(),
                PROVIDER_SMOKE_PROMPT.to_string(),
            ],
        ),
        ProviderKind::Forge => minimal_smoke_command(
            &task,
            env_or("FORGE_BIN", "forge"),
            vec![
                "-C".to_string(),
                task.cwd.to_string(),
                "-p".to_string(),
                PROVIDER_SMOKE_PROMPT.to_string(),
            ],
        ),
        ProviderKind::Antigravity => minimal_smoke_command(
            &task,
            env_or("AGY_BIN", "agy"),
            antigravity_args(&task, PROVIDER_SMOKE_PROMPT.to_string()),
        ),
    };
    Ok((command, "minimal"))
}

pub fn build_command(task: &ProviderTask<'_>) -> Result<ProviderCommand, String> {
    validate_options(task)?;
    let rendered_prompt = render_task_prompt(task);
    Ok(adapter_for(task.provider).build_command(task, rendered_prompt))
}

/// Per-provider behavior behind a single interface, so core code dispatches
/// command construction generically instead of branching on `ProviderKind`.
/// Each provider's CLI contract lives in its own implementation.
pub trait ProviderAdapter: Sync {
    fn build_command(&self, task: &ProviderTask<'_>, rendered_prompt: String) -> ProviderCommand;

    /// Modes this provider accepts. Defaults to accepting every mode.
    fn supports_mode(&self, _mode: TaskMode) -> bool {
        true
    }

    /// Allowed `effort` values; empty means the provider rejects `effort`.
    fn supported_effort(&self) -> &'static [&'static str] {
        &[]
    }

    /// Allowed `thinking` values; empty means the provider rejects `thinking`.
    fn supported_thinking(&self) -> &'static [&'static str] {
        &[]
    }

    /// Whether this provider's stderr should be polled during execution for an
    /// early fatal-denial signal. Defaults to no.
    fn polls_stderr_for_denial(&self) -> bool {
        false
    }

    /// Whether the provider's stderr indicates a fatal denial that should fail
    /// the task even on a zero exit code. Defaults to never.
    fn detects_fatal_denial(&self, _stderr: &[u8]) -> bool {
        false
    }

    /// Whether a successful exit still requires an output-parseability check.
    /// Defaults to no.
    fn enforces_output_parseable(&self) -> bool {
        false
    }

    /// Whether the provider's stdout is acceptable. Defaults to always.
    fn output_is_acceptable(&self, _stdout: &[u8]) -> bool {
        true
    }

    /// Validate task options against this provider's declared capabilities.
    /// Shared across providers so the rules (and their messages) stay identical.
    fn validate(&self, task: &ProviderTask<'_>) -> Result<(), String> {
        if !self.supports_mode(task.mode) {
            return Err(format!(
                "{} does not support mode: {}",
                task.provider.as_str(),
                task.mode.as_str()
            ));
        }
        if let Some(effort) = task.effort
            && !self.supported_effort().contains(&effort)
        {
            return Err("effort is only supported for claude and must be one of: low, medium, high, xhigh, max".to_string());
        }
        if let Some(thinking) = task.thinking
            && !self.supported_thinking().contains(&thinking)
        {
            return Err(format!(
                "thinking is not supported for {}",
                task.provider.as_str()
            ));
        }
        Ok(())
    }
}

struct ClaudeAdapter;
struct CursorAdapter;
struct KimiAdapter;
struct CodexAdapter;
struct ForgeAdapter;
struct AntigravityAdapter;

/// Resolve the adapter for a provider. The set of providers is closed
/// (`ProviderKind` is a fixed enum), so every variant maps to a `'static`
/// adapter instance.
pub fn adapter_for(provider: ProviderKind) -> &'static dyn ProviderAdapter {
    match provider {
        ProviderKind::Claude => &ClaudeAdapter,
        ProviderKind::Cursor => &CursorAdapter,
        ProviderKind::Kimi => &KimiAdapter,
        ProviderKind::Codex => &CodexAdapter,
        ProviderKind::Forge => &ForgeAdapter,
        ProviderKind::Antigravity => &AntigravityAdapter,
    }
}

impl ProviderAdapter for ClaudeAdapter {
    fn supported_effort(&self) -> &'static [&'static str] {
        &["low", "medium", "high", "xhigh", "max"]
    }

    fn enforces_output_parseable(&self) -> bool {
        true
    }

    fn output_is_acceptable(&self, stdout: &[u8]) -> bool {
        claude_output_is_parseable(stdout)
    }

    fn build_command(&self, task: &ProviderTask<'_>, rendered_prompt: String) -> ProviderCommand {
        build_claude_command(task, rendered_prompt)
    }
}

/// A successful Claude run must emit at least one JSON line carrying a non-empty
/// `result` field; otherwise the output is treated as unparseable.
fn claude_output_is_parseable(stdout: &[u8]) -> bool {
    let text = String::from_utf8_lossy(stdout);
    text.lines().any(|line| {
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
            return false;
        };
        value
            .get("result")
            .and_then(Value::as_str)
            .is_some_and(|result| !result.is_empty())
    })
}

/// Codex can exit zero while reporting a fatal sandbox/approval/patch denial in
/// its stderr; these specific phrases identify that case.
fn codex_denial_text(stderr: &[u8]) -> bool {
    let text = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    let mentions_patch_rejection = text.contains("patch rejected");
    let mentions_outside_workspace = text.contains("outside of the project")
        || text.contains("outside the project")
        || text.contains("outside of the workspace")
        || text.contains("outside the workspace")
        || text.contains("out-of-workspace");
    let mentions_sandbox_denial = text.contains("sandbox denied")
        || text.contains("sandbox denial")
        || text.contains("sandbox permission")
        || text.contains("sandbox permissions")
        || text.contains("sandbox policy");
    let mentions_approval_denial = text.contains("approval denied")
        || text.contains("approval denial")
        || text.contains("rejected by approval")
        || text.contains("rejected by user approval")
        || text.contains("user approval settings")
        || text.contains("user denied approval");
    let mentions_workspace_trust_denial =
        text.contains("non-git workspace") && text.contains("untrusted");

    mentions_patch_rejection
        || mentions_outside_workspace
        || mentions_sandbox_denial
        || mentions_approval_denial
        || mentions_workspace_trust_denial
}

impl ProviderAdapter for CursorAdapter {
    fn supports_mode(&self, mode: TaskMode) -> bool {
        mode != TaskMode::Command
    }

    fn build_command(&self, task: &ProviderTask<'_>, rendered_prompt: String) -> ProviderCommand {
        ProviderCommand {
            provider: task.provider,
            command_kind: None,
            claude_host: None,
            command: env_or("CURSOR_AGENT_BIN", "cursor-agent"),
            args: [
                vec![
                    "-p".to_string(),
                    "--output-format".to_string(),
                    "json".to_string(),
                    "--workspace".to_string(),
                    task.cwd.to_string(),
                ],
                cursor_mode_flags(task.mode),
                optional_arg("--model", task.model),
                vec!["--trust".to_string(), "--".to_string(), rendered_prompt],
            ]
            .concat(),
            stdin: None,
            redactions: Vec::new(),
            cwd: task.cwd.to_string(),
            timeout_seconds: task.timeout_seconds,
            env: BTreeMap::new(),
            profile: task.profile,
            prompt_strategy: prompt_strategy(task.profile).to_string(),
            profile_diagnostics: profile_diagnostics(task.provider, task.profile),
        }
    }
}

impl ProviderAdapter for KimiAdapter {
    fn supported_thinking(&self) -> &'static [&'static str] {
        &["off", "minimal", "low", "medium", "high", "xhigh"]
    }

    fn build_command(&self, task: &ProviderTask<'_>, rendered_prompt: String) -> ProviderCommand {
        ProviderCommand {
            provider: task.provider,
            command_kind: None,
            claude_host: None,
            command: env_or("PI_BIN", "pi"),
            args: [
                vec![
                    "-p".to_string(),
                    "--no-session".to_string(),
                    "--no-context-files".to_string(),
                    "--tools".to_string(),
                    kimi_tools(task.mode).to_string(),
                ],
                kimi_profile_flags(task.profile),
                optional_arg("--model", task.model),
                optional_arg("--thinking", task.thinking),
                vec![rendered_prompt],
            ]
            .concat(),
            stdin: None,
            redactions: Vec::new(),
            cwd: task.cwd.to_string(),
            timeout_seconds: task.timeout_seconds,
            env: BTreeMap::new(),
            profile: task.profile,
            prompt_strategy: prompt_strategy(task.profile).to_string(),
            profile_diagnostics: profile_diagnostics(task.provider, task.profile),
        }
    }
}

impl ProviderAdapter for CodexAdapter {
    fn supported_thinking(&self) -> &'static [&'static str] {
        &["low", "medium", "high", "xhigh"]
    }

    fn polls_stderr_for_denial(&self) -> bool {
        true
    }

    fn detects_fatal_denial(&self, stderr: &[u8]) -> bool {
        codex_denial_text(stderr)
    }

    fn build_command(&self, task: &ProviderTask<'_>, rendered_prompt: String) -> ProviderCommand {
        ProviderCommand {
            provider: task.provider,
            command_kind: None,
            claude_host: None,
            command: env_or("CODEX_BIN", "codex"),
            args: [
                vec![
                    "exec".to_string(),
                    "--cd".to_string(),
                    task.cwd.to_string(),
                    "--skip-git-repo-check".to_string(),
                    "--json".to_string(),
                    "--sandbox".to_string(),
                    codex_sandbox(task.mode).to_string(),
                    "--config".to_string(),
                    "shell_environment_policy.inherit=\"all\"".to_string(),
                ],
                codex_profile_flags(task.profile),
                optional_arg("--model", task.model),
                task.thinking
                    .map(|thinking| {
                        vec![
                            "--config".to_string(),
                            format!("model_reasoning_effort=\"{thinking}\""),
                        ]
                    })
                    .unwrap_or_default(),
                vec![rendered_prompt.clone()],
            ]
            .concat(),
            stdin: None,
            redactions: vec![rendered_prompt, task.prompt.to_string()],
            cwd: task.cwd.to_string(),
            timeout_seconds: task.timeout_seconds,
            env: BTreeMap::new(),
            profile: task.profile,
            prompt_strategy: prompt_strategy(task.profile).to_string(),
            profile_diagnostics: profile_diagnostics(task.provider, task.profile),
        }
    }
}

impl ProviderAdapter for ForgeAdapter {
    fn build_command(&self, task: &ProviderTask<'_>, rendered_prompt: String) -> ProviderCommand {
        ProviderCommand {
            provider: task.provider,
            command_kind: None,
            claude_host: None,
            command: env_or("FORGE_BIN", "forge"),
            args: vec![
                "-C".to_string(),
                task.cwd.to_string(),
                "-p".to_string(),
                rendered_prompt.clone(),
            ],
            stdin: None,
            redactions: vec![rendered_prompt, task.prompt.to_string()],
            cwd: task.cwd.to_string(),
            timeout_seconds: task.timeout_seconds,
            env: BTreeMap::new(),
            profile: task.profile,
            prompt_strategy: prompt_strategy(task.profile).to_string(),
            profile_diagnostics: profile_diagnostics(task.provider, task.profile),
        }
    }
}

impl ProviderAdapter for AntigravityAdapter {
    fn build_command(&self, task: &ProviderTask<'_>, rendered_prompt: String) -> ProviderCommand {
        ProviderCommand {
            provider: task.provider,
            command_kind: None,
            claude_host: None,
            command: env_or("AGY_BIN", "agy"),
            args: antigravity_args(task, rendered_prompt.clone()),
            stdin: None,
            redactions: vec![rendered_prompt, task.prompt.to_string()],
            cwd: task.cwd.to_string(),
            timeout_seconds: task.timeout_seconds,
            env: BTreeMap::new(),
            profile: task.profile,
            prompt_strategy: prompt_strategy(task.profile).to_string(),
            profile_diagnostics: profile_diagnostics(task.provider, task.profile),
        }
    }
}

pub fn provider_env(provider: ProviderKind) -> BTreeMap<String, String> {
    let names = match provider {
        ProviderKind::Claude => &[
            "PATH",
            "HOME",
            "TMPDIR",
            "TERM",
            "COLORTERM",
            "USER",
            "LOGNAME",
            "SHELL",
            "LANG",
            "LC_ALL",
            "LC_CTYPE",
            "XDG_CONFIG_DIRS",
            "XDG_DATA_DIRS",
            "NIX_PROFILES",
            "NIX_SSL_CERT_FILE",
            "NIX_USER_PROFILE_DIR",
            "SSL_CERT_FILE",
            "CLAUDE_CONFIG_DIR",
            "CLAUDE_BIN",
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_OAUTH_TOKEN",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "AGENT_BRIDGE_WORKSPACES",
            "AGENT_BRIDGE_STATE_DIR",
        ][..],
        _ => &[
            "PATH",
            "HOME",
            "TMPDIR",
            "TERM",
            "COLORTERM",
            "USER",
            "LOGNAME",
            "SHELL",
            "LANG",
            "LC_ALL",
            "CLAUDE_CONFIG_DIR",
            "CLAUDE_BIN",
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_OAUTH_TOKEN",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "ANTHROPIC_BASE_URL",
            "CURSOR_AGENT_BIN",
            "CURSOR_API_KEY",
            "PI_BIN",
            "PI_CODING_AGENT_DIR",
            "PI_CODING_AGENT_SESSION_DIR",
            "KIMI_API_KEY",
            "FIREWORKS_API_KEY",
            "GEMINI_API_KEY",
            "OPENROUTER_API_KEY",
            "TOGETHER_API_KEY",
            "OPENAI_BASE_URL",
            "CODEX_BIN",
            "CODEX_HOME",
            "FORGE_BIN",
            "FORGE_HOME",
            "AGY_BIN",
            "OPENAI_API_KEY",
            "AGENT_BRIDGE_WORKSPACES",
            "AGENT_BRIDGE_STATE_DIR",
        ][..],
    };
    names
        .iter()
        .filter_map(|name| {
            env::var(name)
                .ok()
                .map(|value| ((*name).to_string(), value))
        })
        .collect()
}

fn build_claude_command(task: &ProviderTask<'_>, rendered_prompt: String) -> ProviderCommand {
    ProviderCommand {
        provider: task.provider,
        command_kind: Some("owned-interactive-claude".to_string()),
        claude_host: Some(ClaudeHostCommand {
            cwd: task.cwd.to_string(),
            timeout_seconds: task.timeout_seconds,
            mode: task.mode,
            prompt: rendered_prompt.clone(),
            model: task.model.map(str::to_string),
            effort: task.effort.map(str::to_string),
        }),
        command: "agent-bridge-claude-host-runner-required".to_string(),
        args: Vec::new(),
        stdin: None,
        redactions: vec![rendered_prompt, task.prompt.to_string()],
        cwd: task.cwd.to_string(),
        timeout_seconds: task.timeout_seconds,
        env: BTreeMap::new(),
        profile: task.profile,
        prompt_strategy: prompt_strategy(task.profile).to_string(),
        profile_diagnostics: profile_diagnostics(task.provider, task.profile),
    }
}

fn provider_command_kind(provider: ProviderKind) -> Option<String> {
    match provider {
        ProviderKind::Claude => Some("owned-interactive-claude".to_string()),
        _ => None,
    }
}

fn resolve_command(provider: ProviderKind) -> String {
    match provider {
        ProviderKind::Claude => env_or("CLAUDE_BIN", "claude"),
        ProviderKind::Cursor => env_or("CURSOR_AGENT_BIN", "cursor-agent"),
        ProviderKind::Kimi => env_or("PI_BIN", "pi"),
        ProviderKind::Codex => env_or("CODEX_BIN", "codex"),
        ProviderKind::Forge => env_or("FORGE_BIN", "forge"),
        ProviderKind::Antigravity => env_or("AGY_BIN", "agy"),
    }
}

fn render_task_prompt(task: &ProviderTask<'_>) -> String {
    if task.profile == LaunchProfile::Bare {
        let safety = match task.mode {
            TaskMode::Research | TaskMode::Review => "Do not edit files.",
            TaskMode::Implement => "Make only the requested code changes.",
            TaskMode::Command => "Run only bounded command-oriented work.",
        };
        return format!(
            "Delegated task.\nMode: {}\nProvider: {}\nCwd: {}\n{}\nReturn: summary, evidence, changed files if any, risks, next steps.\n\nUser instruction:\n{}",
            task.mode.as_str(),
            task.provider.as_str(),
            task.cwd,
            safety,
            task.prompt
        );
    }
    let title = task
        .title
        .map(|title| format!("Title: {title}\n"))
        .unwrap_or_default();
    format!(
        "{title}Mode: {}\nProvider: {}\nInstruction: {}\n\n{}\n\nReturn a concise final report with: summary, changed files if any, evidence, risks, and next steps.",
        task.mode.as_str(),
        task.provider.as_str(),
        mode_description(task.mode),
        task.prompt
    )
}

fn prompt_strategy(profile: LaunchProfile) -> &'static str {
    match profile {
        LaunchProfile::Bridge => "bridge",
        LaunchProfile::Bare => "compact",
    }
}

fn codex_profile_flags(profile: LaunchProfile) -> Vec<String> {
    match profile {
        LaunchProfile::Bridge => Vec::new(),
        LaunchProfile::Bare => vec![
            "--ignore-user-config".to_string(),
            "--ignore-rules".to_string(),
            "--ephemeral".to_string(),
        ],
    }
}

fn kimi_profile_flags(profile: LaunchProfile) -> Vec<String> {
    match profile {
        LaunchProfile::Bridge => Vec::new(),
        LaunchProfile::Bare => vec![
            "--no-extensions".to_string(),
            "--no-skills".to_string(),
            "--no-prompt-templates".to_string(),
            "--no-themes".to_string(),
            "--system-prompt".to_string(),
            "You are a delegated provider task. Follow the user instruction exactly.".to_string(),
        ],
    }
}

pub fn profile_diagnostics(provider: ProviderKind, profile: LaunchProfile) -> Value {
    if profile == LaunchProfile::Bridge {
        return json!({
            "profile": "bridge",
            "promptStrategy": "bridge",
            "appliedReductions": [],
            "unsupportedReductions": [],
            "bestEffortReductions": [],
            "note": "standard Agent Bridge prompt and provider configuration"
        });
    }
    match provider {
        ProviderKind::Codex => json!({
            "profile": "bare",
            "promptStrategy": "compact",
            "appliedReductions": ["compact_prompt", "ignore_user_config", "ignore_rules", "ephemeral_session"],
            "unsupportedReductions": ["custom_system_prompt", "disable_hooks"],
            "bestEffortReductions": ["context_files"],
            "note": "bare means provider-specific reduced configuration; inspect applied reductions"
        }),
        ProviderKind::Forge => json!({
            "profile": "bare",
            "promptStrategy": "compact",
            "appliedReductions": ["compact_prompt"],
            "unsupportedReductions": ["custom_system_prompt", "disable_hooks"],
            "bestEffortReductions": ["disable_skills", "config_isolation", "memory_session", "context_files"],
            "note": "forge bare uses compact prompting; CLI help does not expose reliable flags for disabling ambient settings"
        }),
        ProviderKind::Kimi => json!({
            "profile": "bare",
            "promptStrategy": "compact",
            "appliedReductions": ["compact_prompt", "custom_system_prompt", "no_session", "no_extensions", "no_skills", "no_prompt_templates", "no_themes", "no_context_files"],
            "unsupportedReductions": [],
            "bestEffortReductions": [],
            "note": "bare means provider-specific reduced configuration; inspect applied reductions"
        }),
        ProviderKind::Claude => json!({
            "profile": "bare",
            "promptStrategy": "compact",
            "appliedReductions": ["compact_prompt", "custom_system_prompt"],
            "unsupportedReductions": [],
            "bestEffortReductions": ["setting_sources", "disable_hooks", "disable_skills", "context_files"],
            "note": "owned interactive Claude injects runner-owned lifecycle hooks; bare is best-effort for hook reduction"
        }),
        ProviderKind::Cursor => json!({
            "profile": "bare",
            "promptStrategy": "compact",
            "appliedReductions": ["compact_prompt"],
            "unsupportedReductions": ["custom_system_prompt", "disable_hooks"],
            "bestEffortReductions": ["disable_skills", "config_isolation", "context_files"],
            "note": "cursor-agent exposes limited reduced-configuration flags"
        }),
        ProviderKind::Antigravity => json!({
            "profile": "bare",
            "promptStrategy": "compact",
            "appliedReductions": ["compact_prompt"],
            "unsupportedReductions": ["custom_system_prompt", "disable_hooks", "disable_skills"],
            "bestEffortReductions": ["config_isolation", "memory_session", "context_files"],
            "note": "antigravity bare uses compact prompting; CLI help does not expose reliable print-mode flags for disabling ambient settings"
        }),
    }
}

fn mode_description(mode: TaskMode) -> &'static str {
    match mode {
        TaskMode::Research => "Read and analyze. Do not edit files.",
        TaskMode::Review => "Review the requested code or plan. Do not edit files.",
        TaskMode::Implement => {
            "Make the requested code changes, keep scope tight, and report verification evidence."
        }
        TaskMode::Command => "Run the requested bounded command-oriented task and report evidence.",
    }
}

fn cursor_mode_flags(mode: TaskMode) -> Vec<String> {
    match mode {
        TaskMode::Research | TaskMode::Review => vec!["--mode".to_string(), "ask".to_string()],
        _ => Vec::new(),
    }
}

fn kimi_tools(mode: TaskMode) -> &'static str {
    match mode {
        TaskMode::Implement => "read,bash,edit,write,grep,find,ls",
        TaskMode::Command => "read,bash,grep,find,ls",
        _ => "read,grep,find,ls",
    }
}

fn codex_sandbox(mode: TaskMode) -> &'static str {
    match mode {
        TaskMode::Research | TaskMode::Review => "read-only",
        _ => "workspace-write",
    }
}

fn antigravity_args(task: &ProviderTask<'_>, prompt: String) -> Vec<String> {
    [
        vec![
            "--print-timeout".to_string(),
            format!("{}s", task.timeout_seconds),
        ],
        optional_arg("--model", task.model),
        antigravity_sandbox_flags(task.mode),
        vec!["--print".to_string(), prompt],
    ]
    .concat()
}

fn antigravity_sandbox_flags(mode: TaskMode) -> Vec<String> {
    match mode {
        TaskMode::Research | TaskMode::Review => vec!["--sandbox".to_string()],
        TaskMode::Implement | TaskMode::Command => Vec::new(),
    }
}

fn optional_arg(flag: &str, value: Option<&str>) -> Vec<String> {
    value
        .map(|value| vec![flag.to_string(), value.to_string()])
        .unwrap_or_default()
}

fn env_or(name: &str, fallback: &str) -> String {
    env::var(name).unwrap_or_else(|_| fallback.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(provider: ProviderKind, mode: TaskMode) -> ProviderTask<'static> {
        ProviderTask {
            provider,
            mode,
            prompt: "do the thing",
            title: None,
            cwd: "/tmp/work",
            timeout_seconds: 30,
            model: None,
            effort: None,
            thinking: None,
            profile: LaunchProfile::Bridge,
        }
    }

    #[test]
    fn minimal_smoke_command_fills_shared_boilerplate() {
        let t = task(ProviderKind::Codex, TaskMode::Research);
        let command = minimal_smoke_command(&t, "mybin".to_string(), vec!["exec".to_string()]);

        assert_eq!(command.provider, ProviderKind::Codex);
        assert_eq!(command.command, "mybin");
        assert_eq!(command.args, vec!["exec".to_string()]);
        assert_eq!(command.prompt_strategy, "minimal");
        assert_eq!(command.cwd, "/tmp/work");
        assert_eq!(command.timeout_seconds, 30);
        assert!(command.env.is_empty());
        assert!(command.stdin.is_none());
        assert!(command.command_kind.is_none());
        assert!(command.claude_host.is_none());
        assert!(
            command
                .redactions
                .iter()
                .any(|r| r == PROVIDER_SMOKE_PROMPT)
        );
    }

    #[test]
    fn build_command_dispatches_to_each_provider() {
        let cases = [
            (ProviderKind::Cursor, "cursor-agent"),
            (ProviderKind::Kimi, "pi"),
            (ProviderKind::Codex, "codex"),
            (ProviderKind::Forge, "forge"),
            (ProviderKind::Antigravity, "agy"),
        ];
        for (provider, expected) in cases {
            let command = build_command(&task(provider, TaskMode::Research)).unwrap();
            assert_eq!(command.provider, provider);
            assert_eq!(command.command, expected);
        }
        // Claude routes through the owned interactive host runner.
        let claude = build_command(&task(ProviderKind::Claude, TaskMode::Research)).unwrap();
        assert_eq!(
            claude.command_kind.as_deref(),
            Some("owned-interactive-claude")
        );
    }

    #[test]
    fn codex_build_command_carries_sandbox_and_thinking() {
        let mut t = task(ProviderKind::Codex, TaskMode::Implement);
        t.thinking = Some("high");
        let command = build_command(&t).unwrap();
        assert!(command.args.iter().any(|arg| arg == "exec"));
        assert!(
            command
                .args
                .iter()
                .any(|arg| arg == "--skip-git-repo-check")
        );
        assert!(command.args.iter().any(|arg| arg == "workspace-write"));
        assert!(
            command
                .args
                .iter()
                .any(|arg| arg == "model_reasoning_effort=\"high\"")
        );
    }

    #[test]
    fn validate_options_enforces_provider_rules() {
        // Cursor rejects command mode.
        assert!(validate_options(&task(ProviderKind::Cursor, TaskMode::Command)).is_err());
        // effort only for claude.
        let mut codex = task(ProviderKind::Codex, TaskMode::Research);
        codex.effort = Some("high");
        assert!(validate_options(&codex).is_err());
        let mut claude = task(ProviderKind::Claude, TaskMode::Research);
        claude.effort = Some("high");
        assert!(validate_options(&claude).is_ok());
        // thinking rules per provider.
        let mut kimi = task(ProviderKind::Kimi, TaskMode::Research);
        kimi.thinking = Some("off");
        assert!(validate_options(&kimi).is_ok());
        kimi.thinking = Some("nonsense");
        assert!(validate_options(&kimi).is_err());
        let mut cursor = task(ProviderKind::Cursor, TaskMode::Research);
        cursor.thinking = Some("low");
        assert!(validate_options(&cursor).is_err());
    }

    #[test]
    fn codex_adapter_detects_fatal_denial_via_trait() {
        let adapter = adapter_for(ProviderKind::Codex);
        assert!(adapter.polls_stderr_for_denial());
        for stderr in [
            "patch rejected",
            "Patch rejected: file is outside the workspace",
            "write outside of the project",
            "sandbox denied",
            "sandbox permission blocked command",
            "approval denied",
            "rejected by user approval settings",
            "refusing to run in a non-git workspace because it is untrusted",
        ] {
            assert!(
                adapter.detects_fatal_denial(stderr.as_bytes()),
                "expected fatal Codex denial for: {stderr}"
            );
        }
        for stderr in [
            "sandbox connection denied by proxy",
            "permission denied while reading cache",
            "approval requested",
            "patch failed to apply cleanly",
        ] {
            assert!(
                !adapter.detects_fatal_denial(stderr.as_bytes()),
                "unexpected fatal Codex denial for: {stderr}"
            );
        }
        // Non-Codex providers never poll or detect denial.
        assert!(!adapter_for(ProviderKind::Kimi).polls_stderr_for_denial());
        assert!(!adapter_for(ProviderKind::Kimi).detects_fatal_denial(b"patch rejected"));
    }

    #[test]
    fn claude_adapter_enforces_output_parseability_via_trait() {
        let adapter = adapter_for(ProviderKind::Claude);
        assert!(adapter.enforces_output_parseable());
        assert!(adapter.output_is_acceptable(b"{\"result\":\"done\"}"));
        assert!(!adapter.output_is_acceptable(b"not json"));
        assert!(!adapter.output_is_acceptable(b"{\"result\":\"\"}"));
        // Other providers do not enforce parseability.
        assert!(!adapter_for(ProviderKind::Codex).enforces_output_parseable());
        assert!(adapter_for(ProviderKind::Codex).output_is_acceptable(b"anything"));
    }
}
