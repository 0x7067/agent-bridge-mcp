use clap::Args;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

pub const DEFAULT_MAX_ACTIVE_TASKS: usize = 16;

const DEFAULT_STATE_DIR: &str = "~/.agent-bridge-mcp/state";
const DEFAULT_CONFIG_PATH: &str = "~/.agent-bridge-mcp/config.toml";
const ENV_WORKSPACES: &str = "AGENT_BRIDGE_WORKSPACES";
const ENV_STATE_DIR: &str = "AGENT_BRIDGE_STATE_DIR";
const ENV_CLAUDE_HOST_SOCKET: &str = "AGENT_BRIDGE_CLAUDE_HOST_SOCKET";
const ENV_MAX_ACTIVE_TASKS: &str = "AGENT_BRIDGE_MAX_ACTIVE_TASKS";
const ENV_STRICT_VALIDATION: &str = "AGENT_BRIDGE_STRICT_VALIDATION";

static RUNTIME_WORKSPACE_ROOTS: OnceLock<RwLock<Option<Vec<PathBuf>>>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    workspaces: Vec<PathBuf>,
    state_dir: PathBuf,
    claude_host_socket: Option<PathBuf>,
    max_active_tasks: usize,
    strict_validation: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ConfigSources {
    pub config_path: Option<PathBuf>,
    pub env: BTreeMap<String, OsString>,
    pub cli_workspaces: Option<Vec<PathBuf>>,
    pub cli_state_dir: Option<PathBuf>,
    pub cli_claude_host_socket: Option<PathBuf>,
    pub cli_max_active_tasks: Option<usize>,
    pub home_dir: Option<PathBuf>,
}

#[derive(Args, Debug, Clone, Default)]
pub struct ConfigCliOverrides {
    /// Workspace roots allowed for delegated agents.
    #[arg(long, value_name = "PATH", value_delimiter = ':')]
    pub workspaces: Option<Vec<PathBuf>>,
    /// Directory used for task state and registry files.
    #[arg(long, value_name = "PATH")]
    pub state_dir: Option<PathBuf>,
    /// Unix socket for the Claude host runner.
    #[arg(long, value_name = "PATH")]
    pub claude_host_socket: Option<PathBuf>,
    /// Maximum concurrently active delegated tasks.
    #[arg(long, value_name = "N")]
    pub max_active_tasks: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
struct FileConfig {
    workspaces: Option<WorkspaceValues>,
    state_dir: Option<PathBuf>,
    claude_host_socket: Option<PathBuf>,
    max_active_tasks: Option<usize>,
    strict_validation: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WorkspaceValues {
    One(PathBuf),
    Many(Vec<PathBuf>),
}

impl Config {
    pub fn from_env(cli: ConfigCliOverrides) -> Result<Self, String> {
        let env = env::vars_os()
            .map(|(key, value)| (key.to_string_lossy().into_owned(), value))
            .collect();
        Self::load(ConfigSources {
            env,
            cli_workspaces: cli.workspaces,
            cli_state_dir: cli.state_dir,
            cli_claude_host_socket: cli.claude_host_socket,
            cli_max_active_tasks: cli.max_active_tasks,
            ..ConfigSources::default()
        })
    }

    pub fn load(sources: ConfigSources) -> Result<Self, String> {
        let home_dir = sources
            .home_dir
            .clone()
            .or_else(|| env::var_os("HOME").map(PathBuf::from));
        let config_path = sources
            .config_path
            .clone()
            .unwrap_or_else(|| expand_home_with(DEFAULT_CONFIG_PATH, home_dir.as_deref()));
        let file_config = load_file_config(&config_path)?;
        let mut builder = ConfigBuilder::default();

        if let Some(file_config) = file_config {
            builder.merge_file(file_config);
        }
        builder.merge_env(&sources.env);
        builder.merge_cli(&sources);

        let workspaces = match builder.workspaces {
            Some(workspaces) if !workspaces.is_empty() => workspaces,
            _ => vec![env::current_dir().map_err(|error| error.to_string())?],
        };
        let workspaces = workspaces
            .into_iter()
            .map(|workspace| expand_path(workspace, home_dir.as_deref()).canonicalize())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;

        let state_dir = expand_path(
            builder
                .state_dir
                .unwrap_or_else(|| PathBuf::from(DEFAULT_STATE_DIR)),
            home_dir.as_deref(),
        );
        let claude_host_socket = builder
            .claude_host_socket
            .map(|path| expand_path(path, home_dir.as_deref()));
        let max_active_tasks = builder
            .max_active_tasks
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_MAX_ACTIVE_TASKS);
        let strict_validation = builder.strict_validation.unwrap_or(false);

