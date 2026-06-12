use crate::mcp::{JsonRpcId, JsonRpcRequest, JsonRpcResponse};
use crate::server::handle_request;
use crate::task::TaskManagerHandle;
use clap::{Parser, Subcommand};
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Parser, Debug)]
#[command(
    name = "agent-bridge-mcp",
    version,
    about = "Rust stdio MCP server for delegating bounded tasks to local provider agents",
    after_help = "Config file: ~/.agent-bridge-mcp/config.toml\nLegacy env vars still work: AGENT_BRIDGE_WORKSPACES, AGENT_BRIDGE_STATE_DIR, AGENT_BRIDGE_CLAUDE_HOST_SOCKET, AGENT_BRIDGE_MAX_ACTIVE_TASKS"
)]
struct Cli {
    #[command(flatten)]
    config: crate::config::ConfigCliOverrides,
    /// Validate layered config and print a JSON summary without starting MCP stdio.
    #[arg(long, conflicts_with = "doctor_smoke")]
    config_check: bool,
    /// Run provider readiness smoke and print the doctor provider JSON report.
    #[arg(long)]
    doctor_smoke: bool,
    /// Restrict --doctor-smoke to one provider. Repeat for multiple providers.
    #[arg(long = "provider", value_name = "PROVIDER")]
    providers: Vec<crate::domain::ProviderKind>,
    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Subcommand, Debug)]
enum CliCommand {
    /// Ask a running Agent Bridge server to reload config from disk.
    Reload,
    /// Run the Agent Bridge-owned Claude host runner on the given Unix socket.
    ClaudeHostRunner { socket: std::path::PathBuf },
}

pub async fn main_entry() {
    init_tracing();
    install_panic_hook();
    if std::env::var("AGENT_BRIDGE_FORCE_PANIC").ok().as_deref() == Some("1") {
        panic!("forced panic for integration test");
    }
    let cli = Cli::parse();
    if cli.config_check {
        exit_with_config_check(cli.config);
    }
    if cli.doctor_smoke {
        exit_with_doctor_smoke(cli.providers).await;
    }
    match cli.command {
        Some(CliCommand::Reload) => exit_with_reload(cli.config),
        Some(CliCommand::ClaudeHostRunner {
            socket: socket_path,
        }) => {
            if let Err(error) = crate::claude_host::run_server(socket_path).await {
                tracing::error!(error = %error, "[agent-bridge] fatal {error}");
                std::process::exit(1);
            }
            return;
        }
        None => {}
    }

    let state_dir = match crate::config::Config::from_env(cli.config.clone()) {
        Ok(config) => {
            if let Err(error) = crate::config::install_runtime_config(&config) {
                tracing::error!(error = %error, "[agent-bridge] fatal {error}");
                std::process::exit(1);
            }
            config.state_dir().to_path_buf()
        }
        Err(error) => {
            tracing::error!(
                error = %error,
                "[agent-bridge] startup config load failed; starting with state-dir-only fallback so doctor can report diagnostics"
            );
            crate::config::reload_pid_state_dir(&cli.config)
        }
    };
    let pid_lock = match PidLock::acquire(&state_dir) {
        Ok(lock) => lock,
        Err(error) => {
            tracing::error!(error = %error, "[agent-bridge] fatal {error}");
            std::process::exit(1);
        }
    };
    install_reload_handler(cli.config);
    match run_until_shutdown().await {
        Ok(exit_code) => {
            drop(pid_lock);
            if exit_code != 0 {
                std::process::exit(exit_code);
            }
        }
        Err(error) => {
            drop(pid_lock);
            tracing::error!(error = %error, "[agent-bridge] fatal {error}");
            std::process::exit(1);
        }
    }
}

fn exit_with_reload(cli_config: crate::config::ConfigCliOverrides) -> ! {
    match reload_server(cli_config) {
        Ok(pid) => {
            write_stdout_json(&json!({
                "status": "ok",
                "pid": pid,
                "signal": "SIGHUP",
            }));
            std::process::exit(0);
        }
        Err(error) => {
            write_stdout_json(&json!({
                "status": "error",
                "error": error,
            }));
            std::process::exit(1);
        }
    }
}

#[cfg(unix)]
fn reload_server(cli_config: crate::config::ConfigCliOverrides) -> Result<u32, String> {
    let state_dir = match crate::config::Config::from_env(cli_config.clone()) {
        Ok(config) => config.state_dir().to_path_buf(),
        Err(error) => {
            tracing::warn!(
                error = %error,
                "[agent-bridge] reload command could not parse full config; falling back to state-dir-only PID lookup"
            );
            crate::config::reload_pid_state_dir(&cli_config)
        }
    };
    let pid = read_pid(&state_dir.join("server.pid"))?;
    let result = unsafe { libc::kill(pid as libc::pid_t, libc::SIGHUP) };
    if result == 0 {
        Ok(pid)
    } else {
        Err(std::io::Error::last_os_error().to_string())
    }
}

#[cfg(not(unix))]
fn reload_server(_cli_config: crate::config::ConfigCliOverrides) -> Result<u32, String> {
    Err("reload is only supported on Unix platforms".to_string())
}

fn read_pid(path: &Path) -> Result<u32, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    text.trim()
        .parse::<u32>()
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

struct PidLock {
    path: PathBuf,
    pid: u32,
}

