use crate::claude_host::ClaudeHostCommand;
use crate::domain::{FailureCategory, LaunchProfile, ProviderKind, TaskMode};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::env;

const PROVIDER_SMOKE_PROMPT: &str = "Reply with exactly: AGENT_BRIDGE_PROVIDER_SMOKE_OK";
const UNBLOCKED_SMOKE_MARKER: &str = ".agent-bridge-unblocked-smoke";
pub const PROVIDER_SMOKE_TOKEN: &str = "AGENT_BRIDGE_PROVIDER_SMOKE_OK";
const LEAN_RETURN_CONTRACT: &str = concat!(
    "Return contract:\n",
    "- Return only the task-relevant final answer.\n",
    "- Do not echo source text, narrate progress/polling/waiting, include generic checklists, ",
    "speculate or polish, or restate the prompt unless explicitly asked.\n",
    "- If blocked, return only the blocker and the one missing fact needed to proceed.\n",
    "- Include changed files, verification evidence, risks, blockers, or next steps only when ",
    "they exist; omit empty sections."
);
const STANDARD_PROFILES: &[LaunchProfile] = &[LaunchProfile::Bridge, LaunchProfile::Bare];
const UNBLOCKED_PROFILES: &[LaunchProfile] = &[
    LaunchProfile::Bridge,
    LaunchProfile::Bare,
    LaunchProfile::Unblocked,
];

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

