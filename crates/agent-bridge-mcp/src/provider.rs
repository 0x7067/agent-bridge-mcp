use crate::claude_host::ClaudeHostCommand;
use crate::domain::{ProviderKind, TaskMode};
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
}

pub fn capabilities() -> Value {
    json!({
        "claude": {
            "modes": ["research", "review", "implement", "command"],
            "supportsReply": false,
            "supportsResume": false,
            "supportsWorktreeIsolation": true,
            "effort": ["low", "medium", "high", "xhigh", "max"]
        },
        "cursor": {
            "modes": ["research", "review", "implement"],
            "supportsReply": false,
            "supportsResume": false,
            "supportsWorktreeIsolation": true
        },
        "kimi": {
            "modes": ["research", "review", "implement", "command"],
            "supportsReply": false,
            "supportsResume": false,
            "supportsWorktreeIsolation": true,
            "thinking": ["off", "minimal", "low", "medium", "high", "xhigh"]
        },
        "codex": {
            "modes": ["research", "review", "implement", "command"],
            "supportsReply": false,
            "supportsResume": false,
            "supportsWorktreeIsolation": true,
            "thinking": ["low", "medium", "high", "xhigh"]
        }
    })
}

pub fn validate_options(task: &ProviderTask<'_>) -> Result<(), String> {
    if task.provider == ProviderKind::Cursor && task.mode == TaskMode::Command {
        return Err("cursor does not support mode: command".to_string());
    }
    if let Some(effort) = task.effort {
        let allowed = ["low", "medium", "high", "xhigh", "max"];
        if task.provider != ProviderKind::Claude || !allowed.contains(&effort) {
            return Err("effort is only supported for claude and must be one of: low, medium, high, xhigh, max".to_string());
        }
    }
    if let Some(thinking) = task.thinking {
        let allowed = match task.provider {
            ProviderKind::Kimi => &["off", "minimal", "low", "medium", "high", "xhigh"][..],
            ProviderKind::Codex => &["low", "medium", "high", "xhigh"][..],
            _ => {
                return Err(format!(
                    "thinking is not supported for {}",
                    task.provider.as_str()
                ));
            }
        };
        if !allowed.contains(&thinking) {
            return Err(format!(
                "thinking is not supported for {}",
                task.provider.as_str()
            ));
        }
    }
    Ok(())
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
    };
    validate_options(&task)?;
    let command = match provider {
        ProviderKind::Claude => build_claude_command(&task, PROVIDER_SMOKE_PROMPT.to_string()),
        ProviderKind::Cursor => ProviderCommand {
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
                vec![
                    "--trust".to_string(),
                    "--".to_string(),
                    PROVIDER_SMOKE_PROMPT.to_string(),
                ],
            ]
            .concat(),
            stdin: None,
            redactions: vec![PROVIDER_SMOKE_PROMPT.to_string()],
            cwd: task.cwd.to_string(),
            timeout_seconds: task.timeout_seconds,
            env: BTreeMap::new(),
        },
        ProviderKind::Kimi => ProviderCommand {
            provider: task.provider,
            command_kind: None,
            claude_host: None,
            command: env_or("PI_BIN", "pi"),
            args: vec![
                "-p".to_string(),
                "--no-session".to_string(),
                "--no-context-files".to_string(),
                "--tools".to_string(),
                kimi_tools(task.mode).to_string(),
                PROVIDER_SMOKE_PROMPT.to_string(),
            ],
            stdin: None,
            redactions: vec![PROVIDER_SMOKE_PROMPT.to_string()],
            cwd: task.cwd.to_string(),
            timeout_seconds: task.timeout_seconds,
            env: BTreeMap::new(),
        },
        ProviderKind::Codex => ProviderCommand {
            provider: task.provider,
            command_kind: None,
            claude_host: None,
            command: env_or("CODEX_BIN", "codex"),
            args: vec![
                "exec".to_string(),
                "--cd".to_string(),
                task.cwd.to_string(),
                "--json".to_string(),
                "--sandbox".to_string(),
                codex_sandbox(task.mode).to_string(),
                "--config".to_string(),
                "shell_environment_policy.inherit=\"all\"".to_string(),
                PROVIDER_SMOKE_PROMPT.to_string(),
            ],
            stdin: None,
            redactions: vec![PROVIDER_SMOKE_PROMPT.to_string()],
            cwd: task.cwd.to_string(),
            timeout_seconds: task.timeout_seconds,
            env: BTreeMap::new(),
        },
    };
    Ok((command, "minimal"))
}

