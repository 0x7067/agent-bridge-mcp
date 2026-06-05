use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

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
pub fn terminate_process_group(pid: u32, signal: libc::c_int) {
    unsafe {
        libc::killpg(pid as libc::pid_t, signal);
    }
}

fn to_pty_size(size: PtySize) -> pty_process::Size {
    pty_process::Size::new(size.rows, size.cols)
}

fn to_io_error(error: impl ToString) -> io::Error {
    io::Error::other(error.to_string())
}
