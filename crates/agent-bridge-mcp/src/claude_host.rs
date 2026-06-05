use crate::domain::{MAX_TIMEOUT_SECONDS, MIN_TIMEOUT_SECONDS, TaskMode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::env;
use std::fs;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::process::Command;
use tokio::sync::oneshot;
use tokio::time::{Duration, timeout};

pub const PROTOCOL_VERSION: u32 = 2;
const MAX_REQUEST_BYTES: usize = 1024 * 1024;
const MAX_STREAM_BYTES: usize = 1024 * 1024;
const MAX_PTY_OUTPUT_EXCERPT: usize = 64 * 1024;
const CHILD_SHUTDOWN_GRACE: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeHostCommand {
    pub cwd: String,
    pub timeout_seconds: i64,
    pub mode: TaskMode,
    pub prompt: String,
    pub model: Option<String>,
    pub effort: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HostRequest {
    pub version: u32,
    #[serde(rename = "workspacePolicyId")]
    pub workspace_policy_id: String,
    #[serde(flatten)]
    pub kind: HostRequestKind,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "requestType", rename_all = "snake_case")]
pub enum HostRequestKind {
    Ping,
    #[serde(rename = "claude_interactive")]
    RunClaude {
        cwd: String,
        #[serde(rename = "timeoutSeconds")]
        timeout_seconds: i64,
        mode: TaskMode,
        prompt: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        effort: Option<String>,
        #[serde(
            rename = "bareProfile",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        bare_profile: Option<bool>,
        #[serde(
            rename = "smokeToken",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        smoke_token: Option<String>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HostResponse {
    pub version: u32,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<HostResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<HostError>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "responseType", rename_all = "snake_case")]
pub enum HostResult {
    Pong {
        #[serde(rename = "protocolVersion")]
        protocol_version: u32,
        #[serde(rename = "workspacePolicyId")]
        workspace_policy_id: String,
        ready: bool,
    },
    #[serde(rename = "claude_interactive_result")]
    Run {
        status: String,
        #[serde(rename = "exitCode")]
        exit_code: Option<i32>,
        signal: Option<String>,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
        #[serde(rename = "failureCategory")]
        failure_category: Option<String>,
        #[serde(rename = "ptyOutputExcerpt")]
        pty_output_excerpt: String,
        #[serde(rename = "ptyOutputTruncated")]
        pty_output_truncated: bool,
        #[serde(rename = "redactionsApplied")]
        redactions_applied: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<ClaudeInteractiveSuccess>,
        stop: Option<Value>,
        #[serde(rename = "stopFailure")]
        stop_failure: Option<Value>,
        transcript: Value,
        // Temporary compatibility fields while tasks 3.2-3.5 migrate task and
        // readiness consumers to the structured v2 result.
        stdout: String,
        stderr: String,
        #[serde(rename = "stdoutTruncated")]
        stdout_truncated: bool,
        #[serde(rename = "stderrTruncated")]
        stderr_truncated: bool,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClaudeInteractiveSuccess {
    #[serde(rename = "finalText")]
    pub final_text: String,
    pub source: String,
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HostError {
    pub code: String,
    pub message: String,
}

pub fn configured_workspace_roots() -> Result<Vec<PathBuf>, String> {
    let roots: Vec<PathBuf> = env::var_os("AGENT_BRIDGE_WORKSPACES")
        .ok_or_else(|| "AGENT_BRIDGE_WORKSPACES is required".to_string())
        .map(|value| {
            env::split_paths(&value)
                .filter(|path| !path.as_os_str().is_empty())
                .collect()
        })?;
    if roots.is_empty() {
        return Err("AGENT_BRIDGE_WORKSPACES is required".to_string());
    }
    canonicalize_roots(&roots)
}

pub fn workspace_policy_id(roots: &[PathBuf]) -> Result<String, String> {
    let mut roots = canonicalize_roots(roots)?;
    roots.sort();
    Ok(roots
        .into_iter()
        .map(|root| root.display().to_string())
        .collect::<Vec<_>>()
        .join("\0"))
}

pub fn socket_path_from_env() -> Option<PathBuf> {
    env::var_os("AGENT_BRIDGE_CLAUDE_HOST_SOCKET")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub async fn ping(socket_path: &Path) -> Result<HostResponse, String> {
    let roots = configured_workspace_roots()?;
    let host_request = HostRequest {
        version: PROTOCOL_VERSION,
        workspace_policy_id: workspace_policy_id(&roots)?,
        kind: HostRequestKind::Ping,
    };
    send_request(socket_path, &host_request).await
}

pub async fn run_claude(
    socket_path: &Path,
    command: &ClaudeHostCommand,
) -> Result<HostResponse, String> {
    let roots = configured_workspace_roots()?;
    let host_request = HostRequest {
        version: PROTOCOL_VERSION,
        workspace_policy_id: workspace_policy_id(&roots)?,
        kind: HostRequestKind::RunClaude {
            cwd: command.cwd.clone(),
            timeout_seconds: command.timeout_seconds,
            mode: command.mode,
            prompt: command.prompt.clone(),
            model: command.model.clone(),
            effort: command.effort.clone(),
            bare_profile: None,
            smoke_token: None,
        },
    };
    send_request(socket_path, &host_request).await
}

async fn send_request(socket_path: &Path, request: &HostRequest) -> Result<HostResponse, String> {
    let mut stream = UnixStream::connect(socket_path).await.map_err(|_| {
        "host_runner_unavailable: unable to connect to Claude host runner".to_string()
    })?;
    let mut line = serde_json::to_vec(request).map_err(|error| error.to_string())?;
    line.push(b'\n');
    stream
        .write_all(&line)
        .await
        .map_err(|error| error.to_string())?;
    let bytes = read_capped_line(&mut stream).await?;
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}

pub async fn run_server(socket_path: PathBuf) -> Result<(), String> {
    let roots = configured_workspace_roots()?;
    let workspace_policy_id = workspace_policy_id(&roots)?;
    resolve_claude_p()?;
    validate_socket_path(&socket_path, &roots).await?;
    let listener = UnixListener::bind(&socket_path).map_err(|error| error.to_string())?;
    fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))
        .map_err(|error| error.to_string())?;
    let active_pids = Arc::new(Mutex::new(Vec::new()));
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, _)) => {
                        let roots = roots.clone();
                        let workspace_policy_id = workspace_policy_id.clone();
                        let active_pids = active_pids.clone();
                        tokio::spawn(async move {
                            if let Err(error) = handle_connection(stream, roots, workspace_policy_id, active_pids).await {
                                eprintln!("[agent-bridge-host] error code={}", sanitized_code(&error));
                            }
                        });
                    }
                    Err(error) => return Err(error.to_string()),
                }
            }
            _ = shutdown_signal() => {
                terminate_active_children(&active_pids, libc::SIGTERM);
                return Ok(());
            }
        }
    }
}