        Ok(Self {
            workspaces,
            state_dir,
            claude_host_socket,
            max_active_tasks,
            strict_validation,
        })
    }

    pub fn workspaces(&self) -> &[PathBuf] {
        &self.workspaces
    }

    pub fn configured_workspace_roots(&self) -> &[PathBuf] {
        self.workspaces()
    }

    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    pub fn claude_host_socket(&self) -> Option<&Path> {
        self.claude_host_socket.as_deref()
    }

    pub fn max_active_tasks(&self) -> usize {
        self.max_active_tasks
    }

    pub fn strict_validation(&self) -> bool {
        self.strict_validation
    }
}

pub fn strict_validation_enabled() -> bool {
    Config::from_env(ConfigCliOverrides::default())
        .map(|config| config.strict_validation())
        .unwrap_or(false)
}

pub fn install_runtime_config(config: &Config) -> Result<(), String> {
    let lock = RUNTIME_WORKSPACE_ROOTS.get_or_init(|| RwLock::new(None));
    let mut roots = lock
        .write()
        .map_err(|_| "runtime workspace cache is poisoned".to_string())?;
    *roots = Some(config.configured_workspace_roots().to_vec());
    Ok(())
}

pub fn reload_runtime_config(cli: ConfigCliOverrides) -> Result<Config, String> {
    let config = Config::from_env(cli)?;
    install_runtime_config(&config)?;
    Ok(config)
}

pub fn runtime_workspace_roots() -> Result<Vec<PathBuf>, String> {
    let lock = RUNTIME_WORKSPACE_ROOTS.get_or_init(|| RwLock::new(None));
    if let Some(roots) = lock
        .read()
        .map_err(|_| "runtime workspace cache is poisoned".to_string())?
        .clone()
    {
        return Ok(roots);
    }
    Config::from_env(ConfigCliOverrides::default())
        .map(|config| config.configured_workspace_roots().to_vec())
}

pub fn reload_pid_state_dir(cli: &ConfigCliOverrides) -> PathBuf {
    let home_dir = env::var_os("HOME").map(PathBuf::from);
    if let Some(state_dir) = cli.state_dir.clone() {
        return expand_path(state_dir, home_dir.as_deref());
    }
    if let Some(state_dir) = env::var_os(ENV_STATE_DIR) {
        return expand_path(PathBuf::from(state_dir), home_dir.as_deref());
    }
    expand_home_with(DEFAULT_STATE_DIR, home_dir.as_deref())
}

#[derive(Debug, Default)]
struct ConfigBuilder {
    workspaces: Option<Vec<PathBuf>>,
    state_dir: Option<PathBuf>,
    claude_host_socket: Option<PathBuf>,
    max_active_tasks: Option<usize>,
    strict_validation: Option<bool>,
}

impl ConfigBuilder {
    fn merge_file(&mut self, file: FileConfig) {
        if let Some(workspaces) = file.workspaces {
            self.workspaces = Some(match workspaces {
                WorkspaceValues::One(path) => vec![path],
                WorkspaceValues::Many(paths) => paths,
            });
        }
        if let Some(state_dir) = file.state_dir {
            self.state_dir = Some(state_dir);
        }
        if let Some(socket) = file.claude_host_socket {
            self.claude_host_socket = Some(socket);
        }
        if let Some(max_active_tasks) = file.max_active_tasks {
            self.max_active_tasks = Some(max_active_tasks);
        }
        if let Some(strict_validation) = file.strict_validation {
            self.strict_validation = Some(strict_validation);
        }
    }