pub fn build_command(task: &ProviderTask<'_>) -> Result<ProviderCommand, String> {
    validate_options(task)?;
    let rendered_prompt = render_task_prompt(task);
    let command = match task.provider {
        ProviderKind::Claude => build_claude_command(task, rendered_prompt),
        ProviderKind::Cursor => ProviderCommand {
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
        },
        ProviderKind::Kimi => ProviderCommand {
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
        },
        ProviderKind::Codex => ProviderCommand {
            provider: task.provider,
            command_kind: None,
            claude_host: None,
            command: env_or("CODEX_BIN", "codex"),
            args: [
                vec![
                    "exec".to_string(),
                    "--cd".to_string(),
                    task.cwd.to_string(),
                    "--json".to_string(),
                    "--sandbox".to_string(),
                    codex_sandbox(task.mode).to_string(),
                    "--config".to_string(),
                    "shell_environment_policy.inherit=\"all\"".to_string(),
                ],
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
        },
    };
    Ok(command)
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
            "CLAUDE_P_BIN",
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
            "CLAUDE_P_BIN",
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
    let native_claude = env::var("CLAUDE_BIN").ok();
    let claude_p = env::var("CLAUDE_P_BIN").ok();
    let (command_kind, mut inner_args) =
        if let Some(native_claude) = native_claude.filter(|_| claude_p.is_none()) {
            let args = [
                vec![
                    native_claude,
                    "-p".to_string(),
                    "--output-format".to_string(),
                    "json".to_string(),
                ],
                claude_mode_flags(task.mode),
                optional_arg("--model", task.model),
                optional_arg("--effort", task.effort),
            ]
            .concat();
            ("native-claude".to_string(), args)
        } else {
            let args = [
                vec![
                    claude_p.unwrap_or_else(|| "claude-p".to_string()),
                    "--cwd".to_string(),
                    task.cwd.to_string(),
                    "--timeout".to_string(),
                    task.timeout_seconds.to_string(),
                    "--output-format".to_string(),
                    "json".to_string(),
                ],
                claude_mode_flags(task.mode),
                optional_arg("--model", task.model),
                optional_arg("--effort", task.effort),
            ]
            .concat();
            ("claude-p".to_string(), args)
        };
    let mut args = vec![
        "-flc".to_string(),
        "source ~/.zshenv </dev/null 2>/dev/null || true; source ~/.zprofile </dev/null 2>/dev/null || true; source ~/.zshrc </dev/null 2>/dev/null || true; exec \"$@\"".to_string(),
        "agent-bridge-provider".to_string(),
    ];
    args.append(&mut inner_args);
    ProviderCommand {
        provider: task.provider,
        command_kind: Some(command_kind),
        claude_host: Some(ClaudeHostCommand {
            cwd: task.cwd.to_string(),
            timeout_seconds: task.timeout_seconds,
            mode: task.mode,
            prompt: rendered_prompt.clone(),
            model: task.model.map(str::to_string),
            effort: task.effort.map(str::to_string),
        }),
        command: "/bin/zsh".to_string(),
        args,
        stdin: Some(rendered_prompt.clone()),
        redactions: vec![rendered_prompt, task.prompt.to_string()],
        cwd: task.cwd.to_string(),
        timeout_seconds: task.timeout_seconds,
        env: BTreeMap::new(),
    }
}

fn provider_command_kind(provider: ProviderKind) -> Option<String> {
    match provider {
        ProviderKind::Claude => {
            if env::var("CLAUDE_BIN").is_ok() && env::var("CLAUDE_P_BIN").is_err() {
                Some("native-claude".to_string())
            } else {
                Some("claude-p".to_string())
            }
        }
        _ => None,
    }
}

fn resolve_command(provider: ProviderKind) -> String {
    match provider {
        ProviderKind::Claude => env::var("CLAUDE_P_BIN")
            .ok()
            .or_else(|| env::var("CLAUDE_BIN").ok())
            .unwrap_or_else(|| "claude-p".to_string()),
        ProviderKind::Cursor => env_or("CURSOR_AGENT_BIN", "cursor-agent"),
        ProviderKind::Kimi => env_or("PI_BIN", "pi"),
        ProviderKind::Codex => env_or("CODEX_BIN", "codex"),
    }
}

fn render_task_prompt(task: &ProviderTask<'_>) -> String {
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

fn claude_mode_flags(mode: TaskMode) -> Vec<String> {
    match mode {
        TaskMode::Research | TaskMode::Review => vec![
            "--permission-mode".to_string(),
            "dontAsk".to_string(),
            "--allowedTools".to_string(),
            "Read,Grep,Glob".to_string(),
            "--disallowedTools".to_string(),
            "Bash,Edit,Write".to_string(),
        ],
        TaskMode::Command => vec![
            "--permission-mode".to_string(),
            "default".to_string(),
            "--allowedTools".to_string(),
            "Read,Grep,Glob,Bash".to_string(),
            "--disallowedTools".to_string(),
            "Edit,Write".to_string(),
        ],
        TaskMode::Implement => vec!["--permission-mode".to_string(), "default".to_string()],
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

fn optional_arg(flag: &str, value: Option<&str>) -> Vec<String> {
    value
        .map(|value| vec![flag.to_string(), value.to_string()])
        .unwrap_or_default()
}

fn env_or(name: &str, fallback: &str) -> String {
    env::var(name).unwrap_or_else(|_| fallback.to_string())
}
