use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::Duration;
use tokio::time::timeout;

/// Hard upper bound on how long to wait for a child to be reaped after SIGKILL.
/// A process whose exit status the OS/runtime cannot surface within this window
/// (e.g. the status was already consumed elsewhere) is reported with a
/// best-effort SIGKILL status rather than waited on indefinitely — mirroring the
/// generic provider teardown in `task.rs` (`SIGKILL_REAP_GRACE`).
const SIGKILL_REAP_GRACE: Duration = Duration::from_secs(1);

pub struct PtySpawn {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
    pub size: PtySize,
    pub resize_after_open: Option<PtySize>,
}

#[derive(Clone, Copy)]
pub struct PtySize {
    pub rows: u16,
    pub cols: u16,
}

pub struct PtySession {
    pub pid: Option<u32>,
    pub child: tokio::process::Child,
    pub reader: pty_process::OwnedReadPty,
    pub writer: pty_process::OwnedWritePty,
}

impl PtySession {
    #[cfg(unix)]
    pub async fn terminate_with_grace(&mut self, grace: Duration) -> io::Result<ExitStatus> {
        if let Some(pid) = self.pid {
            terminate_process_tree(pid, libc::SIGTERM);
        }
        match timeout(grace, self.child.wait()).await {
            Ok(status) => status,
            Err(_) => {
                if let Some(pid) = self.pid {
                    terminate_process_tree(pid, libc::SIGKILL);
                }
                // Bound the post-SIGKILL reap. If the runtime cannot surface the
                // child's exit status within the grace window (observed when the
                // status was consumed by another reaper, leaving `wait()` to block
                // forever), report a best-effort SIGKILL status so the run can
                // still finalize instead of hanging teardown indefinitely.
                match timeout(SIGKILL_REAP_GRACE, self.child.wait()).await {
                    Ok(status) => status,
                    Err(_) => Ok(sigkill_exit_status()),
                }
            }
        }
    }
}

/// Best-effort `ExitStatus` representing a child terminated by SIGKILL, used when
/// the real status cannot be reaped within [`SIGKILL_REAP_GRACE`].
#[cfg(unix)]
fn sigkill_exit_status() -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    // Raw wait-status whose low 7 bits encode the terminating signal.
    ExitStatus::from_raw(libc::SIGKILL)
}

pub fn spawn(spec: PtySpawn) -> io::Result<PtySession> {
    let (pty, pts) = pty_process::open().map_err(to_io_error)?;
    pty.resize(to_pty_size(spec.size)).map_err(to_io_error)?;
    if let Some(size) = spec.resize_after_open {
        pty.resize(to_pty_size(size)).map_err(to_io_error)?;
    }
    let command = pty_process::Command::new(&spec.program)
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .envs(&spec.env)
        .kill_on_drop(true);
    let child = command.spawn(pts).map_err(to_io_error)?;
    let pid = child.id();
    let (reader, writer) = pty.into_split();
    Ok(PtySession {
        pid,
        child,
        reader,
        writer,
    })
}

#[cfg(unix)]
pub fn terminate_process_tree(pid: u32, signal: libc::c_int) {
    unsafe {
        libc::killpg(pid as libc::pid_t, signal);
        libc::kill(pid as libc::pid_t, signal);
    }
}

fn to_pty_size(size: PtySize) -> pty_process::Size {
    pty_process::Size::new(size.rows, size.cols)
}

fn to_io_error(error: impl ToString) -> io::Error {
    io::Error::other(error.to_string())
}