impl ProviderCommand {
    pub fn is_acp(&self) -> bool {
        self.command_kind.as_deref() == Some("acp")
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptanceReport {
    pub acceptable: bool,
    pub reason: Option<String>,
    pub category: Option<FailureCategory>,
}

impl AcceptanceReport {
    pub fn accepted() -> Self {
        Self {
            acceptable: true,
            reason: None,
            category: None,
        }
    }
}

pub fn capabilities() -> Value {
    json!({
        "claude": {
            "modes": ["research", "review", "implement", "command"],
            "supportsReply": false,
            "supportsResume": false,
            "supportsWorktreeIsolation": true,
            "effort": ["low", "medium", "high", "xhigh", "max"],
            "launchProfiles": provider_launch_profiles(ProviderKind::Claude),
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
            "launchProfiles": provider_launch_profiles(ProviderKind::Cursor),
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
            "launchProfiles": provider_launch_profiles(ProviderKind::Kimi),
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
            "effort": ["low", "medium", "high", "xhigh"],
            "thinking": ["low", "medium", "high", "xhigh"],
            "launchProfiles": provider_launch_profiles(ProviderKind::Codex),
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
            "launchProfiles": provider_launch_profiles(ProviderKind::Forge),
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
            "launchProfiles": provider_launch_profiles(ProviderKind::Antigravity),
            "presentationActions": presentation_actions(),
            "outputCadence": output_cadence(ProviderKind::Antigravity),
            "readiness": default_readiness(),
            "reducedConfiguration": reduced_configuration(ProviderKind::Antigravity),
            "readOnlyEnforcement": {
                "research": "prompt_enforced",
                "review": "prompt_enforced",
                "note": "--sandbox used; read-only filesystem enforcement unverified."
            }
        }
    })
}

pub fn provider_launch_profiles(provider: ProviderKind) -> Vec<&'static str> {
    adapter_for(provider)
        .supported_profiles()
        .iter()
        .map(|profile| profile.as_str())
        .collect()
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
            "note": "May be silent until final JSON."
        }),
        ProviderKind::Claude => json!({
            "cadence": "provider_dependent",
            "firstOutputExpected": "unknown",
            "recommendedPollMs": 30000,
            "recommendedSilentBudgetMs": 120000,
            "fallbackAfterMs": 180000,
            "advisory": true,
            "note": "Varies by launch strategy/host runner."
        }),
        ProviderKind::Kimi => json!({
            "cadence": "provider_dependent",
            "firstOutputExpected": "intermittent",
            "recommendedPollMs": 30000,
            "recommendedSilentBudgetMs": 120000,
            "fallbackAfterMs": 180000,
            "advisory": true,
            "note": "Provider-dependent."
        }),
        ProviderKind::Codex => json!({
            "cadence": "provider_dependent",
            "firstOutputExpected": "intermittent",
            "recommendedPollMs": 30000,
            "recommendedSilentBudgetMs": 120000,
            "fallbackAfterMs": 180000,
            "advisory": true,
            "note": "Provider-dependent."
        }),
        ProviderKind::Forge => json!({
            "cadence": "provider_dependent",
            "firstOutputExpected": "intermittent",
            "recommendedPollMs": 30000,
            "recommendedSilentBudgetMs": 120000,
            "fallbackAfterMs": 180000,
            "advisory": true,
            "note": "Provider-dependent."
        }),
        ProviderKind::Antigravity => json!({
            "cadence": "provider_dependent",
            "firstOutputExpected": "intermittent",
            "recommendedPollMs": 30000,
            "recommendedSilentBudgetMs": 120000,
            "fallbackAfterMs": 180000,
            "advisory": true,
            "note": "Provider-dependent."
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
    acp_command_config(task.provider)?;
    adapter_for(task.provider).validate(task)
}

pub fn version_command(provider: ProviderKind) -> Result<ProviderCommand, String> {
    let (command, args) = acp_command_config(provider)?;
    Ok(ProviderCommand {
        provider,
        command_kind: Some("acp".to_string()),
        claude_host: None,
        command,
        args: [args, vec!["--version".to_string()]].concat(),
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
    })
}

fn acp_smoke_command(task: &ProviderTask) -> Result<ProviderCommand, String> {
    let (command, args) = acp_command_config(task.provider)?;
    let prompt = smoke_prompt(task.profile);
    Ok(ProviderCommand {
        provider: task.provider,
        command_kind: Some("acp".to_string()),
        claude_host: None,
        command,
        args: acp_args_with_profile(task.provider, task.profile, args),
        stdin: Some(prompt.clone()),
        redactions: vec![prompt],
        cwd: task.cwd.to_string(),
        timeout_seconds: task.timeout_seconds,
        env: BTreeMap::new(),
        profile: task.profile,
        prompt_strategy: format!("{}-smoke", prompt_strategy(task.profile)),
        profile_diagnostics: profile_diagnostics(task.provider, task.profile),
    })
}

pub fn smoke_command(
    provider: ProviderKind,
    cwd: &str,
    timeout_seconds: i64,
    profile: LaunchProfile,
) -> Result<(ProviderCommand, &'static str), String> {
    let prompt = smoke_prompt(profile);
    let task = ProviderTask {
        provider,
        mode: TaskMode::Research,
        prompt: &prompt,
        title: None,
        cwd,
        timeout_seconds,
        model: None,
        effort: None,
        thinking: None,
        profile,
    };
    validate_options(&task)?;
    Ok((acp_smoke_command(&task)?, prompt_strategy(profile)))
}

pub fn build_command(task: &ProviderTask<'_>) -> Result<ProviderCommand, String> {
    validate_options(task)?;
    let rendered_prompt = render_task_prompt(task);
    let (command, args) = acp_command_config(task.provider)?;
    Ok(build_acp_command(task, rendered_prompt, command, args))
}

fn smoke_prompt(profile: LaunchProfile) -> String {
    match profile {
        LaunchProfile::Unblocked => format!(
            "Workspace permission smoke test. In the current working directory, create a file named {UNBLOCKED_SMOKE_MARKER} containing exactly agent-bridge-smoke, read it back, delete it, then reply with exactly: {PROVIDER_SMOKE_TOKEN}"
        ),
        LaunchProfile::Bridge | LaunchProfile::Bare => PROVIDER_SMOKE_PROMPT.to_string(),
    }
}

/// Per-provider behavior behind a single interface, so core code dispatches
/// command construction generically instead of branching on `ProviderKind`.
/// Each provider's CLI contract lives in its own implementation.
pub trait ProviderAdapter: Sync {
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

    /// Launch profiles this provider can build. Defaults to standard bridge and
    /// reduced bare launch; adapters opt into unblocked only with known flags.
    fn supported_profiles(&self) -> &'static [LaunchProfile] {
        STANDARD_PROFILES
    }

    /// Additional ACP CLI arguments required for a launch profile.
    fn profile_args(&self, _profile: LaunchProfile) -> &'static [&'static str] {
        &[]
    }

    fn validate_mode_and_profile(&self, task: &ProviderTask<'_>) -> Result<(), String> {
        if !self.supports_mode(task.mode) {
            return Err(format!(
                "{} does not support mode: {}",
                task.provider.as_str(),
                task.mode.as_str()
            ));
        }
        if !self.supported_profiles().contains(&task.profile) {
            return Err(format!(
                "{} does not support profile: {}",
                task.provider.as_str(),
                task.profile.as_str()
            ));
        }
        Ok(())
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

    /// Whether captured output is acceptable for a zero-exit provider run.
    fn acceptance_report(&self, _stdout: &[u8], _stderr: &[u8]) -> AcceptanceReport {
        AcceptanceReport::accepted()
    }

    /// Human-readable acceptance criteria for previews and diagnostics.
    fn acceptance_criteria(&self) -> &'static str {
        "exit 0 accepted"
    }

    /// Validate task options against this provider's declared capabilities.
    /// Shared across providers so the rules (and their messages) stay identical.
    fn validate(&self, task: &ProviderTask<'_>) -> Result<(), String> {
        self.validate_mode_and_profile(task)?;
        if let Some(effort) = task.effort {
            let supported_effort = self.supported_effort();
            if supported_effort.is_empty() {
                return Err(format!(
                    "effort is not supported for {}",
                    task.provider.as_str()
                ));
            }
            if !supported_effort.contains(&effort) {
                return Err(format!(
                    "{} effort must be one of: {}",
                    task.provider.as_str(),
                    supported_effort.join(", ")
                ));
            }
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

fn build_acp_command(
    task: &ProviderTask<'_>,
    rendered_prompt: String,
    command: String,
    args: Vec<String>,
) -> ProviderCommand {
    ProviderCommand {
        provider: task.provider,
        command_kind: Some("acp".to_string()),
        claude_host: None,
        command,
        args: acp_args_with_profile(task.provider, task.profile, args),
        stdin: Some(rendered_prompt.clone()),
        redactions: vec![rendered_prompt, task.prompt.to_string()],
        cwd: task.cwd.to_string(),
        timeout_seconds: task.timeout_seconds,
        env: BTreeMap::new(),
        profile: task.profile,
        prompt_strategy: prompt_strategy(task.profile).to_string(),
        profile_diagnostics: profile_diagnostics(task.provider, task.profile),
    }
}

fn acp_args_with_profile(
    provider: ProviderKind,
    profile: LaunchProfile,
    mut args: Vec<String>,
) -> Vec<String> {
    args.extend(
        adapter_for(provider)
            .profile_args(profile)
            .iter()
            .map(|arg| (*arg).to_string()),
    );
    args
}

fn acp_command_config(provider: ProviderKind) -> Result<(String, Vec<String>), String> {
    acp_command_config_with(provider, |name| env::var(name).ok())
}

fn acp_command_config_with(
    provider: ProviderKind,
    get_env: impl Fn(&str) -> Option<String>,
) -> Result<(String, Vec<String>), String> {
    let (bin_var, args_var, default_command, default_args): (
        &str,
        &str,
        Option<&str>,
        Vec<String>,
    ) = match provider {
        ProviderKind::Claude => (
            "CLAUDE_ACP_BIN",
            "CLAUDE_ACP_ARGS",
            Some("claude-agent"),
            vec![],
        ),
        ProviderKind::Kimi => (
            "KIMI_ACP_BIN",
            "KIMI_ACP_ARGS",
            Some("kimi"),
            vec!["acp".to_string()],
        ),
        ProviderKind::Codex => ("CODEX_ACP_BIN", "CODEX_ACP_ARGS", None, vec![]),
        ProviderKind::Cursor => ("CURSOR_ACP_BIN", "CURSOR_ACP_ARGS", None, vec![]),
        ProviderKind::Forge => ("FORGE_ACP_BIN", "FORGE_ACP_ARGS", None, vec![]),
        ProviderKind::Antigravity => ("ANTIGRAVITY_ACP_BIN", "ANTIGRAVITY_ACP_ARGS", None, vec![]),
    };
    let command = get_env(bin_var)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| default_command.map(str::to_string))
        .ok_or_else(|| {
            format!(
                "{bin_var} is required for {} ACP launches",
                provider.as_str()
            )
        })?;
    let mut args = default_args;
    if let Some(extra_args) = get_env(args_var).filter(|value| !value.trim().is_empty()) {
        args.extend(split_env_args(&extra_args)?);
    }
    Ok((command, args))
}

fn split_env_args(input: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    for ch in input.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '\'' | '"' if quote == Some(ch) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(ch),
            ch if ch.is_whitespace() && quote.is_none() => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            ch => current.push(ch),
        }
    }
    if escaped {
        current.push('\\');
    }
    if quote.is_some() {
        return Err("ACP args contain an unterminated quote".to_string());
    }
    if !current.is_empty() {
        args.push(current);
    }
    Ok(args)
}

