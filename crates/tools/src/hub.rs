//! `hub` tool: inter-agent messaging, peer discovery, and job management.
//!
//! Provides a unified interface for agents to:
//! - `send` — send a message to another agent or broadcast
//! - `wait` — block until a message arrives (from a specific peer or any)
//! - `inbox` — read pending messages without blocking
//! - `list` — see all live agents, their statuses, and activity
//! - `set_activity` — update the agent's own activity description

use async_trait::async_trait;
use pr_core::irc::{AgentRegistry, DeliveryReceipt, IrcBus, IrcMessage};
use pr_core::ids::AgentId;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::registry::{Tool, ToolContext};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "command")]
enum HubCommand {
    /// Send a message to a peer agent (or broadcast to all).
    #[serde(rename = "send")]
    Send {
        /// Target agent id. Omit or leave empty for broadcast.
        #[serde(default)]
        to: Option<String>,
        /// Message content.
        message: String,
        /// If true, block until the target replies.
        #[serde(default)]
        await_reply: bool,
        /// If true, deliver as a steering directive (mid-run instruction)
        /// instead of a regular inbox message.
        #[serde(default)]
        steer: bool,
    },
    /// Wait for an incoming message from a specific peer (or any).
    #[serde(rename = "wait")]
    Wait {
        /// Optional: only accept messages from this agent id.
        #[serde(default)]
        from: Option<String>,
        /// Timeout in seconds (default 60, 0 = no timeout).
        #[serde(default = "default_wait_timeout")]
        timeout_secs: u64,
    },
    /// Read pending messages without blocking.
    #[serde(rename = "inbox")]
    Inbox {
        /// If true, don't consume the messages.
        #[serde(default)]
        peek: bool,
    },
    /// List all live agents.
    #[serde(rename = "list")]
    List,
    /// List all async jobs owned by this agent.
    #[serde(rename = "jobs")]
    Jobs,
    /// Update this agent's activity description.
    #[serde(rename = "set_activity")]
    SetActivity {
        /// New activity description.
        activity: String,
    },
    /// Start an interactive persistent PTY process (dev server, REPL, watcher).
    #[serde(rename = "start")]
    Start {
        /// Unique process name
        name: String,
        /// Application or binary to execute
        application: String,
        /// Arguments list
        #[serde(default)]
        args: Vec<String>,
        /// Working directory (optional)
        #[serde(default)]
        cwd: Option<String>,
        /// Wait for log regex pattern before returning
        #[serde(default)]
        ready_log: Option<String>,
        /// Timeout for readiness check in seconds (default 30)
        #[serde(default = "default_wait_timeout")]
        timeout_secs: u64,
    },
    /// Read output logs from a PTY process.
    #[serde(rename = "logs")]
    Logs {
        /// Process name
        name: String,
        /// Starting cursor sequence number (optional)
        #[serde(default)]
        cursor: Option<usize>,
        /// Number of lines to return (default 100)
        #[serde(default)]
        limit: Option<usize>,
    },
    /// Send stdin text, keys, or signals to a PTY process.
    #[serde(rename = "pty_send")]
    PtySend {
        /// Process name
        name: String,
        /// Text to write to stdin
        #[serde(default)]
        text: Option<String>,
        /// Press enter after text (default true)
        #[serde(default = "default_true")]
        enter: bool,
        /// Special terminal keys to send (e.g. ["CTRL_C", "ENTER", "TAB", "ESCAPE"])
        #[serde(default)]
        keys: Vec<String>,
    },
    /// Stop/kill a PTY process.
    #[serde(rename = "stop")]
    Stop {
        /// Process name
        name: String,
    },
}

fn default_true() -> bool {
    true
}