async fn handle_connection(
    mut stream: UnixStream,
    roots: Vec<PathBuf>,
    workspace_policy_id: String,
    active_pids: Arc<Mutex<Vec<u32>>>,
) -> Result<(), String> {
    let line = match read_capped_line(&mut stream).await {
        Ok(line) => line,
        Err(error) => {
            let response = error_response("invalid_request", &error);
            write_response(&mut stream, &response).await?;
            return Ok(());
        }
    };
    let raw_request: Value = match serde_json::from_slice(&line) {
        Ok(request) => request,
        Err(_) => {
            write_response(
                &mut stream,
                &error_response("invalid_request", "invalid request"),
            )
            .await?;
            return Ok(());
        }
    };
    if raw_request
        .get("version")
        .and_then(Value::as_u64)
        .is_some_and(|version| version as u32 != PROTOCOL_VERSION)
    {
        write_response(
            &mut stream,
            &error_response("protocol_mismatch", "unsupported protocol version"),
        )
        .await?;
        return Ok(());
    }
    if contains_forbidden_execution_descriptor(&raw_request) {
        write_response(
            &mut stream,
            &error_response("protocol_rejected", "protocol rejected"),
        )
        .await?;
        return Ok(());
    }
    let request: HostRequest = match serde_json::from_value(raw_request) {
        Ok(request) => request,
        Err(_) => {
            write_response(
                &mut stream,
                &error_response("invalid_request", "invalid request"),
            )
            .await?;
            return Ok(());
        }
    };
    let monitor_disconnect = matches!(request.kind, HostRequestKind::RunClaude { .. });
    if monitor_disconnect {
        let (mut read_half, mut write_half) = stream.into_split();
        let (disconnect_tx, disconnect_rx) = oneshot::channel();
        let disconnect_task = tokio::spawn(async move {
            let mut byte = [0u8; 1];
            if !matches!(read_half.read(&mut byte).await, Ok(1)) {
                let _ = disconnect_tx.send(());
            }
        });
        let response = handle_request(
            request,
            &roots,
            &workspace_policy_id,
            active_pids,
            Some(disconnect_rx),
        )
        .await;
        disconnect_task.abort();
        write_response(&mut write_half, &response).await
    } else {
        let response =
            handle_request(request, &roots, &workspace_policy_id, active_pids, None).await;
        write_response(&mut stream, &response).await
    }
}