impl ProviderAdapter for ClaudeAdapter {
    fn supported_effort(&self) -> &'static [&'static str] {
        &["low", "medium", "high", "xhigh", "max"]
    }

    fn acceptance_report(&self, stdout: &[u8], _stderr: &[u8]) -> AcceptanceReport {
        if claude_output_is_parseable(stdout) {
            return AcceptanceReport::accepted();
        }
        AcceptanceReport {
            acceptable: false,
            reason: Some("claude provider output was not parseable".to_string()),
            category: Some(FailureCategory::ProviderOutputError),
        }
    }

    fn acceptance_criteria(&self) -> &'static str {
        "stdout contains a JSON line with a non-empty result field"
    }

    fn supported_profiles(&self) -> &'static [LaunchProfile] {
        UNBLOCKED_PROFILES
    }

    fn profile_args(&self, profile: LaunchProfile) -> &'static [&'static str] {
        match profile {
            LaunchProfile::Unblocked => &["--permission-mode", "bypassPermissions"],
            LaunchProfile::Bridge | LaunchProfile::Bare => &[],
        }
    }
}

/// A successful legacy Claude run must emit at least one JSON line carrying a
/// non-empty `result` field. ACP and owned-runner paths validate separately.
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
    fn acceptance_report(&self, _stdout: &[u8], _stderr: &[u8]) -> AcceptanceReport {
        AcceptanceReport::accepted()
    }

    fn acceptance_criteria(&self) -> &'static str {
        "exit 0 accepted; Cursor output validation not implemented"
    }

    fn supports_mode(&self, mode: TaskMode) -> bool {
        mode != TaskMode::Command
    }
}

