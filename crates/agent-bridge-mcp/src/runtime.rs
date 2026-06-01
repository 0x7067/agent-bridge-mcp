use crate::mcp::{JsonRpcId, JsonRpcRequest, JsonRpcResponse};
use crate::server::handle_request;
use crate::task::TaskManagerHandle;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

pub async fn main_entry() {
    install_panic_hook();
    if std::env::var("AGENT_BRIDGE_FORCE_PANIC").ok().as_deref() == Some("1") {
        panic!("forced panic for integration test");
    }
    if let Err(error) = run_until_shutdown().await {
        eprintln!("[agent-bridge] fatal {error}");
        std::process::exit(1);
    }
}

async fn run_until_shutdown() -> io::Result<()> {
    tokio::select! {
        result = run_stdio_server() => result,
        exit_code = shutdown_signal() => {
            if let Ok(manager) = TaskManagerHandle::from_env().await {
                let _ = manager.shutdown().await;
            }
            std::process::exit(exit_code);
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
        eprintln!("[agent-bridge] panic {info}");
    }));
}