    fn merge_env(&mut self, env: &BTreeMap<String, OsString>) {
        if let Some(value) = env.get(ENV_WORKSPACES) {
            warn_legacy_env(ENV_WORKSPACES);
            self.workspaces = Some(split_paths(value));
        }
        if let Some(value) = env.get(ENV_STATE_DIR) {
            warn_legacy_env(ENV_STATE_DIR);
            self.state_dir = Some(PathBuf::from(value));
        }
        if let Some(value) = env.get(ENV_CLAUDE_HOST_SOCKET)
            && !value.is_empty()
        {
            warn_legacy_env(ENV_CLAUDE_HOST_SOCKET);
            self.claude_host_socket = Some(PathBuf::from(value));
        }
        if let Some(value) = env.get(ENV_MAX_ACTIVE_TASKS) {
            warn_legacy_env(ENV_MAX_ACTIVE_TASKS);
            self.max_active_tasks = value.to_string_lossy().parse::<usize>().ok();
        }
        if let Some(value) = env.get(ENV_STRICT_VALIDATION) {
            self.strict_validation = parse_bool(value);
        }
    }

    fn merge_cli(&mut self, sources: &ConfigSources) {
        if let Some(workspaces) = sources.cli_workspaces.clone() {
            self.workspaces = Some(workspaces);
        }
        if let Some(state_dir) = sources.cli_state_dir.clone() {
            self.state_dir = Some(state_dir);
        }
        if let Some(socket) = sources.cli_claude_host_socket.clone() {
            self.claude_host_socket = Some(socket);
        }
        if let Some(max_active_tasks) = sources.cli_max_active_tasks {
            self.max_active_tasks = Some(max_active_tasks);
        }
    }
}

pub fn expand_home(value: &str) -> PathBuf {
    expand_home_with(value, env::var_os("HOME").map(PathBuf::from).as_deref())
}

fn expand_path(path: PathBuf, home_dir: Option<&Path>) -> PathBuf {
    match path.to_str() {
        Some(value) => expand_home_with(value, home_dir),
        None => path,
    }
}

fn expand_home_with(value: &str, home_dir: Option<&Path>) -> PathBuf {
    if value == "~" {
        return home_dir
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(value));
    }
    if let Some(rest) = value.strip_prefix("~/") {
        return home_dir
            .map(|home| home.join(rest))
            .unwrap_or_else(|| PathBuf::from(value));
    }
    if let Some(user_path) = value.strip_prefix('~')
        && !user_path.is_empty()
    {
        let (user, rest) = user_path.split_once('/').unwrap_or((user_path, ""));
        return home_dir
            .and_then(Path::parent)
            .map(|parent| {
                let user_home = parent.join(user);
                if rest.is_empty() {
                    user_home
                } else {
                    user_home.join(rest)
                }
            })
            .unwrap_or_else(|| PathBuf::from(value));
    }
    PathBuf::from(value)
}

fn load_file_config(path: &Path) -> Result<Option<FileConfig>, String> {
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            let config = toml::from_str(&contents)
                .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
            Ok(Some(config))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("failed to read {}: {error}", path.display())),
    }
}

fn split_paths(value: &OsString) -> Vec<PathBuf> {
    env::split_paths(value)
        .filter(|path| !path.as_os_str().is_empty())
        .collect()
}

fn parse_bool(value: &OsString) -> Option<bool> {
    match value.to_string_lossy().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn warn_legacy_env(key: &str) {
    tracing::warn!(
        env_var = key,
        "legacy Agent Bridge environment variable is deprecated; prefer ~/.agent-bridge-mcp/config.toml or CLI flags"
    );
}
