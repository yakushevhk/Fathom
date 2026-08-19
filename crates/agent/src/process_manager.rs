//! Process manager for spawning and monitoring worker processes.
//! Each worker is a separate OS process communicating via Unix domain sockets.
//!
//! Lifecycle guarantees (fleet report B14):
//! - Workers are spawned with `kill_on_drop(true)`, so any `Child` handle that
//!   is dropped (error paths, `ProcessManager` drop) kills the OS process.
//! - The socket-wait timeout explicitly kills and reaps the worker, so a hung
//!   worker cannot keep burning LLM budget or holding a DB handle.
//! - Socket reads are capped at [`MAX_LINE_BYTES`]: a runaway/malformed worker
//!   cannot balloon coordinator memory with an unterminated line; over-long
//!   lines are treated as a protocol error instead.
//! - `shutdown_all` cancels, then explicitly kills and reaps every child that
//!   is still alive.

use crate::ipc::IpcMessage;
use pr_core::{AgentEvent, AgentId, AgentRole};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::Child;
use tokio::sync::broadcast;

/// Maximum length (in bytes) of a single IPC line read from a worker socket.
///
/// Legitimate messages (progress, tool-call previews, summaries) are small;
/// 8 MiB leaves generous headroom while bounding what a runaway or malicious
/// worker can force the coordinator to allocate.
const MAX_LINE_BYTES: usize = 8 * 1024 * 1024;

/// Default startup timeout: how long `spawn_worker` waits for a worker to
/// create its socket before the worker is killed.
const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

/// Compute a short, filesystem-safe socket filename for an agent.
///
/// Unix domain socket paths are limited to ~104 bytes (`SUN_LEN`). A full
/// agent id (a 36-char UUID) combined with a deep output directory can
/// exceed that, so we hash the id into a short 16-hex-char name. The worker
/// receives its agent id separately over the CLI, so it does not need to
/// recover it from the socket filename. Within a single session socket dir
/// the number of workers is small, making collisions a non-issue in practice.
fn short_socket_name(agent_id: &AgentId) -> String {
    let mut hasher = DefaultHasher::new();
    agent_id.0.hash(&mut hasher);
    format!("{:016x}.sock", hasher.finish())
}

/// Handle to a running worker process.
pub struct WorkerHandle {
    pub agent_id: AgentId,
    pub process: Child,
    pub socket: UnixStream,
    pub socket_path: PathBuf,
}

/// Manages spawning and communication with worker processes.
pub struct ProcessManager {
    socket_dir: PathBuf,
    workers: HashMap<AgentId, WorkerHandle>,
    binary_path: PathBuf,
    /// How long `spawn_worker` waits for the worker socket before killing
    /// the worker. Kept private; tests tune it directly.
    startup_timeout: Duration,
}

impl ProcessManager {
    pub fn new(socket_dir: PathBuf) -> Self {
        let binary_path = std::env::current_exe().unwrap_or_default();
        Self::with_binary_path(socket_dir, binary_path)
    }

    /// Create a manager that spawns a specific binary as workers.
    ///
    /// `new` uses the current executable; this variant is useful for tests
    /// (where the current executable is the test harness) and for embedding
    /// the coordinator in a different host binary.
    pub fn with_binary_path(socket_dir: PathBuf, binary_path: PathBuf) -> Self {
        std::fs::create_dir_all(&socket_dir).ok();
        Self {
            socket_dir,
            workers: HashMap::new(),
            binary_path,
            startup_timeout: DEFAULT_STARTUP_TIMEOUT,
        }
    }