async fn handle_request(
    request: HostRequest,
    roots: &[PathBuf],
    workspace_policy_id: &str,
    active_pids: Arc<Mutex<Vec<u32>>>,
    disconnect: Option<oneshot::Receiver<()>>,
) -> HostResponse {
    if request.version != PROTOCOL_VERSION {
        return error_response("protocol_mismatch", "unsupported protocol version");
    }
    if request.workspace_policy_id != workspace_policy_id {
        return error_response("workspace_policy_mismatch", "workspace policy mismatch");
    }
    match request.kind {
        HostRequestKind::Ping => HostResponse {
            version: PROTOCOL_VERSION,
            ok: true,
            result: Some(HostResult::Pong {
                protocol_version: PROTOCOL_VERSION,
                workspace_policy_id: workspace_policy_id.to_string(),
                ready: true,
            }),
            error: None,
        },
        HostRequestKind::RunClaude {
            cwd,
            timeout_seconds,
            mode,
            prompt,
            model,
            effort,
            bare_profile: _,
            smoke_token: _,
        } => {
            let Ok(cwd) = validate_cwd(&cwd, roots) else {
                return error_response(
                    "cwd_outside_workspace",
                    "cwd is outside configured workspaces",
                );
            };
            if !(MIN_TIMEOUT_SECONDS..=MAX_TIMEOUT_SECONDS).contains(&timeout_seconds) {
                return error_response("invalid_request", "invalid timeout");
            }
            if let Some(effort) = effort.as_deref() {
                let allowed = ["low", "medium", "high", "xhigh", "max"];
                if !allowed.contains(&effort) {
                    return error_response("invalid_request", "invalid effort");
                }
            }
            match run_claude_child(
                &cwd,
                timeout_seconds,
                mode,
                prompt,
                model,
                effort,
                active_pids,
                disconnect,
            )
            .await
            {
                Ok(result) => HostResponse {
                    version: PROTOCOL_VERSION,
                    ok: true,
                    result: Some(HostResult::Run {
                        status: if result.exit_code == Some(0) && result.failure_category.is_none()
                        {
                            "success".to_string()
                        } else {
                            "failure".to_string()
                        },
                        exit_code: result.exit_code,
                        signal: result.signal,
                        duration_ms: result.duration_ms,
                        failure_category: result.failure_category.clone(),
                        pty_output_excerpt: pty_output_excerpt(&result.stdout, &result.stderr),
                        pty_output_truncated: result.stdout_truncated
                            || result.stderr_truncated
                            || result.stdout.len() + result.stderr.len() > MAX_PTY_OUTPUT_EXCERPT,
                        redactions_applied: vec!["prompt".to_string(), "secrets".to_string()],
                        result: if result.exit_code == Some(0) && result.failure_category.is_none()
                        {
                            Some(ClaudeInteractiveSuccess {
                                final_text: result.stdout.clone(),
                                source: "transcript".to_string(),
                                session_id: None,
                            })
                        } else {
                            None
                        },
                        stop: None,
                        stop_failure: None,
                        transcript: json!({
                            "parseStatus": "legacy_pending",
                            "fallbackUsed": false
                        }),
                        stdout: result.stdout,
                        stderr: result.stderr,
                        stdout_truncated: result.stdout_truncated,
                        stderr_truncated: result.stderr_truncated,
                    }),
                    error: None,
                },
                Err(error) => error_response("spawn_failed", &error),
            }
        }
    }
}

fn contains_forbidden_execution_descriptor(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    ["command", "shell", "script", "argv", "executablePath"]
        .iter()
        .any(|field| object.contains_key(*field))
}

#[derive(Debug)]
struct ChildResult {
    exit_code: Option<i32>,
    signal: Option<String>,
    stdout: String,
    stderr: String,
    stdout_truncated: bool,
    stderr_truncated: bool,
    failure_category: Option<String>,
    duration_ms: u64,
}

