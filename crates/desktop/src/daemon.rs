//! Daemon supervisor — locates, spawns, and manages `fathom serve` background process.

use anyhow::{bail, Result};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

pub struct DaemonManager {
    child: Mutex<Option<Child>>,
    port: u16,
    binary: Option<PathBuf>,
}

impl DaemonManager {
    pub fn new(port: u16) -> Self {
        let binary = Self::locate_binary();
        Self {
            child: Mutex::new(None),
            port,
            binary,
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn locate_binary() -> Option<PathBuf> {
        // 1. Check `fathom` on PATH
        if let Ok(path) = which::which("fathom") {
            return Some(path);
        }
        // 2. Look next to current executable
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                let candidate = parent.join("fathom");
                if candidate.is_file() {
                    return Some(candidate);
                }
                // Check target/debug or target/release
                let candidate2 = parent.join("../../target/debug/fathom");
                if candidate2.is_file() {
                    return Some(candidate2);
                }
            }
        }
        None
    }

    pub fn start(&self) -> Result<()> {
        let mut lock = self.child.lock();
        if lock.is_some() {
            return Ok(());
        }

        let binary = self.binary.as_ref().cloned().or_else(Self::locate_binary);
        let Some(bin) = binary else {
            bail!("fathom engine binary not found");
        };

        let child = Command::new(&bin)
            .arg("serve")
            .arg("--port")
            .arg(self.port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        *lock = Some(child);
        Ok(())
    }

    pub fn stop(&self) {
        let mut lock = self.child.lock();
        if let Some(mut child) = lock.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    pub fn is_running(&self) -> bool {
        let mut lock = self.child.lock();
        if let Some(child) = &mut *lock {
            match child.try_wait() {
                Ok(None) => true,
                _ => {
                    *lock = None;
                    false
                }
            }
        } else {
            false
        }
    }
}

impl Drop for DaemonManager {
    fn drop(&mut self) {
        self.stop();
    }
}
