//! Task actors: each AX task runs as an isolated child process — the local,
//! dependency-free equivalent of AX's suspendable actor images.
//!
//! * stdout/stderr are appended to `.ax/logs/<task>.log` (durable, survives
//!   controller restarts, backs `fathom ax logs`).
//! * Suspend/resume map to SIGSTOP/SIGCONT on the process.
//! * Liveness is probed with `kill(pid, 0)` so the controller can reattach
//!   to still-running actors after a restart, or detect loss and mark the
//!   task Interrupted (resumable via `ax resume`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use std::os::unix::process::CommandExt;
use std::process::{Child, Command};

use crate::error::{AxError, AxResult};

/// Command resolved for a task's actor.
#[derive(Debug, Clone)]
pub struct ActorSpec {
    /// argv — either the manifest's `spec.command` or the Fathom harness
    /// invocation synthesized by the controller.
    pub argv: Vec<String>,
    /// Working directory (first bound workspace path, else the state dir).
    pub cwd: PathBuf,
    /// Fully assembled environment for the actor.
    pub env: HashMap<String, String>,
}

/// Spawn the actor process: stdin null, stdout+stderr tee'd to `log_path`.
pub fn spawn(spec: &ActorSpec, log_path: &Path) -> AxResult<(Child, i64)> {
    let Some(program) = spec.argv.first() else {
        return Err(AxError::Actor("empty actor argv".into()));
    };
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    let err = log.try_clone()?;

    let mut cmd = Command::new(program);
    cmd.args(&spec.argv[1..])
        .current_dir(&spec.cwd)
        .stdin(std::process::Stdio::null())
        .stdout(log)
        .stderr(err)
        .envs(&spec.env);
    // Own process group: suspend/kill target the group, covering
    // grandchildren the actor spawns.
    cmd.process_group(0);

    let child = cmd
        .spawn()
        .map_err(|e| AxError::Actor(format!("spawn '{}': {e}", spec.argv.join(" "))))?;
    let pid = child.id() as i64;
    Ok((child, pid))
}

/// SIGSTOP the actor's process group (suspend).
pub fn suspend(pid: i64) -> AxResult<()> {
    group_signal(pid, libc::SIGSTOP).map_err(|e| AxError::Actor(format!("suspend pid {pid}: {e}")))
}

/// SIGCONT the actor's process group (resume in place).
pub fn resume(pid: i64) -> AxResult<()> {
    group_signal(pid, libc::SIGCONT).map_err(|e| AxError::Actor(format!("resume pid {pid}: {e}")))
}

/// Signal the whole process group (negative pid), falling back to the
/// process alone when the group signal fails.
fn group_signal(pid: i64, sig: i32) -> Result<(), std::io::Error> {
    match signal(-pid, sig) {
        Ok(()) => Ok(()),
        Err(e) => signal(pid, sig).map_err(|_| e),
    }
}

/// SIGTERM the process group, then escalate to SIGKILL after `grace_ms`.
pub async fn kill(pid: i64, grace_ms: u64) {
    if pid <= 0 {
        return;
    }
    let _ = signal(-pid, libc::SIGTERM); // group first
    if signal(pid, libc::SIGTERM).is_err() && signal(-pid, libc::SIGTERM).is_err() {
        return; // already gone
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(grace_ms);
    while std::time::Instant::now() < deadline {
        if !alive(pid) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let _ = signal(-pid, libc::SIGKILL);
    let _ = signal(pid, libc::SIGKILL);
}

/// Probe whether pid (or its process group) is still running.
pub fn alive(pid: i64) -> bool {
    pid > 0 && signal(pid, 0).is_ok()
}

fn signal(pid: i64, sig: i32) -> Result<(), std::io::Error> {
    let rc = unsafe { libc::kill(pid as libc::pid_t, sig) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Tail a log file: returns `(offset, bytes_read)` starting at `offset`.
pub fn read_log(path: &Path, offset: u64, max_bytes: usize) -> AxResult<(u64, Vec<u8>)> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((offset, Vec::new())),
        Err(e) => return Err(e.into()),
    };
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; max_bytes];
    let n = f.read(&mut buf)?;
    buf.truncate(n);
    Ok((offset + n as u64, buf))
}