#[allow(clippy::too_many_arguments)]
async fn run_claude_child(
    cwd: &Path,
    timeout_seconds: i64,
    mode: TaskMode,
    prompt: String,
    model: Option<String>,
    effort: Option<String>,
    active_pids: Arc<Mutex<Vec<u32>>>,
    disconnect: Option<oneshot::Receiver<()>>,
) -> Result<ChildResult, String> {
    let claude_p = resolve_claude_p()?;
    run_claude_child_with_executable(
        claude_p,
        cwd,
        timeout_seconds,
        mode,
        prompt,
        model,
        effort,
        active_pids,
        disconnect,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn run_claude_child_with_executable(
    claude_p: String,
    cwd: &Path,
    timeout_seconds: i64,
    mode: TaskMode,
    prompt: String,
    model: Option<String>,
    effort: Option<String>,
    active_pids: Arc<Mutex<Vec<u32>>>,
    disconnect: Option<oneshot::Receiver<()>>,
) -> Result<ChildResult, String> {
    let started = Instant::now();
    let mut child = Command::new("/bin/zsh");
    let mut inner_args = vec![
        claude_p,
        "--cwd".to_string(),
        cwd.display().to_string(),
        "--timeout".to_string(),
        timeout_seconds.to_string(),
        "--output-format".to_string(),
        "json".to_string(),
    ];
    inner_args.extend(claude_mode_flags(mode));
    if let Some(model) = model {
        inner_args.extend(["--model".to_string(), model]);
    }
    if let Some(effort) = effort {
        inner_args.extend(["--effort".to_string(), effort]);
    }
    child
        .arg("-flc")
        .arg("source ~/.zshenv </dev/null 2>/dev/null || true; source ~/.zprofile </dev/null 2>/dev/null || true; source ~/.zshrc </dev/null 2>/dev/null || true; exec \"$@\"")
        .arg("agent-bridge-claude-host")
        .args(inner_args)
        .current_dir(cwd)
        .envs(crate::provider::provider_env(crate::domain::ProviderKind::Claude))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    configure_child_process_group(&mut child);
    let mut child = child
        .spawn()
        .map_err(|_| "failed to spawn Claude provider".to_string())?;
    let pid = child.id();
    if let Some(pid) = pid
        && let Ok(mut pids) = active_pids.lock()
    {
        pids.push(pid);
    }
    if let Some(mut stdin) = child.stdin.take() {
        tokio::spawn(async move {
            let _ = stdin.write_all(prompt.as_bytes()).await;
        });
    }
    let stdout_task = child.stdout.take().map(read_stream_capped);
    let stderr_task = child.stderr.take().map(read_stream_capped);
    enum WaitOutcome {
        Exited(std::io::Result<std::process::ExitStatus>),
        Timeout,
        ClientDisconnected,
    }

    let wait = tokio::select! {
        result = child.wait() => WaitOutcome::Exited(result),
        _ = tokio::time::sleep(Duration::from_secs(timeout_seconds as u64)) => WaitOutcome::Timeout,
        _ = wait_for_disconnect(disconnect) => WaitOutcome::ClientDisconnected,
    };
    let (status, failure_category) = match wait {
        WaitOutcome::Exited(Ok(status)) => {
            let failure_category = if status.success() {
                None
            } else {
                Some("provider_exit_error".to_string())
            };
            (Some(status), failure_category)
        }
        WaitOutcome::Exited(Err(_)) => (None, Some("provider_exit_error".to_string())),
        WaitOutcome::Timeout => {
            terminate_child_tree(pid, libc::SIGTERM);
            let status = match timeout(CHILD_SHUTDOWN_GRACE, child.wait()).await {
                Ok(result) => result.ok(),
                Err(_) => {
                    terminate_child_tree(pid, libc::SIGKILL);
                    child.wait().await.ok()
                }
            };
            (status, Some("provider_timeout".to_string()))
        }
        WaitOutcome::ClientDisconnected => {
            terminate_child_tree(pid, libc::SIGTERM);
            let status = match timeout(CHILD_SHUTDOWN_GRACE, child.wait()).await {
                Ok(result) => result.ok(),
                Err(_) => {
                    terminate_child_tree(pid, libc::SIGKILL);
                    child.wait().await.ok()
                }
            };
            (status, Some("client_disconnected".to_string()))
        }
    };
    if let Some(pid) = pid
        && let Ok(mut pids) = active_pids.lock()
    {
        pids.retain(|active| *active != pid);
    }
    let (stdout, stdout_truncated) = match stdout_task {
        Some(task) => task.await,
        None => (Vec::new(), false),
    };
    let (stderr, stderr_truncated) = match stderr_task {
        Some(task) => task.await,
        None => (Vec::new(), false),
    };
    Ok(ChildResult {
        exit_code: status.as_ref().and_then(|status| status.code()),
        signal: signal_name(status.as_ref()),
        stdout: String::from_utf8_lossy(&stdout).to_string(),
        stderr: String::from_utf8_lossy(&stderr).to_string(),
        stdout_truncated,
        stderr_truncated,
        failure_category,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

async fn wait_for_disconnect(disconnect: Option<oneshot::Receiver<()>>) {
    match disconnect {
        Some(receiver) => {
            let _ = receiver.await;
        }
        None => std::future::pending::<()>().await,
    }
}

fn resolve_claude_p() -> Result<String, String> {
    if let Ok(path) = env::var("CLAUDE_P_BIN") {
        let candidate = PathBuf::from(&path);
        let metadata =
            fs::metadata(&candidate).map_err(|_| "failed to spawn Claude provider".to_string())?;
        if metadata.permissions().mode() & 0o111 != 0 {
            return Ok(path);
        }
        return Err("failed to spawn Claude provider".to_string());
    }
    let Some(path) = env::var_os("PATH") else {
        return Ok("claude-p".to_string());
    };
    for dir in env::split_paths(&path) {
        let candidate = dir.join("claude-p");
        if let Ok(metadata) = fs::metadata(&candidate)
            && metadata.permissions().mode() & 0o111 != 0
        {
            return Ok(candidate.display().to_string());
        }
    }
    Ok("claude-p".to_string())
}

async fn read_stream_capped(mut reader: impl tokio::io::AsyncRead + Unpin) -> (Vec<u8>, bool) {
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut buffer = [0u8; 8192];
    loop {
        let Ok(count) = reader.read(&mut buffer).await else {
            break;
        };
        if count == 0 {
            break;
        }
        let remaining = MAX_STREAM_BYTES.saturating_sub(bytes.len());
        if remaining == 0 {
            truncated = true;
            continue;
        }
        let take = remaining.min(count);
        bytes.extend_from_slice(&buffer[..take]);
        if take < count {
            truncated = true;
        }
    }
    (bytes, truncated)
}

async fn read_capped_line(stream: &mut UnixStream) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut one = [0u8; 1];
    loop {
        let count = stream
            .read(&mut one)
            .await
            .map_err(|_| "invalid request".to_string())?;
        if count == 0 {
            return Err("invalid request".to_string());
        }
        if one[0] == b'\n' {
            return Ok(bytes);
        }
        if bytes.len() >= MAX_REQUEST_BYTES {
            return Err("request too large".to_string());
        }
        bytes.push(one[0]);
    }
}

async fn write_response(
    stream: &mut (impl AsyncWrite + Unpin),
    response: &HostResponse,
) -> Result<(), String> {
    let mut bytes = serde_json::to_vec(response).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    stream
        .write_all(&bytes)
        .await
        .map_err(|error| error.to_string())
}

fn error_response(code: &str, message: &str) -> HostResponse {
    HostResponse {
        version: PROTOCOL_VERSION,
        ok: false,
        result: None,
        error: Some(HostError {
            code: code.to_string(),
            message: sanitize_message(message),
        }),
    }
}

fn pty_output_excerpt(stdout: &str, stderr: &str) -> String {
    let mut excerpt = String::new();
    if !stdout.is_empty() {
        excerpt.push_str(stdout);
    }
    if !stderr.is_empty() {
        if !excerpt.is_empty() {
            excerpt.push('\n');
        }
        excerpt.push_str(stderr);
    }
    if excerpt.len() > MAX_PTY_OUTPUT_EXCERPT {
        excerpt.truncate(MAX_PTY_OUTPUT_EXCERPT);
    }
    excerpt
}

fn sanitize_message(message: &str) -> String {
    let allowed = [
        "unsupported protocol version",
        "workspace policy mismatch",
        "cwd is outside configured workspaces",
        "invalid timeout",
        "invalid effort",
        "request too large",
        "invalid request",
        "failed to spawn Claude provider",
        "protocol rejected",
    ];
    if allowed.contains(&message) {
        message.to_string()
    } else {
        "host runner error".to_string()
    }
}

fn sanitized_code(error: &str) -> &'static str {
    if error.contains("request too large") {
        "invalid_request"
    } else {
        "host_runner_error"
    }
}

async fn validate_socket_path(socket_path: &Path, roots: &[PathBuf]) -> Result<(), String> {
    let parent = socket_path
        .parent()
        .ok_or_else(|| "socket path must have a parent".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let canonical_parent = parent.canonicalize().map_err(|error| error.to_string())?;
    if roots.iter().any(|root| is_inside(&canonical_parent, root)) {
        return Err("socket path must not be under a workspace".to_string());
    }
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
        .map_err(|error| error.to_string())?;
    let metadata = fs::symlink_metadata(parent).map_err(|error| error.to_string())?;
    if metadata.permissions().mode() & 0o777 != 0o700 {
        return Err("socket directory must be owner-only".to_string());
    }
    if let Ok(metadata) = fs::symlink_metadata(socket_path) {
        let file_type = metadata.file_type();
        if file_type.is_symlink() || file_type.is_file() {
            return Err("unsafe socket path".to_string());
        }
        if file_type.is_socket() {
            let is_live = timeout(Duration::from_millis(250), UnixStream::connect(socket_path))
                .await
                .is_ok_and(|result| result.is_ok());
            handle_existing_socket_file(socket_path, is_live)?;
        }
    }
    Ok(())
}

fn handle_existing_socket_file(socket_path: &Path, is_live: bool) -> Result<(), String> {
    if is_live {
        return Err("host runner socket is already active".to_string());
    }
    fs::remove_file(socket_path).map_err(|error| error.to_string())
}

fn validate_cwd(cwd: &str, roots: &[PathBuf]) -> Result<PathBuf, String> {
    let cwd = PathBuf::from(cwd);
    if cwd
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err("cwd must not contain parent segments".to_string());
    }
    let real_cwd = cwd.canonicalize().map_err(|error| error.to_string())?;
    if roots.iter().any(|root| is_inside(&real_cwd, root)) {
        Ok(real_cwd)
    } else {
        Err("cwd is outside configured workspaces".to_string())
    }
}

fn canonicalize_roots(roots: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    roots
        .iter()
        .map(|root| root.canonicalize().map_err(|error| error.to_string()))
        .collect()
}

fn is_inside(candidate: &Path, root: &Path) -> bool {
    candidate == root || candidate.strip_prefix(root).is_ok()
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

#[cfg(unix)]
fn configure_child_process_group(command: &mut Command) {
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) == 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error())
            }
        });
    }
}