impl PidLock {
    fn acquire(state_dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(state_dir).map_err(|error| error.to_string())?;
        let path = state_dir.join("server.pid");
        if let Ok(pid) = read_pid(&path) {
            if process_is_alive(pid) {
                return Err(format!(
                    "Agent Bridge server already appears to be running with pid {pid}; stop it before starting another instance"
                ));
            }
            let _ = std::fs::remove_file(&path);
        }
        let pid = std::process::id();
        std::fs::write(&path, format!("{pid}\n")).map_err(|error| error.to_string())?;
        Ok(Self { path, pid })
    }
}

impl Drop for PidLock {
    fn drop(&mut self) {
        if read_pid(&self.path).ok() == Some(self.pid) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    result == 0 || std::io::Error::last_os_error().kind() == std::io::ErrorKind::PermissionDenied
}

#[cfg(not(unix))]
fn process_is_alive(_pid: u32) -> bool {
    false
}

#[cfg(unix)]
fn install_reload_handler(cli_config: crate::config::ConfigCliOverrides) {
    tokio::spawn(async move {
        let mut sighup = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
        {
            Ok(signal) => signal,
            Err(error) => {
                tracing::error!(error = %error, "[agent-bridge] failed to install SIGHUP handler");
                return;
            }
        };
        while sighup.recv().await.is_some() {
            match crate::config::reload_runtime_config(cli_config.clone()) {
                Ok(config) => tracing::info!(
                    workspace_count = config.configured_workspace_roots().len(),
                    "[agent-bridge] reloaded runtime config"
                ),
                Err(error) => tracing::error!(
                    error = %error,
                    "[agent-bridge] failed to reload runtime config; preserving incumbent workspace roots"
                ),
            }
        }
    });
}

#[cfg(not(unix))]
fn install_reload_handler(_cli_config: crate::config::ConfigCliOverrides) {}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .json()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_current_span(true)
        .with_span_list(true)
        .try_init();
}

fn exit_with_config_check(cli_config: crate::config::ConfigCliOverrides) -> ! {
    match crate::config::Config::from_env(cli_config) {
        Ok(config) => {
            write_stdout_json(&json!({
                "status": "ok",
                "valid": true,
                "workspaces": config.configured_workspace_roots()
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>(),
                "stateDir": config.state_dir().display().to_string(),
                "claudeHostSocket": config.claude_host_socket()
                    .map(|path| path.display().to_string()),
                "maxActiveTasks": config.max_active_tasks(),
            }));
            std::process::exit(0);
        }
        Err(error) => {
            write_stdout_json(&json!({
                "status": "error",
                "valid": false,
                "error": error,
            }));
            std::process::exit(1);
        }
    }
}

async fn exit_with_doctor_smoke(providers: Vec<crate::domain::ProviderKind>) -> ! {
    let provider_names = providers
        .iter()
        .map(|provider| provider.as_str())
        .collect::<Vec<_>>();
    let mut arguments = json!({
        "smoke": true,
    });
    if !provider_names.is_empty() {
        arguments["providers"] = json!(provider_names);
    }
    match crate::server::doctor_report(arguments).await {
        Ok(report) => {
            let ready = report["launchReadiness"]["status"].as_str() == Some("ready");
            write_stdout_json(&report);
            std::process::exit(if ready { 0 } else { 1 });
        }
        Err(error) => {
            write_stdout_json(&json!({
                "status": "error",
                "error": error,
            }));
            std::process::exit(1);
        }
    }
}

fn write_stdout_json(value: &Value) {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, value).expect("serialize CLI JSON");
    stdout.write_all(b"\n").expect("write CLI JSON newline");
    stdout.flush().expect("flush CLI JSON");
}

async fn run_until_shutdown() -> io::Result<i32> {
    tokio::select! {
        result = run_stdio_server() => result.map(|_| 0),
        exit_code = shutdown_signal() => {
            if let Ok(manager) = TaskManagerHandle::from_env().await {
                let _ = manager.shutdown().await;
            }
            Ok(exit_code)
        }
    }
}

async fn run_stdio_server() -> io::Result<()> {
    let stdin = io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    let mut stdout = io::stdout();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        let request: Result<JsonRpcRequest, _> = serde_json::from_str(&line);
        match request {
            Ok(request) => {
                if let Some(response) = handle_request(request).await {
                    write_response(&mut stdout, &response).await?;
                }
            }
            Err(_) => {
                let response = JsonRpcResponse::error(JsonRpcId::Null, -32700, "Parse error");
                write_response(&mut stdout, &response).await?;
            }
        }
    }

    Ok(())
}

#[cfg(unix)]
async fn shutdown_signal() -> i32 {
    use tokio::signal::unix::{SignalKind, signal};

    let mut sigterm = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => 130,
        _ = sigterm.recv() => 143,
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() -> i32 {
    let _ = tokio::signal::ctrl_c().await;
    130
}

async fn write_response(stdout: &mut io::Stdout, response: &JsonRpcResponse) -> io::Result<()> {
    let mut line = serde_json::to_vec(response).map_err(io::Error::other)?;
    line.push(b'\n');
    stdout.write_all(&line).await?;
    stdout.flush().await
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        tracing::error!(panic = %info, "[agent-bridge] panic {info}");
        // Best-effort: terminate any tracked provider children so a panic does
        // not leave orphaned processes behind.
        #[cfg(unix)]
        crate::task::terminate_all_active_pids(libc::SIGTERM);
    }));
}
