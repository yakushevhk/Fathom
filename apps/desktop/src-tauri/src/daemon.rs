//! DaemonManager — locates, spawns, health-checks, and terminates the
//! `fathom serve` process. Runs as a long-lived background child process.

use crate::types::{DaemonStatus, StartOptions};
use portpicker::pick_unused_port;
use reqwest::Client;
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::process::Command as AsyncCommand;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::info;

/// How long we wait for the engine to respond to /health.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(8);
/// Poll interval while waiting for the engine to start.
const HEALTH_POLL: Duration = Duration::from_millis(200);

pub struct DaemonManager {
    handle: AppHandle,
    child: Mutex<Option<Child>>,
    port: Mutex<Option<u16>>,
    url: Mutex<Option<String>>,
    http: Client,
}

impl DaemonManager {
    pub fn new(handle: AppHandle) -> Self {
        Self {
            handle,
            child: Mutex::new(None),
            port: Mutex::new(None),
            url: Mutex::new(None),
            http: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("reqwest client"),
        }
    }

    /// Locate the freight binary on PATH or next to our own executable.
    pub fn locate_binary() -> Option<PathBuf> {
        // 1. Check `fathom` on PATH
        if let Ok(path) = which::which("fathom") {
            return Some(path);
        }
        // 2. Next to the current executable (dev builds, bundled)
        if let Ok(exe) = std::env::current_exe() {
            let sibling = exe.parent()?.join("fathom");
            if sibling.is_file() {
                return Some(sibling);
            }
            // 3. One level up (cargo run --bin desktop, binary is in target/debug/)
            let parent = exe.parent()?.parent()?.join("fathom");
            if parent.is_file() {
                return Some(parent);
            }
        }
        None
    }

    /// Start the fathom engine daemon. Idempotent: if already running,
    /// returns the current status.
    pub async fn start(&self, opts: StartOptions) -> Result<DaemonStatus, String> {
        // Already running?
        if let Ok(status) = self.health().await {
            if status.running && !opts.force {
                return Ok(status);
            }
        }

        let binary = Self::locate_binary()
            .ok_or_else(|| "fathom binary not found on PATH or next to the app".to_string())?;

        let port = opts.port.unwrap_or_else(|| {
            pick_unused_port().expect("no free port available")
        });
        let url = format!("http://127.0.0.1:{port}");

        info!("starting fathom serve on {url} from {:?}", binary);

        let child = std::process::Command::new(&binary)
            .arg("serve")
            .arg("--port")
            .arg(port.to_string())
            .arg("--host")
            .arg("127.0.0.1")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to spawn fathom: {e}"))?;

        *self.child.lock().await = Some(child);
        *self.port.lock().await = Some(port);
        *self.url.lock().await = Some(url.clone());

        // Wait for /health
        let deadline = sleep(HEALTH_TIMEOUT);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                _ = &mut deadline => {
                    let _ = self.kill().await;
                    return Err("engine failed to start within timeout".into());
                }
                _ = sleep(HEALTH_POLL) => {}
            }
            if let Ok(resp) = self.http.get(format!("{url}/health")).send().await {
                if resp.status().is_success() {
                    let status = self.build_status(url.clone(), port, "running").await;
                    info!("fathom engine is healthy on {url}");
                    // Mark the engine port as free for the next start
                    // (portpicker marks it, but we already spawned).
                    self.emit_event("daemon:started", &status);
                    return Ok(status);
                }
            }
        }
    }

    /// Graceful shutdown: kill the child process.
    pub async fn kill(&self) {
        if let Some(mut child) = self.child.lock().await.take() {
            info!("shutting down fathom engine");
            let _ = child.kill();
            let _ = child.wait();
        }
        *self.port.lock().await = None;
        *self.url.lock().await = None;
        let status = DaemonStatus {
            phase: "stopped".into(),
            ..Default::default()
        };
        self.emit_event("daemon:status", &status);
    }

    /// Quick health check against the running engine.
    pub async fn health(&self) -> Result<DaemonStatus, String> {
        let url = self.url.lock().await.clone();
        let port = *self.port.lock().await;
        let Some(url) = url else {
            return Ok(DaemonStatus {
                phase: "stopped".into(),
                ..Default::default()
            });
        };
        let port = port.unwrap_or(0);

        match self.http.get(format!("{url}/health")).send().await {
            Ok(resp) if resp.status().is_success() => {
                Ok(self.build_status(url, port, "running").await)
            }
            Ok(resp) => Ok(DaemonStatus {
                running: false,
                url: Some(url),
                port: Some(port),
                phase: "error".into(),
                error: Some(format!("health check returned {}", resp.status())),
                ..Default::default()
            }),
            Err(e) => {
                // Engine might have died
                self.child.lock().await.take();
                *self.port.lock().await = None;
                *self.url.lock().await = None;
                Ok(DaemonStatus {
                    phase: "stopped".into(),
                    error: Some(e.to_string()),
                    ..Default::default()
                })
            }
        }
    }

    /// Get the current base URL, or None if not running.
    pub async fn base_url(&self) -> Option<String> {
        self.url.lock().await.clone()
    }

    async fn build_status(&self, url: String, port: u16, phase: &str) -> DaemonStatus {
        let binary = Self::locate_binary();
        let version = if let Some(path) = binary.as_ref() {
            // The version probe is bounded and async so health/status calls never
            // block the Tauri runtime. Keep the output exactly as reported by the
            // managed engine (typically `fathom 0.3.0`).
            match tokio::time::timeout(
                Duration::from_secs(2),
                AsyncCommand::new(path).arg("--version").output(),
            )
            .await
            {
                Ok(Ok(output)) if output.status.success() => {
                    let value = if output.stdout.is_empty() {
                        String::from_utf8_lossy(&output.stderr).trim().to_owned()
                    } else {
                        String::from_utf8_lossy(&output.stdout).trim().to_owned()
                    };
                    (!value.is_empty()).then_some(value)
                }
                _ => None,
            }
        } else {
            None
        };

        DaemonStatus {
            running: true,
            url: Some(url),
            port: Some(port),
            binary: binary.map(|p| p.to_string_lossy().into()),
            version,
            phase: phase.into(),
            error: None,
        }
    }

    fn emit_event<T: serde::Serialize + Clone>(&self, event: &str, payload: &T) {
        let _ = self.handle.emit(event, payload);
    }
}