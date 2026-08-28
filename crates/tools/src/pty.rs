use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::Mutex;
use pr_core::{PrError, PrResult};
use tokio::sync::broadcast;

/// Output line chunk in the circular buffer with monotonic sequence ID.
#[derive(Debug, Clone)]
pub struct PtyOutputChunk {
    pub seq: usize,
    pub timestamp: Instant,
    pub text: String,
}

/// Managed interactive PTY session.
pub struct PtySession {
    pub id: String,
    pub name: String,
    pub app: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub started_at: Instant,
    pub pid: u32,
    pub ring_buffer: Arc<Mutex<Vec<PtyOutputChunk>>>,
    pub seq_counter: Arc<Mutex<usize>>,
    pub stdin_tx: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    pub exit_rx: Arc<Mutex<Option<tokio::sync::oneshot::Receiver<i32>>>>,
    pub exit_code: Arc<Mutex<Option<i32>>>,
    pub notifier: broadcast::Sender<PtyOutputChunk>,
}

impl PtySession {
    /// Send plaintext stdin to the PTY process (with optional newline).
    pub fn write_stdin(&self, text: &str, enter: bool) -> PrResult<()> {
        let mut guard = self.stdin_tx.lock();
        if let Some(writer) = guard.as_mut() {
            let mut payload = text.to_string();
            if enter {
                payload.push('\n');
            }
            writer.write_all(payload.as_bytes()).map_err(|e| {
                PrError::Tool(format!("Failed to write stdin to PTY '{}': {}", self.name, e))
            })?;
            writer.flush().map_err(|e| {
                PrError::Tool(format!("Failed to flush stdin to PTY '{}': {}", self.name, e))
            })?;
            Ok(())
        } else {
            Err(PrError::Tool(format!("PTY stdin channel closed for '{}'", self.name)))
        }
    }

    /// Send special terminal control keystrokes (e.g. CTRL_C, ESCAPE, ENTER).
    pub fn send_key(&self, key: &str) -> PrResult<()> {
        let bytes: &[u8] = match key.to_uppercase().as_str() {
            "ENTER" => b"\n",
            "TAB" => b"\t",
            "ESCAPE" | "ESC" => b"\x1b",
            "CTRL_C" => b"\x03",
            "CTRL_D" => b"\x04",
            "CTRL_Z" => b"\x1a",
            "UP" => b"\x1b[A",
            "DOWN" => b"\x1b[B",
            "RIGHT" => b"\x1b[C",
            "LEFT" => b"\x1b[D",
            other => {
                return Err(PrError::Tool(format!("Unsupported key identifier: '{}'", other)));
            }
        };

        let mut guard = self.stdin_tx.lock();
        if let Some(writer) = guard.as_mut() {
            writer.write_all(bytes).map_err(|e| {
                PrError::Tool(format!("Failed to send key '{}' to PTY '{}': {}", key, self.name, e))
            })?;
            writer.flush().map_err(|e| {
                PrError::Tool(format!("Failed to flush key '{}' to PTY '{}': {}", key, self.name, e))
            })?;
            Ok(())
        } else {
            Err(PrError::Tool(format!("PTY stdin channel closed for '{}'", self.name)))
        }
    }

    /// Read lines from ring buffer starting at cursor offset, up to limit lines.
    pub fn read_logs(&self, cursor: Option<usize>, limit: usize) -> (Vec<PtyOutputChunk>, usize) {
        let buffer = self.ring_buffer.lock();
        let from_seq = cursor.unwrap_or(0);

        let matching: Vec<PtyOutputChunk> = buffer
            .iter()
            .filter(|c| c.seq > from_seq)
            .take(limit)
            .cloned()
            .collect();

        let latest_seq = buffer.last().map(|c| c.seq).unwrap_or(from_seq);
        (matching, latest_seq)
    }

    /// Wait for a log regex pattern or timeout.
    pub async fn wait_for_pattern(&self, regex_pattern: &str, timeout_secs: u64) -> PrResult<bool> {
        let re = regex::Regex::new(regex_pattern).map_err(|e| {
            PrError::Tool(format!("Invalid readiness regex '{}': {}", regex_pattern, e))
        })?;

        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        let mut cursor = 0;

        while Instant::now() < deadline {
            let (chunks, latest) = self.read_logs(Some(cursor), 100);
            cursor = latest;
            for chunk in chunks {
                if re.is_match(&chunk.text) {
                    return Ok(true);
                }
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }

        Ok(false)
    }
}

/// Process-global registry of interactive PTY sessions.
#[derive(Clone)]
pub struct PtyBroker {
    sessions: Arc<Mutex<HashMap<String, Arc<PtySession>>>>,
}

impl PtyBroker {
    pub fn global() -> &'static PtyBroker {
        static INSTANCE: std::sync::LazyLock<PtyBroker> = std::sync::LazyLock::new(|| PtyBroker {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        });
        &INSTANCE
    }