    /// Spawn a new worker process.
    ///
    /// If the worker does not open its socket within the startup timeout (or
    /// exits before doing so), it is explicitly killed and reaped before this
    /// method returns an error — no orphaned worker can outlive a failed
    /// spawn.
    pub async fn spawn_worker(
        &mut self,
        agent_id: AgentId,
        session_id: &str,
        task: String,
        role: AgentRole,
    ) -> anyhow::Result<()> {
        let socket_path = self.socket_dir.join(short_socket_name(&agent_id));
        
        // Remove existing socket if present
        let _ = std::fs::remove_file(&socket_path);

        // Spawn worker process
        let mut cmd = tokio::process::Command::new(&self.binary_path);
        // Belt and braces: if the Child handle is ever dropped (spawn timeout
        // below, ProcessManager drop, ...), tokio kills the OS process.
        cmd.kill_on_drop(true);
        cmd.arg("worker")
            .arg("--session-id").arg(session_id)
            .arg("--agent-id").arg(&agent_id.0)
            .arg("--task").arg(&task)
            .arg("--socket").arg(&socket_path)
            .arg("--role").arg(role.to_string());

        // Tell the worker where the session output directory lives so it
        // opens the same SQLite database as the coordinator. The socket dir
        // is conventionally `<output_dir>/.sockets`.
        if let Some(output_dir) = self.socket_dir.parent() {
            cmd.env("PR_OUTPUT_DIR", output_dir);
        }

        let mut process = cmd.spawn()?;

        // Wait for the worker to create its socket and connect. On failure
        // wait_for_socket has already killed and reaped the child.
        let socket = match wait_for_socket(&socket_path, &mut process, self.startup_timeout).await
        {
            Ok(stream) => stream,
            Err(e) => {
                let _ = std::fs::remove_file(&socket_path);
                return Err(e.context(format!("worker {agent_id} failed to start")));
            }
        };

        let handle = WorkerHandle {
            agent_id: agent_id.clone(),
            process,
            socket,
            socket_path,
        };

        self.workers.insert(agent_id, handle);
        Ok(())
    }