impl ProviderAdapter for KimiAdapter {
    fn acceptance_report(&self, _stdout: &[u8], _stderr: &[u8]) -> AcceptanceReport {
        AcceptanceReport::accepted()
    }

    fn acceptance_criteria(&self) -> &'static str {
        "exit 0 accepted; Kimi output validation not implemented"
    }

    fn supported_thinking(&self) -> &'static [&'static str] {
        &["off", "minimal", "low", "medium", "high", "xhigh"]
    }
}

impl ProviderAdapter for CodexAdapter {
    fn supported_effort(&self) -> &'static [&'static str] {
        &["low", "medium", "high", "xhigh"]
    }

    fn supported_thinking(&self) -> &'static [&'static str] {
        &["low", "medium", "high", "xhigh"]
    }

    fn validate(&self, task: &ProviderTask<'_>) -> Result<(), String> {
        self.validate_mode_and_profile(task)?;
        if let Some(effort) = task.effort
            && !self.supported_effort().contains(&effort)
        {
            return Err("codex effort must be one of: low, medium, high, xhigh".to_string());
        }
        if let Some(thinking) = task.thinking
            && !self.supported_thinking().contains(&thinking)
        {
            return Err("codex thinking must be one of: low, medium, high, xhigh".to_string());
        }
        if let (Some(effort), Some(thinking)) = (task.effort, task.thinking)
            && effort != thinking
        {
            return Err("codex effort and thinking must match when both are set".to_string());
        }
        Ok(())
    }

    fn polls_stderr_for_denial(&self) -> bool {
        true
    }

    fn detects_fatal_denial(&self, stderr: &[u8]) -> bool {
        codex_denial_text(stderr)
    }

    fn acceptance_report(&self, _stdout: &[u8], stderr: &[u8]) -> AcceptanceReport {
        if self.detects_fatal_denial(stderr) {
            return AcceptanceReport {
                acceptable: false,
                reason: Some("Codex sandbox or approval denied".to_string()),
                category: Some(FailureCategory::ProviderSandboxDenied),
            };
        }
        AcceptanceReport::accepted()
    }

    fn acceptance_criteria(&self) -> &'static str {
        "exit 0 with no sandbox or approval denial on stderr"
    }
}