#[cfg(not(unix))]
fn configure_child_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn terminate_child_tree(pid: Option<u32>, signal: i32) {
    if let Some(pid) = pid {
        unsafe {
            libc::killpg(pid as libc::pid_t, signal);
        }
    }
}

#[cfg(not(unix))]
fn terminate_child_tree(_pid: Option<u32>, _signal: i32) {}

fn terminate_active_children(active_pids: &Arc<Mutex<Vec<u32>>>, signal: i32) {
    if let Ok(pids) = active_pids.lock() {
        for pid in pids.iter().copied() {
            terminate_child_tree(Some(pid), signal);
        }
    }
}

#[cfg(unix)]
fn signal_name(status: Option<&std::process::ExitStatus>) -> Option<String> {
    use std::os::unix::process::ExitStatusExt;
    status.and_then(|status| {
        status.signal().map(|signal| match signal {
            libc::SIGTERM => "SIGTERM".to_string(),
            libc::SIGKILL => "SIGKILL".to_string(),
            other => format!("SIG{other}"),
        })
    })
}

#[cfg(not(unix))]
fn signal_name(_status: Option<&std::process::ExitStatus>) -> Option<String> {
    None
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut sigterm = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = sigterm.recv() => {}
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn unique_temp(name: &str) -> PathBuf {
        let path = PathBuf::from("/private/tmp").join(format!(
            "agent-bridge-host-runner-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn host_request(workspace_policy_id: String, kind: HostRequestKind) -> HostRequest {
        HostRequest {
            version: PROTOCOL_VERSION,
            workspace_policy_id,
            kind,
        }
    }

    fn active_pids() -> Arc<Mutex<Vec<u32>>> {
        Arc::new(Mutex::new(Vec::new()))
    }

    fn write_executable(path: &Path, content: &str) {
        fs::write(path, content).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    async fn exchange_raw(
        raw: &[u8],
        roots: Vec<PathBuf>,
        workspace_policy_id: String,
    ) -> HostResponse {
        let (mut client, server) = UnixStream::pair().unwrap();
        let active_pids = active_pids();
        let raw = raw.to_vec();
        let server_task = tokio::spawn(async move {
            handle_connection(server, roots, workspace_policy_id, active_pids)
                .await
                .unwrap();
        });
        client.write_all(&raw).await.unwrap();
        let line = read_capped_line(&mut client).await.unwrap();
        server_task.await.unwrap();
        serde_json::from_slice(&line).unwrap()
    }

    #[test]
    fn workspace_policy_id_uses_sorted_canonical_paths() {
        let root = unique_temp("policy-id");
        let a = root.join("a");
        let b = root.join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();

        let id = workspace_policy_id(&[b.clone(), a.clone()]).unwrap();

        assert_eq!(
            id,
            format!(
                "{}\0{}",
                a.canonicalize().unwrap().display(),
                b.canonicalize().unwrap().display()
            )
        );
    }

    #[tokio::test]
    async fn oversized_request_is_rejected_before_json_parse() {
        let (mut client, mut server) = UnixStream::pair().unwrap();
        tokio::spawn(async move {
            let bytes = vec![b'a'; MAX_REQUEST_BYTES + 1];
            let _ = client.write_all(&bytes).await;
        });

        let error = read_capped_line(&mut server).await.unwrap_err();

        assert_eq!(error, "request too large");
    }

    #[tokio::test]
    async fn unterminated_request_is_rejected() {
        let (mut client, mut server) = UnixStream::pair().unwrap();
        tokio::spawn(async move {
            let _ = client.write_all(br#"{"type":"ping"}"#).await;
            drop(client);
        });

        let error = read_capped_line(&mut server).await.unwrap_err();

        assert_eq!(error, "invalid request");
    }

    #[tokio::test]
    async fn ping_request_returns_ready_policy_metadata() {
        let root = unique_temp("ping");
        let roots = vec![root.canonicalize().unwrap()];
        let policy = workspace_policy_id(&roots).unwrap();
        let response = handle_request(
            host_request(policy.clone(), HostRequestKind::Ping),
            &roots,
            &policy,
            active_pids(),
            None,
        )
        .await;

        assert!(response.ok);
        let Some(HostResult::Pong {
            protocol_version,
            workspace_policy_id,
            ready,
        }) = response.result
        else {
            panic!("expected pong result");
        };
        assert_eq!(protocol_version, PROTOCOL_VERSION);
        assert_eq!(workspace_policy_id, policy);
        assert!(ready);
    }

    #[tokio::test]
    async fn protocol_workspace_timeout_and_effort_validation_fail_before_spawn() {
        let root = unique_temp("validation");
        let roots = vec![root.canonicalize().unwrap()];
        let policy = workspace_policy_id(&roots).unwrap();
        let run_kind = |timeout_seconds, effort: Option<&str>| HostRequestKind::RunClaude {
            cwd: root.display().to_string(),
            timeout_seconds,
            mode: TaskMode::Research,
            prompt: "hello".to_string(),
            model: None,
            effort: effort.map(ToString::to_string),
            bare_profile: None,
            smoke_token: None,
        };

        let unsupported_protocol = handle_request(
            HostRequest {
                version: PROTOCOL_VERSION + 1,
                workspace_policy_id: policy.clone(),
                kind: HostRequestKind::Ping,
            },
            &roots,
            &policy,
            active_pids(),
            None,
        )
        .await;
        assert_eq!(
            unsupported_protocol.error.unwrap().code,
            "protocol_mismatch"
        );

        let wrong_policy = handle_request(
            host_request("different-policy".to_string(), HostRequestKind::Ping),
            &roots,
            &policy,
            active_pids(),
            None,
        )
        .await;
        assert_eq!(
            wrong_policy.error.unwrap().code,
            "workspace_policy_mismatch"
        );

        let low_timeout = handle_request(
            host_request(policy.clone(), run_kind(MIN_TIMEOUT_SECONDS - 1, None)),
            &roots,
            &policy,
            active_pids(),
            None,
        )
        .await;
        assert_eq!(low_timeout.error.unwrap().message, "invalid timeout");

        let high_timeout = handle_request(
            host_request(policy.clone(), run_kind(MAX_TIMEOUT_SECONDS + 1, None)),
            &roots,
            &policy,
            active_pids(),
            None,
        )
        .await;
        assert_eq!(high_timeout.error.unwrap().message, "invalid timeout");

        let bad_effort = handle_request(
            host_request(
                policy.clone(),
                run_kind(MIN_TIMEOUT_SECONDS, Some("extreme")),
            ),
            &roots,
            &policy,
            active_pids(),
            None,
        )
        .await;
        assert_eq!(bad_effort.error.unwrap().message, "invalid effort");
    }

    #[tokio::test]
    async fn command_descriptors_and_unknown_provider_requests_are_rejected() {
        let root = unique_temp("descriptor-reject");
        let roots = vec![root.canonicalize().unwrap()];
        let policy = workspace_policy_id(&roots).unwrap();
        let raw = format!(
            "{{\"version\":{PROTOCOL_VERSION},\"workspacePolicyId\":\"{}\",\"requestType\":\"cursor\",\"command\":\"/bin/sh -c secret\",\"argv\":[\"/bin/sh\"],\"executablePath\":\"/bin/sh\"}}\n",
            policy
        );

        let response = exchange_raw(raw.as_bytes(), roots, policy).await;

        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "protocol_rejected");
    }

    #[tokio::test]
    async fn host_protocol_serializes_v2_request_and_result_schema() {
        let root = unique_temp("protocol-v2");
        let roots = vec![root.canonicalize().unwrap()];
        let policy = workspace_policy_id(&roots).unwrap();
        let request = host_request(
            policy.clone(),
            HostRequestKind::RunClaude {
                cwd: root.display().to_string(),
                timeout_seconds: MIN_TIMEOUT_SECONDS,
                mode: TaskMode::Research,
                prompt: "hello".to_string(),
                model: Some("sonnet".to_string()),
                effort: Some("high".to_string()),
                bare_profile: Some(true),
                smoke_token: Some("AGENT_BRIDGE_PROVIDER_SMOKE_OK".to_string()),
            },
        );

        let request_json = serde_json::to_value(&request).unwrap();
        assert_eq!(request_json["version"], PROTOCOL_VERSION);
        assert_eq!(request_json["requestType"], "claude_interactive");
        assert!(request_json.get("type").is_none());
        assert_eq!(request_json["bareProfile"], true);
        assert_eq!(request_json["smokeToken"], "AGENT_BRIDGE_PROVIDER_SMOKE_OK");

        let response = HostResponse {
            version: PROTOCOL_VERSION,
            ok: true,
            result: Some(HostResult::Run {
                status: "success".to_string(),
                exit_code: Some(0),
                signal: None,
                duration_ms: 42,
                failure_category: None,
                pty_output_excerpt: "done".to_string(),
                pty_output_truncated: false,
                redactions_applied: vec!["prompt".to_string()],
                result: Some(ClaudeInteractiveSuccess {
                    final_text: "done".to_string(),
                    source: "transcript".to_string(),
                    session_id: Some("session".to_string()),
                }),
                stop: None,
                stop_failure: None,
                transcript: json!({"parseStatus": "ok"}),
                stdout: String::new(),
                stderr: String::new(),
                stdout_truncated: false,
                stderr_truncated: false,
            }),
            error: None,
        };
        let response_json = serde_json::to_value(&response).unwrap();
        let run = &response_json["result"];
        assert_eq!(run["responseType"], "claude_interactive_result");
        assert_eq!(run["status"], "success");
        assert_eq!(run["result"]["finalText"], "done");
        assert_eq!(run["ptyOutputExcerpt"], "done");
        assert_eq!(run["redactionsApplied"], json!(["prompt"]));
    }

    #[test]
    fn workspace_policy_id_fails_for_noncanonical_workspace() {
        let root = unique_temp("missing-policy-root");
        let missing = root.join("missing");

        let error = workspace_policy_id(&[missing]).unwrap_err();

        assert!(!error.is_empty());
    }

    #[test]
    fn symlink_escape_cwd_is_rejected() {
        let root = unique_temp("symlink-root");
        let outside = unique_temp("symlink-outside");
        let link = root.join("link-out");
        symlink(&outside, &link).unwrap();
        let roots = vec![root.canonicalize().unwrap()];

        let error = validate_cwd(&link.display().to_string(), &roots).unwrap_err();

        assert!(!error.is_empty());
    }

    #[tokio::test]
    async fn socket_path_rejects_workspace_file_and_symlink_paths() {
        let root = unique_temp("socket-root");
        let roots = vec![root.canonicalize().unwrap()];
        let workspace_socket = root.join("claude.sock");
        assert_eq!(
            validate_socket_path(&workspace_socket, &roots)
                .await
                .unwrap_err(),
            "socket path must not be under a workspace"
        );

        let socket_dir = unique_temp("socket-unsafe");
        let file_socket = socket_dir.join("file.sock");
        fs::write(&file_socket, "not a socket").unwrap();
        assert_eq!(
            validate_socket_path(&file_socket, &roots)
                .await
                .unwrap_err(),
            "unsafe socket path"
        );

        let symlink_socket = socket_dir.join("link.sock");
        let target = socket_dir.join("target.sock");
        fs::write(&target, "not a socket").unwrap();
        symlink(&target, &symlink_socket).unwrap();
        assert_eq!(
            validate_socket_path(&symlink_socket, &roots)
                .await
                .unwrap_err(),
            "unsafe socket path"
        );
    }

    #[test]
    fn socket_path_rejects_live_socket_and_removes_stale_socket() {
        let socket_dir = unique_temp("socket-live");
        let socket_path = socket_dir.join("claude.sock");
        fs::write(&socket_path, "placeholder").unwrap();

        assert_eq!(
            handle_existing_socket_file(&socket_path, true).unwrap_err(),
            "host runner socket is already active"
        );
        assert!(socket_path.exists());

        handle_existing_socket_file(&socket_path, false).unwrap();
        assert!(!socket_path.exists());
    }

    #[tokio::test]
    async fn stream_capture_is_bounded_and_marks_truncation() {
        let (mut writer, reader) = tokio::io::duplex(8192);
        let writer_task = tokio::spawn(async move {
            let bytes = vec![b'x'; MAX_STREAM_BYTES + 4096];
            writer.write_all(&bytes).await.unwrap();
        });

        let (bytes, truncated) = read_stream_capped(reader).await;

        writer_task.await.unwrap();
        assert_eq!(bytes.len(), MAX_STREAM_BYTES);
        assert!(truncated);
    }

    #[test]
    fn runner_error_messages_and_logs_are_sanitized() {
        let response = error_response(
            "spawn_failed",
            "/Users/pedro/Development/secret token prompt stdout stderr",
        );

        assert_eq!(response.error.unwrap().message, "host runner error");
        assert_eq!(
            sanitized_code("request too large /Users/pedro/secret"),
            "invalid_request"
        );
        assert_eq!(
            sanitized_code("/Users/pedro/secret prompt token"),
            "host_runner_error"
        );
    }

    #[tokio::test]
    async fn client_disconnect_terminates_and_reaps_child() {
        let root = unique_temp("disconnect-child");
        let fake_claude = root.join("claude-p");
        write_executable(
            &fake_claude,
            "#!/bin/sh\ntrap 'exit 143' TERM\nwhile true; do sleep 1; done\n",
        );
        let active_pids = active_pids();
        let (disconnect_tx, disconnect_rx) = oneshot::channel();

        let child = run_claude_child_with_executable(
            fake_claude.display().to_string(),
            &root,
            30,
            TaskMode::Research,
            "hello".to_string(),
            None,
            None,
            active_pids.clone(),
            Some(disconnect_rx),
        );
        tokio::pin!(child);
        tokio::select! {
            result = &mut child => panic!("child exited before disconnect: {result:?}"),
            _ = tokio::time::sleep(Duration::from_millis(200)) => {}
        }
        disconnect_tx.send(()).unwrap();

        let result = timeout(Duration::from_secs(5), child)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            result.failure_category.as_deref(),
            Some("client_disconnected")
        );
        assert!(active_pids.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn shutdown_cleanup_terminates_active_child_process_group() {
        let mut child = Command::new("/bin/sh");
        child
            .arg("-c")
            .arg("trap 'exit 143' TERM; while true; do sleep 1; done")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        configure_child_process_group(&mut child);
        let mut child = child.spawn().unwrap();
        let pid = child.id().unwrap();
        let active_pids = active_pids();
        active_pids.lock().unwrap().push(pid);

        terminate_active_children(&active_pids, libc::SIGTERM);
        let status = timeout(Duration::from_secs(5), child.wait())
            .await
            .unwrap()
            .unwrap();

        assert!(!status.success());
    }
}