    /// Spawn and register a new interactive child process attached to a pipe/pseudoterminal stream.
    pub fn spawn_process(
        &self,
        name: &str,
        app: &str,
        args: &[String],
        cwd: &Path,
    ) -> PrResult<Arc<PtySession>> {
        let mut cmd = std::process::Command::new(app);
        cmd.args(args);
        cmd.current_dir(cwd);
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            PrError::Tool(format!("Failed to spawn process '{}' ({}): {}", name, app, e))
        })?;

        let pid = child.id();
        let stdin = child.stdin.take().expect("stdin piped");
        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");

        let ring_buffer = Arc::new(Mutex::new(Vec::with_capacity(10000)));
        let seq_counter = Arc::new(Mutex::new(0));
        let (notifier, _) = broadcast::channel(512);

        let (exit_tx, exit_rx) = tokio::sync::oneshot::channel();
        let exit_code = Arc::new(Mutex::new(None));

        let session = Arc::new(PtySession {
            id: uuid::Uuid::now_v7().to_string(),
            name: name.to_string(),
            app: app.to_string(),
            args: args.to_vec(),
            cwd: cwd.to_path_buf(),
            started_at: Instant::now(),
            pid,
            ring_buffer: ring_buffer.clone(),
            seq_counter: seq_counter.clone(),
            stdin_tx: Arc::new(Mutex::new(Some(Box::new(stdin)))),
            exit_rx: Arc::new(Mutex::new(Some(exit_rx))),
            exit_code: exit_code.clone(),
            notifier: notifier.clone(),
        });

        // Spawn stdout reader thread
        let rb_out = ring_buffer.clone();
        let seq_out = seq_counter.clone();
        let notif_out = notifier.clone();
        std::thread::spawn(move || {
            let mut reader = std::io::BufReader::new(stdout);
            let mut line = String::new();
            use std::io::BufRead;
            while let Ok(n) = reader.read_line(&mut line) {
                if n == 0 { break; }
                let mut s = seq_out.lock();
                *s += 1;
                let chunk = PtyOutputChunk {
                    seq: *s,
                    timestamp: Instant::now(),
                    text: line.trim_end().to_string(),
                };
                let _ = notif_out.send(chunk.clone());
                let mut buf = rb_out.lock();
                if buf.len() >= 10000 {
                    buf.remove(0);
                }
                buf.push(chunk);
                line.clear();
            }
        });

        // Spawn stderr reader thread
        let rb_err = ring_buffer.clone();
        let seq_err = seq_counter.clone();
        let notif_err = notifier.clone();
        std::thread::spawn(move || {
            let mut reader = std::io::BufReader::new(stderr);
            let mut line = String::new();
            use std::io::BufRead;
            while let Ok(n) = reader.read_line(&mut line) {
                if n == 0 { break; }
                let mut s = seq_err.lock();
                *s += 1;
                let chunk = PtyOutputChunk {
                    seq: *s,
                    timestamp: Instant::now(),
                    text: line.trim_end().to_string(),
                };
                let _ = notif_err.send(chunk.clone());
                let mut buf = rb_err.lock();
                if buf.len() >= 10000 {
                    buf.remove(0);
                }
                buf.push(chunk);
                line.clear();
            }
        });

        // Spawn exit waiter thread
        let exit_code_clone = exit_code.clone();
        std::thread::spawn(move || {
            let code = match child.wait() {
                Ok(status) => status.code().unwrap_or(0),
                Err(_) => -1,
            };
            *exit_code_clone.lock() = Some(code);
            let _ = exit_tx.send(code);
        });

        let mut sessions = self.sessions.lock();
        sessions.insert(name.to_string(), session.clone());
        Ok(session)
    }

    /// Get an active PTY session by name.
    pub fn get(&self, name: &str) -> Option<Arc<PtySession>> {
        self.sessions.lock().get(name).cloned()
    }

    /// List all registered PTY session names.
    pub fn list_sessions(&self) -> Vec<String> {
        self.sessions.lock().keys().cloned().collect()
    }

    /// Terminate and unregister a session.
    pub fn stop(&self, name: &str) -> PrResult<()> {
        let session = self.sessions.lock().remove(name);
        if let Some(s) = session {
            #[cfg(unix)]
            {
                let _ = std::process::Command::new("kill")
                    .arg("-15")
                    .arg(s.pid.to_string())
                    .status();
            }
            Ok(())
        } else {
            Err(PrError::Tool(format!("No PTY session named '{}' found", name)))
        }
    }
}