fn default_wait_timeout() -> u64 {
    60
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct HubParams {
    #[serde(flatten)]
    command: HubCommand,
}

/// Unified peer-to-peer coordination tool.
pub struct HubTool;

#[async_trait]
impl Tool for HubTool {
    fn name(&self) -> &str {
        "hub"
    }

    fn description(&self) -> &str {
        "Inter-agent communication and coordination. Use `hub` to send \
         messages to other agents, wait for replies, read your inbox, \
         list all live agents, or update your activity description."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(HubParams)).unwrap(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: HubParams = serde_json::from_value(args)?;
        let agent_id = match &ctx.agent_id {
            Some(id) => id.clone(),
            None => {
                return Ok(ToolOutput::err(
                    "hub tool requires an agent_id to be set in ToolContext",
                ))
            }
        };

        match params.command {
            HubCommand::Send {
                to,
                message,
                await_reply,
                steer,
            } => {
                let bus = IrcBus::global();
                let msg_id = bus.next_msg_id();

                if let Some(target_id) = to {
                    // Check if this is a steering directive.
                    if steer {
                        let target = AgentId(target_id.clone());
                        if pr_core::SteerRegistry::global().steer(&target, message.clone()) {
                            return Ok(ToolOutput::ok(format!(
                                "Steering directive sent to {target_id}"
                            )));
                        } else {
                            return Ok(ToolOutput::ok(format!(
                                "Cannot steer {target_id}: agent not found or not registered for steering"
                            )));
                        }
                    }

                    // Direct message
                    let target = AgentId(target_id.clone());
                    let msg = IrcMessage {
                        from: agent_id.clone(),
                        to: Some(target.clone()),
                        content: message,
                        id: msg_id.clone(),
                        expects_reply: await_reply,
                        reply_to: None,
                    };

                    let receipt = if await_reply {
                        // Register before sending to avoid losing a fast reply.
                        // Correlate both sender and recipient to avoid stealing
                        // another agent's message.
                        let (tx, rx) = oneshot::channel();
                        let waiter_id = bus.register_waiter(Some(target.clone()), Some(agent_id.clone()), tx);
                        let _receipt = bus.send(msg);

                        let timeout = std::time::Duration::from_secs(120);
                        match tokio::time::timeout(timeout, rx).await {
                            Ok(Ok(reply)) => {
                                return Ok(ToolOutput::ok(format!(
                                    "Reply from {}: {}",
                                    reply.from, reply.content
                                )));
                            }
                            Ok(Err(_)) => {
                                bus.cancel_waiter(waiter_id);
                                // Side-channel auto-reply: when the target
                                // agent is parked or not listening, the
                                // IrcBus reviver may have spawned a new turn
                                // but the result may not arrive. Generate
                                // a canned auto-reply so the caller doesn't
                                // hang.
                                if let Some(llm) = &ctx.aux_llm() {
                                    let prompt = format!(
                                        "Agent {} is not available to reply. \
                                         Generate a brief, helpful auto-reply \
                                         acknowledging the message.",
                                        target_id
                                    );
                                    if let Ok(resp) = llm
                                        .complete(&pr_llm::CompletionRequest {
                                            messages: vec![
                                                pr_core::Message::system(
                                                    "You are generating an auto-reply \
                                                     for an unavailable agent.",
                                                ),
                                                pr_core::Message::user(&prompt),
                                            ],
                                            tools: vec![],
                                            temperature: Some(0.3),
                                            max_tokens: Some(100),
                                            stream: false,
                                        })
                                        .await
                                    {
                                        if let pr_core::Message::Assistant { content, .. } = &resp.message {
                                            if let Some(text) = content {
                                                return Ok(ToolOutput::ok(format!(
                                                    "[auto-reply from {}] {}",
                                                    target_id, text
                                                )));
                                            }
                                        }
                                    }
                                }
                                return Ok(ToolOutput::ok(
                                    "No reply received (agent unavailable).",
                                ));
                            }
                            Err(_) => {
                                bus.cancel_waiter(waiter_id);
                                return Ok(ToolOutput::ok(
                                    "Timed out waiting for reply.",
                                ));
                            }
                        }
                    } else {
                        bus.send(msg)
                    };

                    let status = match receipt {
                        DeliveryReceipt::Delivered => "delivered".to_string(),
                        DeliveryReceipt::WaiterDelivered => "delivered (waiter)".to_string(),
                        DeliveryReceipt::AgentNotFound => {
                            "agent not found (message queued)".to_string()
                        }
                        DeliveryReceipt::Broadcast(_) => "broadcast".to_string(),
                    };
                    Ok(ToolOutput::ok(format!(
                        "Message sent to {target_id}: {status}"
                    )))
                } else {
                    // Broadcast
                    let msg = IrcMessage {
                        from: agent_id,
                        to: None,
                        content: message,
                        id: msg_id,
                        expects_reply: false,
                        reply_to: None,
                    };
                    match bus.send(msg) {
                        DeliveryReceipt::Broadcast(n) => {
                            Ok(ToolOutput::ok(format!("Broadcast sent to {n} agent(s)")))
                        }
                        other => Ok(ToolOutput::ok(format!("Broadcast result: {other:?}"))),
                    }
                }
            }

            HubCommand::Wait { from, timeout_secs } => {
                let bus = IrcBus::global();
                let from_id = from.map(AgentId);

                let (tx, mut rx) = oneshot::channel();
                let waiter_id = bus.register_waiter(from_id, Some(agent_id.clone()), tx);

                let timeout = if timeout_secs > 0 {
                    std::time::Duration::from_secs(timeout_secs)
                } else {
                    std::time::Duration::from_secs(300) // default 5 min
                };

                match tokio::time::timeout(timeout, &mut rx).await {
                    Ok(Ok(msg)) => Ok(ToolOutput::ok(format!(
                        "Message from {}: {}",
                        msg.from, msg.content
                    ))),
                    Ok(Err(_)) => {
                        bus.cancel_waiter(waiter_id);
                        Ok(ToolOutput::ok("No message received."))
                    }
                    Err(_) => {
                        bus.cancel_waiter(waiter_id);
                        Ok(ToolOutput::ok("Timed out waiting for message."))
                    },
                }
            }

            HubCommand::Inbox { peek } => {
                let bus = IrcBus::global();
                let msgs = if peek {
                    bus.peek_mailbox(&agent_id)
                } else {
                    bus.drain_mailbox(&agent_id)
                };

                if msgs.is_empty() {
                    return Ok(ToolOutput::ok("Inbox is empty."));
                }

                let summary: Vec<String> = msgs
                    .iter()
                    .map(|m| format!("[from {}] {}", m.from, m.content))
                    .collect();
                Ok(ToolOutput::ok(format!(
                    "{} message(s):\n{}",
                    msgs.len(),
                    summary.join("\n")
                )))
            }

            HubCommand::List => {
                let registry = AgentRegistry::global();
                let agents = registry.list();

                if agents.is_empty() {
                    return Ok(ToolOutput::ok("No other agents registered."));
                }

                let lines: Vec<String> = agents
                    .iter()
                    .map(|a| {
                        format!(
                            "- {} (role={}, status={}, activity={})",
                            a.id, a.role, a.status, a.activity
                        )
                    })
                    .collect();
                Ok(ToolOutput::ok(format!(
                    "{} agent(s):\n{}",
                    agents.len(),
                    lines.join("\n")
                )))
            }

            HubCommand::Jobs => {
                let jobs = pr_core::async_job::AsyncJobManager::global()
                    .list_by_owner(&agent_id);
                if jobs.is_empty() {
                    return Ok(ToolOutput::ok("No async jobs."));
                }
                let lines: Vec<String> = jobs
                    .iter()
                    .map(|j| {
                        format!(
                            "- #{} label={} status={:?} tokens={}",
                            j.id, j.label, j.status, j.tokens
                        )
                    })
                    .collect();
                Ok(ToolOutput::ok(format!(
                    "{} job(s):\n{}",
                    jobs.len(),
                    lines.join("\n")
                )))
            }

            HubCommand::SetActivity { activity } => {
                let registry = AgentRegistry::global();
                registry.update_activity(&agent_id, activity);
                Ok(ToolOutput::ok("Activity updated."))
            }

            HubCommand::Start {
                name,
                application,
                args,
                cwd,
                ready_log,
                timeout_secs,
            } => {
                let broker = crate::pty::PtyBroker::global();
                let work_dir = cwd
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| ctx.working_dir.clone());

                let session = match broker.spawn_process(&name, &application, &args, &work_dir) {
                    Ok(s) => s,
                    Err(e) => return Ok(ToolOutput::err(format!("Failed to start PTY process '{}': {}", name, e))),
                };

                if let Some(pattern) = ready_log {
                    let matched = session.wait_for_pattern(&pattern, timeout_secs).await.unwrap_or(false);
                    if !matched {
                        return Ok(ToolOutput::ok(format!(
                            "Process '{}' started (PID {}), but timed out waiting for ready_log '{}'",
                            name, session.pid, pattern
                        )));
                    }
                }

                Ok(ToolOutput::ok(format!(
                    "Started PTY process '{}' (PID {}) in {}",
                    name, session.pid, work_dir.display()
                )))
            }

            HubCommand::Logs { name, cursor, limit } => {
                let broker = crate::pty::PtyBroker::global();
                let session = match broker.get(&name) {
                    Some(s) => s,
                    None => return Ok(ToolOutput::err(format!("No PTY session named '{}' found", name))),
                };

                let (chunks, latest_seq) = session.read_logs(cursor, limit.unwrap_or(100));
                let lines: Vec<String> = chunks.iter().map(|c| format!("[#{}] {}", c.seq, c.text)).collect();
                Ok(ToolOutput::ok(format!(
                    "--- PTY '{}' logs (cursor: {}) ---\n{}",
                    name,
                    latest_seq,
                    lines.join("\n")
                )))
            }

            HubCommand::PtySend { name, text, enter, keys } => {
                let broker = crate::pty::PtyBroker::global();
                let session = match broker.get(&name) {
                    Some(s) => s,
                    None => return Ok(ToolOutput::err(format!("No PTY session named '{}' found", name))),
                };

                if let Some(t) = text {
                    session.write_stdin(&t, enter)?;
                }
                for key in keys {
                    session.send_key(&key)?;
                }
                Ok(ToolOutput::ok(format!("Sent input to PTY '{}'", name)))
            }

            HubCommand::Stop { name } => {
                let broker = crate::pty::PtyBroker::global();
                match broker.stop(&name) {
                    Ok(_) => Ok(ToolOutput::ok(format!("Stopped PTY process '{}'", name))),
                    Err(e) => Ok(ToolOutput::err(format!("Failed to stop PTY '{}': {}", name, e))),
                }
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hub_schema_is_valid() {
        let tool = HubTool;
        let schema = tool.schema();
        assert_eq!(schema.name, "hub");
        assert!(!schema.description.is_empty());
    }

    #[test]
    fn test_hub_params_serde_send() {
        let json = serde_json::json!({
            "command": "send",
            "to": "agent-123",
            "message": "hello",
            "await_reply": false
        });
        let params: HubParams = serde_json::from_value(json).unwrap();
        match params.command {
            HubCommand::Send { to, message, await_reply, steer: _ } => {
                assert_eq!(to, Some("agent-123".to_string()));
                assert_eq!(message, "hello");
                assert!(!await_reply);
            }
            _ => panic!("expected Send"),
        }
    }

    #[test]
    fn test_hub_params_serde_list() {
        let json = serde_json::json!({
            "command": "list"
        });
        let params: HubParams = serde_json::from_value(json).unwrap();
        match params.command {
            HubCommand::List => {}
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn test_hub_params_serde_wait() {
        let json = serde_json::json!({
            "command": "wait",
            "from": "agent-456",
            "timeout_secs": 30
        });
        let params: HubParams = serde_json::from_value(json).unwrap();
        match params.command {
            HubCommand::Wait { from, timeout_secs } => {
                assert_eq!(from, Some("agent-456".to_string()));
                assert_eq!(timeout_secs, 30);
            }
            _ => panic!("expected Wait"),
        }
    }

    #[test]
    fn test_hub_params_serde_set_activity() {
        let json = serde_json::json!({
            "command": "set_activity",
            "activity": "searching for contacts"
        });
        let params: HubParams = serde_json::from_value(json).unwrap();
        match params.command {
            HubCommand::SetActivity { activity } => {
                assert_eq!(activity, "searching for contacts");
            }
            _ => panic!("expected SetActivity"),
        }
    }
}