impl ProviderAdapter for ForgeAdapter {}

impl ProviderAdapter for AntigravityAdapter {
    fn acceptance_report(&self, _stdout: &[u8], _stderr: &[u8]) -> AcceptanceReport {
        AcceptanceReport::accepted()
    }

    fn acceptance_criteria(&self) -> &'static str {
        "exit 0 accepted; Antigravity output validation not implemented"
    }

    fn supported_profiles(&self) -> &'static [LaunchProfile] {
        UNBLOCKED_PROFILES
    }

    fn profile_args(&self, profile: LaunchProfile) -> &'static [&'static str] {
        match profile {
            LaunchProfile::Unblocked => &["--dangerously-skip-permissions"],
            LaunchProfile::Bridge | LaunchProfile::Bare => &[],
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
            "CLAUDE_ACP_BIN",
            "CLAUDE_ACP_ARGS",
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
            "CLAUDE_ACP_BIN",
            "CLAUDE_ACP_ARGS",
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_OAUTH_TOKEN",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "ANTHROPIC_BASE_URL",
            "CURSOR_AGENT_BIN",
            "CURSOR_ACP_BIN",
            "CURSOR_ACP_ARGS",
            "CURSOR_API_KEY",
            "PI_BIN",
            "KIMI_ACP_BIN",
            "KIMI_ACP_ARGS",
            "PI_CODING_AGENT_DIR",
            "PI_CODING_AGENT_SESSION_DIR",
            "KIMI_API_KEY",
            "FIREWORKS_API_KEY",
            "GEMINI_API_KEY",
            "OPENROUTER_API_KEY",
            "TOGETHER_API_KEY",
            "OPENAI_BASE_URL",
            "CODEX_BIN",
            "CODEX_ACP_BIN",
            "CODEX_ACP_ARGS",
            "CODEX_HOME",
            "FORGE_BIN",
            "FORGE_ACP_BIN",
            "FORGE_ACP_ARGS",
            "FORGE_HOME",
            "AGY_BIN",
            "ANTIGRAVITY_ACP_BIN",
            "ANTIGRAVITY_ACP_ARGS",
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

fn render_task_prompt(task: &ProviderTask<'_>) -> String {
    let title = task
        .title
        .map(|title| format!("Title: {title}\n"))
        .unwrap_or_default();
    let safety = match task.mode {
        TaskMode::Research | TaskMode::Review => "Do not edit files.",
        TaskMode::Implement => "Make only the requested code changes.",
        TaskMode::Command => "Run only bounded command-oriented work.",
    };
    format!(
        "{title}Delegated task.\nMode: {}\nProvider: {}\nCwd: {}\nInstruction: {}\nSafety: {}{}\n\nUser instruction:\n{}\n\n{}",
        task.mode.as_str(),
        task.provider.as_str(),
        task.cwd,
        mode_description(task.mode),
        safety,
        nested_delegation_boundary(task.mode),
        task.prompt,
        LEAN_RETURN_CONTRACT,
    )
}

fn prompt_strategy(profile: LaunchProfile) -> &'static str {
    match profile {
        LaunchProfile::Bridge => "bridge",
        LaunchProfile::Bare => "compact",
        LaunchProfile::Unblocked => "unblocked",
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
            "note": "standard provider config; lean task contract"
        });
    }
    if profile == LaunchProfile::Unblocked {
        return json!({
            "profile": "unblocked",
            "promptStrategy": "unblocked",
            "appliedReductions": [],
            "unsupportedReductions": [],
            "bestEffortReductions": [],
            "permissionBypass": match provider {
                ProviderKind::Claude => "--permission-mode bypassPermissions",
                ProviderKind::Antigravity => "--dangerously-skip-permissions",
                _ => "unsupported"
            },
            "note": "permission bypass after allowed-cwd validation"
        });
    }
    match provider {
        ProviderKind::Codex => json!({
            "profile": "bare",
            "promptStrategy": "compact",
            "appliedReductions": ["compact_prompt", "ignore_user_config", "ignore_rules", "ephemeral_session"],
            "unsupportedReductions": ["custom_system_prompt", "disable_hooks"],
            "bestEffortReductions": ["context_files"],
            "note": "provider-specific reduced config; lean contract still applies"
        }),
        ProviderKind::Forge => json!({
            "profile": "bare",
            "promptStrategy": "compact",
            "appliedReductions": ["compact_prompt"],
            "unsupportedReductions": ["custom_system_prompt", "disable_hooks"],
            "bestEffortReductions": ["disable_skills", "config_isolation", "memory_session", "context_files"],
            "note": "reduced config; ambient-setting flags limited"
        }),
        ProviderKind::Kimi => json!({
            "profile": "bare",
            "promptStrategy": "compact",
            "appliedReductions": ["compact_prompt", "custom_system_prompt", "no_session", "no_extensions", "no_skills", "no_prompt_templates", "no_themes", "no_context_files"],
            "unsupportedReductions": [],
            "bestEffortReductions": [],
            "note": "provider-specific reduced config; lean contract still applies"
        }),
        ProviderKind::Claude => json!({
            "profile": "bare",
            "promptStrategy": "compact",
            "appliedReductions": ["compact_prompt", "custom_system_prompt"],
            "unsupportedReductions": [],
            "bestEffortReductions": ["setting_sources", "disable_hooks", "disable_skills", "context_files"],
            "note": "host runner injects lifecycle hooks; hook reduction best-effort"
        }),
        ProviderKind::Cursor => json!({
            "profile": "bare",
            "promptStrategy": "compact",
            "appliedReductions": ["compact_prompt"],
            "unsupportedReductions": ["custom_system_prompt", "disable_hooks"],
            "bestEffortReductions": ["disable_skills", "config_isolation", "context_files"],
            "note": "limited reduced-config flags"
        }),
        ProviderKind::Antigravity => json!({
            "profile": "bare",
            "promptStrategy": "compact",
            "appliedReductions": ["compact_prompt"],
            "unsupportedReductions": ["custom_system_prompt", "disable_hooks", "disable_skills"],
            "bestEffortReductions": ["config_isolation", "memory_session", "context_files"],
            "note": "reduced config; inspect ACP ambient-setting support"
        }),
    }
}

