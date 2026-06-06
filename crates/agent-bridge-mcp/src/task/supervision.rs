//! Child-process supervision helpers: the active process-group registry plus
//! the low-level primitives for grouping, terminating, and naming signals.
//!
//! The registry is modeled as an `ActivePids` value rather than a bare global.
//! Production uses a single process-global instance (`ACTIVE_PIDS`) because the
//! panic hook has no handle to thread through; tests construct isolated local
//! instances so signal-delivery can be exercised without a shared global that
//! would cross-kill other tests' children.

use std::collections::BTreeSet;
use std::sync::Mutex;
use tokio::process::Command as ProcessCommand;

/// Tracks live child process-group ids and can signal them as a group.
pub(crate) struct ActivePids {
    inner: Mutex<BTreeSet<u32>>,
}

impl ActivePids {
    pub(crate) const fn new() -> Self {
        Self {
            inner: Mutex::new(BTreeSet::new()),
        }
    }

    pub(crate) fn register(&self, pid: u32) {
        if let Ok(mut pids) = self.inner.lock() {
            pids.insert(pid);
        }
    }

    pub(crate) fn unregister(&self, pid: u32) {
        if let Ok(mut pids) = self.inner.lock() {
            pids.remove(&pid);
        }
    }

    #[cfg(test)]
    pub(crate) fn contains(&self, pid: u32) -> bool {
        self.inner
            .lock()
            .map(|pids| pids.contains(&pid))
            .unwrap_or(false)
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.inner
            .lock()
            .map(|pids| pids.is_empty())
            .unwrap_or(true)
    }

    /// Best-effort termination of every tracked child process group. Safe to
    /// call from the panic hook: it only takes a lock and issues signals.
    pub(crate) fn terminate_all(&self, signal: i32) {
        let pids: Vec<u32> = self
            .inner
            .lock()
            .map(|pids| pids.iter().copied().collect())
            .unwrap_or_default();
        for pid in pids {
            terminate_child_tree(pid, signal);
        }
    }
}

/// Process-global registry consulted by the panic hook and host shutdown path.
pub(crate) static ACTIVE_PIDS: ActivePids = ActivePids::new();

pub(crate) fn register_active_pid(pid: u32) {
    ACTIVE_PIDS.register(pid);
}

pub(crate) fn unregister_active_pid(pid: u32) {
    ACTIVE_PIDS.unregister(pid);
}

pub(crate) fn terminate_all_active_pids(signal: i32) {
    ACTIVE_PIDS.terminate_all(signal);
}

#[cfg(unix)]
pub(crate) fn configure_child_process_group(command: &mut ProcessCommand) {
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
pub(crate) fn configure_child_process_group(_command: &mut ProcessCommand) {}

#[cfg(unix)]
pub(crate) fn terminate_child_tree(pid: u32, signal: i32) {
    unsafe {
        libc::killpg(pid as libc::pid_t, signal);
    }
}

#[cfg(not(unix))]
pub(crate) fn terminate_child_tree(_pid: u32, _signal: i32) {}

#[cfg(unix)]
pub(crate) fn signal_name(status: &std::process::ExitStatus) -> Option<String> {
    use std::os::unix::process::ExitStatusExt;
    status.signal().map(|signal| match signal {
        libc::SIGTERM => "SIGTERM".to_string(),
        libc::SIGKILL => "SIGKILL".to_string(),
        other => format!("SIG{other}"),
    })
}

#[cfg(not(unix))]
pub(crate) fn signal_name(_status: &std::process::ExitStatus) -> Option<String> {
    None
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::process::Stdio;
    use tokio::time::{Duration, timeout};

    #[test]
    fn registry_tracks_and_clears() {
        let registry = ActivePids::new();
        registry.register(4242);
        assert!(registry.contains(4242));
        registry.unregister(4242);
        assert!(registry.is_empty());
    }

    fn spawn_sleep() -> tokio::process::Child {
        let mut command = ProcessCommand::new("/bin/sleep");
        command
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_child_process_group(&mut command);
        command.spawn().unwrap()
    }

    #[tokio::test]
    async fn terminate_child_tree_sends_sigterm_to_unix_process_group() {
        let mut child = spawn_sleep();
        let pid = child.id().unwrap();
        terminate_child_tree(pid, libc::SIGTERM);
        let status = timeout(Duration::from_secs(3), child.wait())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(signal_name(&status).as_deref(), Some("SIGTERM"));
    }

    #[tokio::test]
    async fn terminate_all_signals_only_registered_children() {
        // A local registry instance is fully isolated from any concurrently
        // running test, so terminating "all" of its children is safe.
        let registry = ActivePids::new();
        let mut child = spawn_sleep();
        let pid = child.id().unwrap();
        registry.register(pid);

        registry.terminate_all(libc::SIGTERM);
        let status = timeout(Duration::from_secs(3), child.wait())
            .await
            .unwrap()
            .unwrap();
        registry.unregister(pid);

        assert_eq!(signal_name(&status).as_deref(), Some("SIGTERM"));
        assert!(registry.is_empty());
    }
}