    /// Send a message to a worker.
    pub async fn send_message(&mut self, agent_id: &AgentId, msg: IpcMessage) -> anyhow::Result<()> {
        let handle = self.workers.get_mut(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Worker not found: {}", agent_id))?;

        let line = msg.to_line();
        handle.socket.write_all(line.as_bytes()).await?;
        handle.socket.flush().await?;
        Ok(())
    }

    /// Read messages from a worker until completion or failure.
    pub async fn wait_for_completion(&mut self, agent_id: &AgentId) -> anyhow::Result<WorkerResult> {
        self.wait_for_completion_with_events(agent_id, None).await
    }

    /// Read messages from a worker until completion or failure.
    ///
    /// When `event_tx` is provided, intermediate worker messages (progress,
    /// tool calls, LLM chunks) are re-emitted as [`AgentEvent`]s on the
    /// coordinator's local event bus so the TUI / headless output stays live
    /// even though the agent runs in a separate OS process.
    ///
    /// Lines are read with a hard size cap ([`MAX_LINE_BYTES`]); a worker
    /// that violates the cap (or sends non-UTF-8 data) fails the wait with
    /// [`WorkerResult::Failed`] instead of ballooning memory.
    pub async fn wait_for_completion_with_events(
        &mut self,
        agent_id: &AgentId,
        event_tx: Option<&broadcast::Sender<AgentEvent>>,
    ) -> anyhow::Result<WorkerResult> {
        let handle = self.workers.get_mut(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Worker not found: {}", agent_id))?;

        let mut reader = BufReader::new(&mut handle.socket);
        let mut buf: Vec<u8> = Vec::new();

        loop {
            let n = match read_line_capped(&mut reader, &mut buf, MAX_LINE_BYTES).await {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                    // Protocol violation (over-long line): fail the worker
                    // cleanly rather than propagating an IO error or OOMing.
                    return Ok(WorkerResult::Failed {
                        error: format!("worker protocol error: {e}"),
                    });
                }
                Err(e) => return Err(e.into()),
            };
            if n == 0 {
                // Socket closed - worker exited
                return Ok(WorkerResult::Disconnected);
            }

            let line = match std::str::from_utf8(&buf) {
                Ok(s) => s,
                Err(_) => {
                    return Ok(WorkerResult::Failed {
                        error: "worker protocol error: invalid UTF-8 in message line"
                            .to_string(),
                    });
                }
            };

            if let Some(msg) = IpcMessage::from_line(line) {
                match msg {
                    IpcMessage::Completed { summary, tokens_used, .. } => {
                        return Ok(WorkerResult::Completed { summary, tokens_used });
                    }
                    IpcMessage::Failed { error, .. } => {
                        return Ok(WorkerResult::Failed { error });
                    }
                    // Other messages (Progress, ToolCall, etc.) are forwarded
                    // to the local event bus (if requested) and logged.
                    other => {
                        if let Some(tx) = event_tx {
                            if let Some(event) = other.to_agent_event() {
                                let _ = tx.send(event);
                            }
                        }
                        tracing::debug!("Worker message: {:?}", other);
                    }
                }
            }
        }
    }

    /// Shutdown all workers gracefully.
    ///
    /// Sends `Cancel` to every worker, gives them a short grace period, then
    /// explicitly kills and reaps every child that is still alive and removes
    /// the socket files. No live worker or zombie outlives this call.
    pub async fn shutdown_all(&mut self) {
        // Collect agent IDs first to avoid borrow issues
        let agent_ids: Vec<AgentId> = self.workers.keys().cloned().collect();

        for agent_id in &agent_ids {
            // Send cancel message
            let _ = self.send_cancel(agent_id).await;
        }

        // Wait a bit for graceful shutdown
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Force kill and cleanup every child that survived the grace period.
        // kill_on_drop(true) would also fire if the handles were merely
        // dropped, but we kill and reap explicitly so shutdown is leak-proof
        // even if a drop is deferred.
        for (_, mut handle) in self.workers.drain() {
            match handle.process.try_wait() {
                Ok(None) => kill_and_reap(&mut handle.process).await,
                Ok(Some(_)) => {} // already exited (try_wait reaped it)
                Err(e) => {
                    tracing::warn!(
                        "Failed to check worker {} during shutdown: {e}",
                        handle.agent_id
                    );
                    kill_and_reap(&mut handle.process).await;
                }
            }
            let _ = std::fs::remove_file(&handle.socket_path);
        }
    }

    async fn send_cancel(&mut self, agent_id: &AgentId) -> anyhow::Result<()> {
        self.send_message(agent_id, IpcMessage::Cancel).await
    }

    /// Get the number of active workers.
    pub fn active_count(&self) -> usize {
        self.workers.len()
    }

    /// Check if a worker is still running.
    pub fn is_running(&mut self, agent_id: &AgentId) -> bool {
        if let Some(handle) = self.workers.get_mut(agent_id) {
            matches!(handle.process.try_wait(), Ok(None))
        } else {
            false
        }
    }
}

/// Poll until a worker creates its Unix socket and we can connect.
///
/// Leak-proofing: on timeout the child is explicitly killed and reaped before
/// returning an error, and if the child exits before creating the socket we
/// fail fast instead of waiting out the full timeout. A hung worker can thus
/// never survive a failed spawn.
async fn wait_for_socket(
    path: &Path,
    child: &mut Child,
    timeout: Duration,
) -> anyhow::Result<UnixStream> {
    let start = Instant::now();

    loop {
        if start.elapsed() > timeout {
            kill_and_reap(child).await;
            anyhow::bail!(
                "Timeout waiting for worker socket {} after {:?}",
                path.display(),
                timeout
            );
        }

        // Fail fast if the worker already exited: it will never create the
        // socket. (try_wait reaps the child when it observes the exit.)
        match child.try_wait() {
            Ok(Some(status)) => {
                anyhow::bail!(
                    "Worker exited with {status} before creating socket {}",
                    path.display()
                );
            }
            Ok(None) => {}
            Err(e) => anyhow::bail!("Failed to check worker status: {e}"),
        }

        match UnixStream::connect(path).await {
            Ok(stream) => return Ok(stream),
            Err(_) => {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }
}

/// Kill a child process and reap it. Errors are ignored: a child that is
/// already dead counts as success here.
async fn kill_and_reap(child: &mut Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}

/// Pure line-assembly step used by [`read_line_capped`]: given the number of
/// bytes already accumulated for the current line (`line_len`) and the next
/// buffered chunk, decide how many bytes belong to this line and whether the
/// line is complete.
///
/// Returns `Ok((take, done))` where `take` bytes are copied from the start of
/// `chunk` (including the newline terminator when `done`), or an
/// [`std::io::ErrorKind::InvalidData`] error if copying them would push the
/// line past `max_len`. Bytes after the newline belong to the next line and
/// are left for the caller to consume separately.
fn take_capped(line_len: usize, chunk: &[u8], max_len: usize) -> std::io::Result<(usize, bool)> {
    let too_long = |max_len: usize| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("line exceeds {max_len} bytes"),
        )
    };
    match chunk.iter().position(|&b| b == b'\n') {
        Some(pos) => {
            let take = pos + 1;
            if line_len + take > max_len {
                return Err(too_long(max_len));
            }
            Ok((take, true))
        }
        None => {
            if line_len + chunk.len() > max_len {
                return Err(too_long(max_len));
            }
            Ok((chunk.len(), false))
        }
    }
}

/// Read a single newline-terminated line from `reader`, capping the line at
/// `max_len` bytes so a runaway or malicious worker cannot balloon memory.
///
/// Follows `AsyncBufReadExt::read_line` semantics: returns `Ok(0)` on EOF
/// with no (partial) line pending, otherwise the number of bytes read (a
/// trailing partial line at EOF is returned as-is). Returns
/// [`std::io::ErrorKind::InvalidData`] if the line would exceed `max_len`.
async fn read_line_capped<R>(
    reader: &mut R,
    line: &mut Vec<u8>,
    max_len: usize,
) -> std::io::Result<usize>
where
    R: AsyncBufRead + Unpin,
{
    line.clear();
    loop {
        let (take, done) = {
            let chunk = reader.fill_buf().await?;
            if chunk.is_empty() {
                // EOF; return any partial line (may be 0).
                return Ok(line.len());
            }
            let (take, done) = take_capped(line.len(), chunk, max_len)?;
            line.extend_from_slice(&chunk[..take]);
            (take, done)
        };
        reader.consume(take);
        if done {
            return Ok(line.len());
        }
    }
}

/// Result from waiting for a worker to complete.
#[derive(Debug)]
pub enum WorkerResult {
    Completed { summary: String, tokens_used: u64 },
    Failed { error: String },
    Disconnected,
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        // Sockets are cleaned up here; the child processes themselves are
        // killed via `kill_on_drop(true)` set at spawn time.
        for handle in self.workers.values() {
            let _ = std::fs::remove_file(&handle.socket_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// True if a process with this pid still exists. Uses `/bin/kill -0`,
    /// which signals nothing but checks existence. Stderr is silenced because
    /// "No such process" is the expected outcome for a reaped worker.
    #[cfg(unix)]
    fn pid_alive(pid: u32) -> bool {
        std::process::Command::new("/bin/kill")
            .arg("-0")
            .arg(pid.to_string())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[test]
    fn test_process_manager_creation() {
        let dir = std::env::temp_dir().join("pr-test-sockets");
        let pm = ProcessManager::new(dir.clone());
        assert_eq!(pm.active_count(), 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_worker_result_variants_are_distinct() {
        let completed = WorkerResult::Completed {
            summary: "done".to_string(),
            tokens_used: 5,
        };
        let failed = WorkerResult::Failed {
            error: "boom".to_string(),
        };
        let disconnected = WorkerResult::Disconnected;
        assert!(matches!(completed, WorkerResult::Completed { .. }));
        assert!(matches!(failed, WorkerResult::Failed { .. }));
        assert!(matches!(disconnected, WorkerResult::Disconnected));
    }

    #[test]
    fn test_wait_for_unknown_worker_errors() {
        // A synchronous harness is enough: wait_for_completion errors before
        // doing any IO when the agent id is not registered.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let dir = std::env::temp_dir().join(format!("pr-pm-unknown-{}", std::process::id()));
        let mut pm = ProcessManager::new(dir.clone());
        let result = rt.block_on(pm.wait_for_completion(&pr_core::AgentId::new()));
        assert!(result.is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_short_socket_name_is_short_and_stable() {
        let id = pr_core::AgentId::new();
        let a = short_socket_name(&id);
        let b = short_socket_name(&id);
        assert_eq!(a, b, "name must be deterministic");
        assert!(a.ends_with(".sock"));
        // 16 hex chars + ".sock" = 21 chars, well under SUN_LEN headroom.
        assert_eq!(a.len(), 21);

        let other = short_socket_name(&pr_core::AgentId::new());
        assert_ne!(a, other, "distinct ids should map to distinct sockets");
    }

    // ------------------------------------------------------------------
    // Line-cap guard (pure function tests)
    // ------------------------------------------------------------------

    #[test]
    fn test_take_capped_within_cap() {
        let chunk = b"{\"ok\":true}\n";
        let (take, done) = take_capped(0, chunk, 1024).unwrap();
        assert_eq!(take, chunk.len());
        assert!(done);

        let (take, done) = take_capped(0, b"no-newline", 1024).unwrap();
        assert_eq!(take, 10);
        assert!(!done);
    }

    #[test]
    fn test_take_capped_stops_at_newline() {
        // Bytes after the newline belong to the next line and must not be
        // consumed with this one.
        let (take, done) = take_capped(3, b"def\nnext-line", 1024).unwrap();
        assert_eq!(take, 4);
        assert!(done);
    }

    #[test]
    fn test_take_capped_enforces_cap() {
        // Exactly filling the cap is allowed...
        assert_eq!(take_capped(0, &[b'a'; 16], 16).unwrap(), (16, false));
        // ...one byte more is a protocol error...
        let err = take_capped(0, &[b'a'; 17], 16).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        // ...also when the cap is reached across chunks...
        let err = take_capped(16, b"b", 16).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        // ...and the newline terminator counts toward the cap.
        let err = take_capped(16, b"\n", 16).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn test_read_line_capped_reads_consecutive_lines() {
        let data: &[u8] = b"first\nsecond\n";
        let mut reader = tokio::io::BufReader::new(data);
        let mut line = Vec::new();

        let n = read_line_capped(&mut reader, &mut line, 64).await.unwrap();
        assert_eq!(&line[..n], b"first\n");
        let n = read_line_capped(&mut reader, &mut line, 64).await.unwrap();
        assert_eq!(&line[..n], b"second\n");
        let n = read_line_capped(&mut reader, &mut line, 64).await.unwrap();
        assert_eq!(n, 0, "EOF must read as 0");
    }

    #[tokio::test]
    async fn test_read_line_capped_rejects_overlong_line() {
        let data: Vec<u8> = vec![b'x'; 64]; // no newline anywhere
        let mut reader = tokio::io::BufReader::new(&data[..]);
        let mut line = Vec::new();

        let err = read_line_capped(&mut reader, &mut line, 16)
            .await
            .expect_err("over-long line must be rejected");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(line.len() <= 16, "buffer must never grow past the cap");
    }

    #[tokio::test]
    async fn test_read_line_capped_eof_semantics() {
        // Empty input: immediate EOF.
        let mut reader = tokio::io::BufReader::new(&b""[..]);
        let mut line = Vec::new();
        assert_eq!(read_line_capped(&mut reader, &mut line, 64).await.unwrap(), 0);

        // Partial line at EOF is returned as-is (like read_line), and the
        // next read reports EOF.
        let mut reader = tokio::io::BufReader::new(&b"partial"[..]);
        let n = read_line_capped(&mut reader, &mut line, 64).await.unwrap();
        assert_eq!(&line[..n], b"partial");
        assert_eq!(read_line_capped(&mut reader, &mut line, 64).await.unwrap(), 0);
    }

    // ------------------------------------------------------------------
    // Startup timeout: kill, do not orphan
    // ------------------------------------------------------------------

    /// A worker that never creates its socket must be killed and reaped when
    /// the startup timeout elapses (uses a real long-running `sleep`).
    #[tokio::test]
    #[cfg(unix)]
    async fn test_wait_for_socket_timeout_kills_child() {
        let dir = std::path::PathBuf::from(format!("/tmp/prpm-to{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let socket_path = dir.join("never.sock");

        let mut cmd = tokio::process::Command::new("/bin/sleep");
        cmd.arg("30").kill_on_drop(true);
        let mut child = cmd.spawn().expect("failed to spawn /bin/sleep");
        let pid = child.id().expect("spawned child must have a pid");

        let started = Instant::now();
        let result = wait_for_socket(&socket_path, &mut child, Duration::from_millis(300)).await;
        assert!(
            result.is_err(),
            "must time out when the worker never creates a socket"
        );
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "timeout must fire promptly, not hang"
        );

        // The child must be killed and reaped: try_wait observes the exit...
        let status = child
            .try_wait()
            .expect("try_wait must not fail")
            .expect("child must have been reaped after timeout kill");
        use std::os::unix::process::ExitStatusExt as _;
        assert_eq!(status.signal(), Some(9), "timed-out worker must be SIGKILLed");
        // ...and the OS agrees the pid is gone.
        assert!(!pid_alive(pid), "worker pid {pid} must be dead");

        let _ = std::fs::remove_dir_all(dir);
    }

    /// A worker that exits before creating its socket must fail the spawn
    /// fast instead of waiting out the full timeout.
    #[tokio::test]
    #[cfg(unix)]
    async fn test_wait_for_socket_errors_fast_when_child_exits() {
        let dir = std::path::PathBuf::from(format!("/tmp/prpm-ex{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let socket_path = dir.join("never.sock");

        let mut cmd = tokio::process::Command::new("/usr/bin/false");
        cmd.kill_on_drop(true);
        let mut child = cmd.spawn().expect("failed to spawn /usr/bin/false");

        let started = Instant::now();
        let result = wait_for_socket(&socket_path, &mut child, Duration::from_secs(30)).await;
        assert!(result.is_err());
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "must fail fast when the child exits, got {:?}",
            started.elapsed()
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    /// End-to-end: `spawn_worker` with a worker binary that runs forever but
    /// never opens its socket must error out, kill the worker, and leave no
    /// registered worker behind. `/usr/bin/yes` ignores its arguments and
    /// runs forever (it self-throttles once the stdout pipe fills), making
    /// it a stand-in for a hung worker.
    #[tokio::test]
    #[cfg(unix)]
    async fn test_spawn_worker_times_out_kills_child_and_stays_clean() {
        let dir = std::path::PathBuf::from(format!("/tmp/prpm-sp{}", std::process::id()));
        let mut pm = ProcessManager::with_binary_path(dir.clone(), PathBuf::from("/usr/bin/yes"));
        pm.startup_timeout = Duration::from_millis(300);

        let agent_id = pr_core::AgentId::new();
        let result = pm
            .spawn_worker(
                agent_id.clone(),
                "sess-timeout",
                "task".to_string(),
                pr_core::AgentRole::Researcher,
            )
            .await;

        assert!(
            result.is_err(),
            "spawn must fail when the worker never opens its socket"
        );
        assert_eq!(pm.active_count(), 0, "no worker may be registered on failure");

        let _ = std::fs::remove_dir_all(dir);
    }

    // ------------------------------------------------------------------
    // shutdown_all: kill and reap everything still alive
    // ------------------------------------------------------------------

    /// A worker that ignores `Cancel` must still be killed, reaped, and have
    /// its socket removed by `shutdown_all`.
    #[tokio::test]
    #[cfg(unix)]
    async fn test_shutdown_all_kills_and_reaps_stubborn_workers() {
        let dir = std::path::PathBuf::from(format!("/tmp/prpm-sh{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let mut pm = ProcessManager::with_binary_path(dir.clone(), PathBuf::from("/bin/sleep"));

        // Register a fake worker: `sleep 30` will ignore the Cancel message.
        let mut cmd = tokio::process::Command::new("/bin/sleep");
        cmd.arg("30").kill_on_drop(true);
        let process = cmd.spawn().expect("failed to spawn /bin/sleep");
        let pid = process.id().expect("spawned child must have a pid");
        let (coord_end, _worker_end) = tokio::net::UnixStream::pair().unwrap();
        let socket_path = dir.join(format!("fake-{pid}.sock"));
        std::fs::write(&socket_path, b"").unwrap();
        let agent_id = pr_core::AgentId::new();
        pm.workers.insert(
            agent_id.clone(),
            WorkerHandle {
                agent_id: agent_id.clone(),
                process,
                socket: coord_end,
                socket_path: socket_path.clone(),
            },
        );
        assert!(pm.is_running(&agent_id));

        pm.shutdown_all().await;

        assert_eq!(pm.active_count(), 0);
        assert!(!socket_path.exists(), "socket file must be removed");
        assert!(
            !pid_alive(pid),
            "worker pid {pid} must be killed and reaped by shutdown_all"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    /// Spawn the real `parallel-research` binary as a worker, verify the
    /// socket handshake and bookkeeping, then shut it down. This does not
    /// wait for the agent to finish (that would require a live LLM), so it
    /// stays fast and hermetic.
    ///
    /// Ignored by default because it spawns a real subprocess; run with
    /// `cargo test -p pr-agent -- --ignored` to exercise it. Requires the
    /// `parallel-research` binary to be built (`cargo build`).
    #[tokio::test]
    #[ignore = "spawns the real binary as a subprocess"]
    async fn test_spawn_and_shutdown_real_worker() {
        // Locate the CLI binary relative to this crate's manifest dir.
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest.join("..").join("..");
        let candidates = [
            workspace_root.join("target/debug/parallel-research"),
            workspace_root.join("target/release/parallel-research"),
        ];
        let binary_path = candidates
            .iter()
            .find(|p| p.exists())
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "parallel-research binary not found; run `cargo build` first (looked in {:?})",
                    candidates
                )
            });

        // Use a SHORT socket dir: Unix domain socket paths are limited to
        // ~104 bytes (SUN_LEN) and std::env::temp_dir() is long on macOS.
        let dir = std::path::PathBuf::from(format!("/tmp/prpm{}", std::process::id()));
        let mut pm = ProcessManager::with_binary_path(dir.clone(), binary_path);

        let agent_id = pr_core::AgentId::new();
        pm.spawn_worker(
            agent_id.clone(),
            "sess-integration-test",
            "integration test task".to_string(),
            pr_core::AgentRole::Researcher,
        )
        .await
        .expect("worker should spawn and connect its socket");

        assert_eq!(pm.active_count(), 1);
        assert!(pm.is_running(&agent_id));

        pm.shutdown_all().await;
        assert_eq!(pm.active_count(), 0);

        let _ = std::fs::remove_dir_all(dir);
    }
}
