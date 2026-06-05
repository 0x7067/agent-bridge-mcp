use crate::claude_interactive::pty::{PtySession, PtySize, PtySpawn, spawn};
use crate::domain::TaskMode;
use crate::provider;
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

const LOGIN_SHELL: &str = "/bin/zsh";
const LOGIN_SHELL_BOOTSTRAP: &str = "exec \"$@\"";
const LOGIN_SHELL_ARG0: &str = "agent-bridge-claude";

pub struct ClaudeRunnerRequest {
    pub claude_bin: PathBuf,
    pub cwd: PathBuf,
    pub mode: TaskMode,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub settings_path: Option<PathBuf>,
    pub extra_env: BTreeMap<String, String>,
}

pub fn spawn_claude(request: ClaudeRunnerRequest) -> io::Result<PtySession> {
    spawn(build_pty_spawn(request))
}

pub fn build_pty_spawn(request: ClaudeRunnerRequest) -> PtySpawn {
    let mut args = vec![
        "-flc".to_string(),
        LOGIN_SHELL_BOOTSTRAP.to_string(),
        LOGIN_SHELL_ARG0.to_string(),
        request.claude_bin.display().to_string(),
    ];
    args.extend(mode_flags(request.mode));
    if let Some(settings_path) = request.settings_path {
        args.extend([
            "--settings".to_string(),
            settings_path.display().to_string(),
        ]);
    }
    if let Some(model) = request.model {
        args.extend(["--model".to_string(), model]);
    }
    if let Some(effort) = request.effort {
        args.extend(["--effort".to_string(), effort]);
    }
    let mut env = provider::provider_env(crate::domain::ProviderKind::Claude);
    env.extend(request.extra_env);
    PtySpawn {
        program: Path::new(LOGIN_SHELL).to_path_buf(),
        args,
        cwd: request.cwd,
        env,
        size: PtySize {
            rows: 40,
            cols: 120,
        },
        resize_after_open: None,
    }
}

fn mode_flags(mode: TaskMode) -> Vec<String> {
    match mode {
        TaskMode::Research | TaskMode::Review => vec![
            "--permission-mode".to_string(),
            "dontAsk".to_string(),
            "--tools".to_string(),
            "Read,Grep,Glob".to_string(),
            "--allowedTools".to_string(),
            "Read,Grep,Glob".to_string(),
            "--disallowedTools".to_string(),
            "Bash,Edit,Write".to_string(),
        ],
        TaskMode::Command => vec![
            "--permission-mode".to_string(),
            "default".to_string(),
            "--tools".to_string(),
            "Read,Grep,Glob,Bash".to_string(),
            "--allowedTools".to_string(),
            "Read,Grep,Glob,Bash".to_string(),
            "--disallowedTools".to_string(),
            "Edit,Write".to_string(),
        ],
        TaskMode::Implement => vec!["--permission-mode".to_string(), "default".to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_spawn_uses_fixed_login_shell_and_interactive_flags() {
        let spawn = build_pty_spawn(ClaudeRunnerRequest {
            claude_bin: PathBuf::from("/usr/local/bin/claude"),
            cwd: PathBuf::from("/tmp/workspace"),
            mode: TaskMode::Research,
            model: Some("sonnet".to_string()),
            effort: Some("high".to_string()),
            settings_path: Some(PathBuf::from("/tmp/settings.json")),
            extra_env: BTreeMap::new(),
        });

        assert_eq!(spawn.program, PathBuf::from(LOGIN_SHELL));
        assert_eq!(spawn.args[0], "-flc");
        assert_eq!(spawn.args[1], LOGIN_SHELL_BOOTSTRAP);
        assert_eq!(spawn.args[2], LOGIN_SHELL_ARG0);
        assert_eq!(spawn.args[3], "/usr/local/bin/claude");
        assert!(spawn.args.contains(&"--permission-mode".to_string()));
        assert!(spawn.args.contains(&"dontAsk".to_string()));
        assert!(spawn.args.contains(&"--tools".to_string()));
        assert!(spawn.args.contains(&"Read,Grep,Glob".to_string()));
        assert!(spawn.args.contains(&"--settings".to_string()));
        assert!(spawn.args.contains(&"/tmp/settings.json".to_string()));
        assert!(spawn.args.contains(&"--model".to_string()));
        assert!(spawn.args.contains(&"sonnet".to_string()));
        assert!(spawn.args.contains(&"--effort".to_string()));
        assert!(spawn.args.contains(&"high".to_string()));
    }
}
