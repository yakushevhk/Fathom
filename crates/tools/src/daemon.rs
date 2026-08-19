//! `daemon` tool: manage long-running processes (dev servers, watchers,
//! REPLs). Agents can start, stop, restart, check logs, and wait for
//! readiness conditions (port binding, log regex).

use async_trait::async_trait;
use pr_core::daemon::{DaemonInfo, DaemonRegistry, DaemonStatus};
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

use crate::registry::{Tool, ToolContext};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op")]
enum DaemonOp {
    /// Start a new daemon process.
    #[serde(rename = "start")]
    Start {
        /// Unique name for the daemon (used for subsequent operations).
        name: String,
        /// Shell command to run.
        shell: String,
        /// Working directory (optional, defaults to agent's working dir).
        #[serde(default)]
        cwd: Option<String>,
        /// Expected port the daemon will listen on. When set, the tool
        /// blocks until the port is reachable.
        #[serde(default)]
        port: Option<u16>,
        /// Regex pattern to wait for in stdout/stderr before returning.
        #[serde(default)]
        ready_pattern: Option<String>,
        /// Timeout in seconds for readiness checks (default 30).
        #[serde(default = "default_timeout")]
        timeout_secs: u64,
    },
    /// Stop a running daemon.
    #[serde(rename = "stop")]
    Stop {
        /// Name of the daemon to stop.
        name: String,
    },
    /// Restart a daemon.
    #[serde(rename = "restart")]
    Restart {
        /// Name of the daemon to restart.
        name: String,
    },
    /// Check daemon status.
    #[serde(rename = "status")]
    Status {
        /// Name of the daemon (or omit to list all).
        #[serde(default)]
        name: Option<String>,
    },
    /// List all daemons owned by this agent.
    #[serde(rename = "list")]
    List,
}