fn mode_description(mode: TaskMode) -> &'static str {
    match mode {
        TaskMode::Research => "Read and analyze. Do not edit files.",
        TaskMode::Review => "Review the requested code or plan. Do not edit files.",
        TaskMode::Implement => "Make the requested code changes and keep scope tight.",
        TaskMode::Command => "Run the requested bounded command-oriented task.",
    }
}

fn nested_delegation_boundary(mode: TaskMode) -> &'static str {
    match mode {
        TaskMode::Research | TaskMode::Review => {
            "\nDo not call Agent Bridge or spawn/use other provider or subagent review tools; return your own bounded report directly."
        }
        TaskMode::Implement | TaskMode::Command => "",
    }
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
    fn read_only_prompts_prohibit_nested_delegation() {
        let review = render_task_prompt(&task(ProviderKind::Codex, TaskMode::Review));
        assert!(review.contains("Do not call Agent Bridge"));
        assert!(review.contains("return your own bounded report directly"));

        let research = render_task_prompt(&task(ProviderKind::Codex, TaskMode::Research));
        assert!(research.contains("Do not call Agent Bridge"));

        let implement = render_task_prompt(&task(ProviderKind::Codex, TaskMode::Implement));
        assert!(!implement.contains("Do not call Agent Bridge"));
    }

    #[test]
    fn prompts_use_shared_lean_return_contract_for_all_profiles() {
        for profile in [
            LaunchProfile::Bridge,
            LaunchProfile::Bare,
            LaunchProfile::Unblocked,
        ] {
            let mut t = task(ProviderKind::Claude, TaskMode::Review);
            t.profile = profile;
            let prompt = render_task_prompt(&t);

            assert!(prompt.contains("Return only the task-relevant final answer."));
            assert!(prompt.contains("Do not echo source text"));
            assert!(prompt.contains("narrate progress/polling/waiting"));
            assert!(prompt.contains("include generic checklists"));
            assert!(prompt.contains("restate the prompt unless explicitly asked"));
            assert!(prompt.contains("Include changed files, verification evidence, risks, blockers, or next steps only when they exist"));
            assert!(!prompt.contains("Return a concise final report"));
            assert!(!prompt.contains("summary, evidence, changed files if any, risks, next steps"));
        }
    }

    #[test]
    fn lean_return_contract_preserves_mode_boundaries() {
        let research = render_task_prompt(&task(ProviderKind::Codex, TaskMode::Research));
        assert!(research.contains("Do not edit files."));
        assert!(research.contains("User instruction:\ndo the thing"));

        let implement = render_task_prompt(&task(ProviderKind::Codex, TaskMode::Implement));
        assert!(implement.contains("Make only the requested code changes."));
        assert!(implement.contains("Make the requested code changes and keep scope tight."));
        assert!(implement.contains("verification evidence"));
    }

    #[test]
    fn acp_config_uses_defaults_and_required_bins() {
        let missing = |_name: &str| None;
        assert_eq!(
            acp_command_config_with(ProviderKind::Claude, missing).unwrap(),
            ("claude-agent".to_string(), vec![])
        );
        assert_eq!(
            acp_command_config_with(ProviderKind::Kimi, missing).unwrap(),
            ("kimi".to_string(), vec!["acp".to_string()])
        );
        for provider in [
            ProviderKind::Codex,
            ProviderKind::Cursor,
            ProviderKind::Forge,
            ProviderKind::Antigravity,
        ] {
            assert!(
                acp_command_config_with(provider, missing)
                    .unwrap_err()
                    .contains("_ACP_BIN is required")
            );
        }
    }

    #[test]
    fn acp_config_parses_optional_args() {
        let env = |name: &str| match name {
            "CODEX_ACP_BIN" => Some("codex-acp".to_string()),
            "CODEX_ACP_ARGS" => Some("--model \"gpt 5\" --flag".to_string()),
            _ => None,
        };
        let (command, args) = acp_command_config_with(ProviderKind::Codex, env).unwrap();
        assert_eq!(command, "codex-acp");
        assert_eq!(args, vec!["--model", "gpt 5", "--flag"]);
    }

    #[test]
    fn build_command_uses_acp_transport() {
        let t = task(ProviderKind::Codex, TaskMode::Implement);
        let command = build_acp_command(
            &t,
            "rendered prompt".to_string(),
            "codex-acp".to_string(),
            vec!["--json".to_string()],
        );

        assert_eq!(command.provider, ProviderKind::Codex);
        assert_eq!(command.command_kind.as_deref(), Some("acp"));
        assert!(command.is_acp());
        assert_eq!(command.command, "codex-acp");
        assert_eq!(command.args, vec!["--json"]);
        assert_eq!(command.stdin.as_deref(), Some("rendered prompt"));
        assert_eq!(command.prompt_strategy, "bridge");
        assert!(command.claude_host.is_none());
    }

    #[test]
    fn unblocked_profile_adds_provider_owned_args() {
        let mut claude = task(ProviderKind::Claude, TaskMode::Implement);
        claude.profile = LaunchProfile::Unblocked;
        let claude_command = build_acp_command(
            &claude,
            "rendered prompt".to_string(),
            "claude-agent".to_string(),
            vec![],
        );
        assert_eq!(
            claude_command.args,
            vec!["--permission-mode", "bypassPermissions"]
        );
        assert_eq!(claude_command.prompt_strategy, "unblocked");

        let mut antigravity = task(ProviderKind::Antigravity, TaskMode::Implement);
        antigravity.profile = LaunchProfile::Unblocked;
        let antigravity_command = build_acp_command(
            &antigravity,
            "rendered prompt".to_string(),
            "agy-acp".to_string(),
            vec!["--existing".to_string()],
        );
        assert_eq!(
            antigravity_command.args,
            vec!["--existing", "--dangerously-skip-permissions"]
        );
    }

    #[test]
    fn codex_options_accept_effort_as_thinking_alias() {
        let mut t = task(ProviderKind::Codex, TaskMode::Review);
        t.effort = Some("high");
        adapter_for(ProviderKind::Codex).validate(&t).unwrap();

        let mut conflicting = task(ProviderKind::Codex, TaskMode::Review);
        conflicting.effort = Some("high");
        conflicting.thinking = Some("low");
        let error = adapter_for(ProviderKind::Codex)
            .validate(&conflicting)
            .unwrap_err();
        assert!(error.contains("codex effort and thinking must match"));
    }

    #[test]
    fn validate_options_enforces_provider_rules() {
        // Cursor rejects command mode.
        assert!(
            adapter_for(ProviderKind::Cursor)
                .validate(&task(ProviderKind::Cursor, TaskMode::Command))
                .is_err()
        );
        // effort is accepted only where the provider has a reasoning contract.
        let mut codex = task(ProviderKind::Codex, TaskMode::Research);
        codex.effort = Some("high");
        assert!(adapter_for(ProviderKind::Codex).validate(&codex).is_ok());
        let mut claude = task(ProviderKind::Claude, TaskMode::Research);
        claude.effort = Some("high");
        assert!(validate_options(&claude).is_ok());
        let mut cursor_effort = task(ProviderKind::Cursor, TaskMode::Research);
        cursor_effort.effort = Some("high");
        assert!(
            adapter_for(ProviderKind::Cursor)
                .validate(&cursor_effort)
                .is_err()
        );
        // thinking rules per provider.
        let mut kimi = task(ProviderKind::Kimi, TaskMode::Research);
        kimi.thinking = Some("off");
        assert!(validate_options(&kimi).is_ok());
        kimi.thinking = Some("nonsense");
        assert!(validate_options(&kimi).is_err());
        let mut cursor = task(ProviderKind::Cursor, TaskMode::Research);
        cursor.thinking = Some("low");
        assert!(adapter_for(ProviderKind::Cursor).validate(&cursor).is_err());

        let mut unsupported_profile = task(ProviderKind::Codex, TaskMode::Research);
        unsupported_profile.profile = LaunchProfile::Unblocked;
        let error = adapter_for(ProviderKind::Codex)
            .validate(&unsupported_profile)
            .unwrap_err();
        assert!(error.contains("codex does not support profile: unblocked"));
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
    fn acp_adapters_do_not_parse_legacy_cli_output() {
        let codex = adapter_for(ProviderKind::Codex)
            .acceptance_report(b"{}", b"Patch rejected: file is outside the workspace");
        assert!(!codex.acceptable);
        assert_eq!(
            codex.category,
            Some(crate::domain::FailureCategory::ProviderSandboxDenied)
        );
    }

    #[test]
    fn claude_acceptance_report_requires_result_json_line() {
        let adapter = adapter_for(ProviderKind::Claude);

        let accepted = adapter.acceptance_report(b"{\"result\":\"done\"}\n", b"");
        assert!(accepted.acceptable);
        assert_eq!(accepted.reason, None);
        assert_eq!(accepted.category, None);

        let rejected = adapter.acceptance_report(b"not json\n{\"result\":\"\"}\n", b"");
        assert!(!rejected.acceptable);
        assert_eq!(
            rejected.reason.as_deref(),
            Some("claude provider output was not parseable")
        );
        assert_eq!(
            rejected.category,
            Some(crate::domain::FailureCategory::ProviderOutputError)
        );
    }

    #[test]
    fn permissive_acceptance_reports_document_current_gaps() {
        for provider in [
            ProviderKind::Cursor,
            ProviderKind::Kimi,
            ProviderKind::Antigravity,
        ] {
            let adapter = adapter_for(provider);
            let report = adapter.acceptance_report(b"not json", b"");
            assert!(report.acceptable, "{provider:?}");
            assert!(adapter.acceptance_criteria().contains("not implemented"));
        }
    }
}