fn default_timeout() -> u64 {
    30
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct DaemonParams {
    #[serde(flatten)]
    command: DaemonOp,
}

/// Manage long-running processes (dev servers, watchers, REPLs).
pub struct DaemonTool;

#[async_trait]
impl Tool for DaemonTool {
    fn name(&self) -> &str {
        "daemon"
    }

    fn description(&self) -> &str {
        "Manage long-running background processes. Use `daemon` to start \
         (with optional port/regex readiness checks), stop, restart, check \
         status, or list daemons. Each daemon is identified by a unique name."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(DaemonParams)).unwrap(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: DaemonParams = serde_json::from_value(args)?;
        let agent_id = match &ctx.agent_id {
            Some(id) => id.clone(),
            None => {
                return Ok(ToolOutput::err(
                    "daemon tool requires agent_id in ToolContext",
                ))
            }
        };

        let reg = DaemonRegistry::global();

        match params.command {
            DaemonOp::Start {
                name,
                shell,
                cwd,
                port,
                ready_pattern,
                timeout_secs,
            } => {
                if reg.get(&name).is_some() {
                    return Ok(ToolOutput::err(format!(
                        "Daemon '{name}' already exists. Stop it first or use a different name."
                    )));
                }

                reg.register(DaemonInfo {
                    name: name.clone(),
                    shell: shell.clone(),
                    status: DaemonStatus::Starting,
                    pid: None,
                    port: None,
                    created_by: agent_id,
                    started_at: SystemTime::now(),
                    last_heartbeat: SystemTime::now(),
                });

                let working_dir = cwd
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| ctx.working_dir.clone());

                let mut child = match tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(&shell)
                    .current_dir(&working_dir)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .kill_on_drop(true)
                    .spawn()
                {
                    Ok(c) => c,
                    Err(e) => {
                        reg.update_status(&name, DaemonStatus::Failed);
                        return Ok(ToolOutput::err(format!("Failed to spawn daemon: {e}")));
                    }
                };

                let pid = child.id().ok_or(0).unwrap_or(0);
                reg.update_pid(&name, pid);

                if port.is_some() || ready_pattern.is_some() {
                    let timeout = std::time::Duration::from_secs(timeout_secs);
                    let deadline = std::time::Instant::now() + timeout;
                    let mut ready = false;

                    if let Some(check_port) = port {
                        let addr = format!("127.0.0.1:{check_port}");
                        loop {
                            if std::time::Instant::now() > deadline {
                                break;
                            }
                            match tokio::net::TcpStream::connect(&addr).await {
                                Ok(_) => {
                                    reg.update_port(&name, check_port);
                                    ready = true;
                                    break;
                                }
                                Err(_) => {
                                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                                }
                            }
                        }
                        if !ready {
                            let _ = child.start_kill();
                            let _ = child.wait().await;
                            reg.update_status(&name, DaemonStatus::Failed);
                            return Ok(ToolOutput::err(format!(
                                "Daemon '{name}' port {check_port} not reachable within {timeout_secs}s"
                            )));
                        }
                    }

                    if let Some(pattern) = ready_pattern {
                        let regex = regex::Regex::new(&pattern)
                            .map_err(|e| anyhow::anyhow!("Invalid ready_pattern: {e}"))?;
                        let Some(stdout) = child.stdout.take() else {
                            let _ = child.start_kill();
                            let _ = child.wait().await;
                            reg.update_status(&name, DaemonStatus::Failed);
                            return Ok(ToolOutput::err("Daemon stdout unavailable for ready_pattern"));
                        };
                        use tokio::io::AsyncBufReadExt;
                        let mut lines = tokio::io::BufReader::new(stdout).lines();
                        let mut matched = false;
                        while std::time::Instant::now() < deadline {
                            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                            match tokio::time::timeout(remaining, lines.next_line()).await {
                                Ok(Ok(Some(line))) if regex.is_match(&line) => {
                                    matched = true;
                                    break;
                                }
                                Ok(Ok(Some(_))) => {}
                                Ok(Ok(None)) | Ok(Err(_)) | Err(_) => break,
                            }
                        }
                        if !matched {
                            let _ = child.start_kill();
                            let _ = child.wait().await;
                            reg.update_status(&name, DaemonStatus::Failed);
                            return Ok(ToolOutput::err(format!(
                                "Daemon '{name}' ready_pattern not matched within {timeout_secs}s"
                            )));
                        }
                        ready = true;
                    }

                    if !ready {
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        reg.update_status(&name, DaemonStatus::Running);
                    } else {
                        reg.update_status(&name, DaemonStatus::Running);
                    }
                } else {
                    reg.update_status(&name, DaemonStatus::Running);
                }

                let n = name.clone();
                tokio::spawn(async move {
                    let _ = child.wait().await;
                    reg.update_status(&n, DaemonStatus::Stopped);
                });

                Ok(ToolOutput::ok(format!(
                    "Daemon '{name}' started (pid={pid}){}",
                    port.map(|p| format!(", port={p}")).unwrap_or_default()
                )))
            }

            DaemonOp::Stop { name } => {
                let info = reg.get(&name);
                if let Some(info) = info {
                    if let Some(pid) = info.pid {
                        #[cfg(unix)]
                        let _ = std::process::Command::new("kill")
                            .arg(pid.to_string())
                            .spawn();
                        #[cfg(not(unix))]
                        let _ = std::process::Command::new("taskkill")
                            .args(["/PID", &pid.to_string(), "/F"])
                            .spawn();
                        reg.update_status(&name, DaemonStatus::Stopped);
                        Ok(ToolOutput::ok(format!(
                            "Daemon '{name}' (pid={pid}) stopped."
                        )))
                    } else {
                        reg.update_status(&name, DaemonStatus::Stopped);
                        Ok(ToolOutput::ok(format!("Daemon '{name}' cleaned up (no pid).")))
                    }
                } else {
                    Ok(ToolOutput::err(format!("Daemon '{name}' not found.")))
                }
            }

            DaemonOp::Restart { name } => {
                let info = reg.get(&name);
                let Some(info) = info else {
                    return Ok(ToolOutput::err(format!("Daemon '{name}' not found.")));
                };

                if let Some(pid) = info.pid {
                    #[cfg(unix)]
                    let _ = std::process::Command::new("kill")
                        .arg(pid.to_string())
                        .spawn();
                    #[cfg(not(unix))]
                    let _ = std::process::Command::new("taskkill")
                        .args(["/PID", &pid.to_string(), "/F"])
                        .spawn();
                }
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                reg.unregister(&name);

                let shell = info.shell.clone();
                let mut child = match tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(&shell)
                    .current_dir(&ctx.working_dir)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .kill_on_drop(true)
                    .spawn()
                {
                    Ok(c) => c,
                    Err(e) => {
                        return Ok(ToolOutput::err(format!("Failed to restart: {e}")));
                    }
                };

                let pid = child.id().ok_or(0).unwrap_or(0);
                reg.register(DaemonInfo {
                    name: name.clone(),
                    shell: shell.clone(),
                    status: DaemonStatus::Running,
                    pid: Some(pid),
                    port: info.port,
                    created_by: info.created_by,
                    started_at: SystemTime::now(),
                    last_heartbeat: SystemTime::now(),
                });

                let n = name.clone();
                tokio::spawn(async move {
                    let _ = child.wait().await;
                    reg.update_status(&n, DaemonStatus::Stopped);
                });

                Ok(ToolOutput::ok(format!(
                    "Daemon '{name}' restarted (pid={pid})."
                )))
            }

            DaemonOp::Status { name } => {
                if let Some(n) = name {
                    match reg.get(&n) {
                        Some(info) => Ok(ToolOutput::ok(format!(
                            "Daemon '{n}': status={:?}, pid={:?}, port={:?}, command={}",
                            info.status, info.pid, info.port, info.shell
                        ))),
                        None => Ok(ToolOutput::err(format!("Daemon '{n}' not found."))),
                    }
                } else {
                    let daemons = reg.list();
                    if daemons.is_empty() {
                        return Ok(ToolOutput::ok("No daemons running."));
                    }
                    let lines: Vec<String> = daemons
                        .iter()
                        .map(|d| {
                            format!(
                                "- {}: {:?} pid={:?} port={:?}",
                                d.name, d.status, d.pid, d.port
                            )
                        })
                        .collect();
                    Ok(ToolOutput::ok(format!(
                        "{} daemon(s):\n{}",
                        daemons.len(),
                        lines.join("\n")
                    )))
                }
            }

            DaemonOp::List => {
                let daemons = reg.list_by_owner(&agent_id);
                if daemons.is_empty() {
                    return Ok(ToolOutput::ok("No daemons owned by this agent."));
                }
                let lines: Vec<String> = daemons
                    .iter()
                    .map(|d| format!("- {}: {:?} pid={:?}", d.name, d.status, d.pid))
                    .collect();
                Ok(ToolOutput::ok(format!(
                    "{} daemon(s):\n{}",
                    daemons.len(),
                    lines.join("\n")
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ToolContext;
    use pr_core::{SearchConfig, AgentId};
    use std::path::PathBuf;

    #[test]
    fn test_daemon_schema() {
        let tool = DaemonTool;
        let schema = tool.schema();
        assert_eq!(schema.name, "daemon");
    }

    #[tokio::test]
    async fn test_daemon_list_empty() {
        let tool = DaemonTool;
        let mut ctx = ToolContext::new(PathBuf::from("/tmp"), SearchConfig::default());
        ctx.agent_id = Some(AgentId::new());
        let out = tool
            .execute(serde_json::json!({"op": "list"}), &ctx)
            .await
            .unwrap();
        assert!(out.success);
        assert!(out.content.contains("No daemons"));
    }
}