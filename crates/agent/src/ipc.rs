//! IPC protocol for multi-process agent coordination.
//! Uses Unix domain sockets for communication between coordinator and workers.

use serde::{Deserialize, Serialize};
use pr_core::{AgentEvent, AgentId, AgentRole, AgentState};

/// Messages sent between coordinator and worker processes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcMessage {
    // Coordinator -> Worker
    #[serde(rename = "start_task")]
    StartTask {
        task: String,
        role: AgentRole,
        depth: u32,
    },
    #[serde(rename = "cancel")]
    Cancel,

    // Worker -> Coordinator
    #[serde(rename = "progress")]
    Progress {
        agent_id: AgentId,
        state: AgentState,
    },
    #[serde(rename = "tool_call")]
    ToolCall {
        agent_id: AgentId,
        tool: String,
        args: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        agent_id: AgentId,
        tool: String,
        result_preview: String,
        duration_ms: u64,
    },
    #[serde(rename = "completed")]
    Completed {
        agent_id: AgentId,
        summary: String,
        tokens_used: u64,
    },
    #[serde(rename = "failed")]
    Failed {
        agent_id: AgentId,
        error: String,
    },
    #[serde(rename = "llm_chunk")]
    LlmChunk {
        agent_id: AgentId,
        chunk: String,
    },
}

impl IpcMessage {
    /// Serialize message to JSON line (for socket transmission).
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).unwrap_or_default() + "\n"
    }

    /// Parse message from JSON line.
    pub fn from_line(line: &str) -> Option<Self> {
        serde_json::from_str(line.trim()).ok()
    }

    /// Convert a worker->coordinator IPC message into the equivalent
    /// [`AgentEvent`] so the coordinator can re-emit it on its local event
    /// bus (feeding the TUI / headless progress output).
    ///
    /// Terminal messages (`Completed`, `Failed`) and coordinator->worker
    /// messages return `None`; the caller handles those explicitly.
    pub fn to_agent_event(&self) -> Option<AgentEvent> {
        match self {
            IpcMessage::Progress { agent_id, state } => Some(AgentEvent::AgentStateChanged {
                id: agent_id.clone(),
                state: state.clone(),
            }),
            IpcMessage::ToolCall { agent_id, tool, args } => Some(AgentEvent::ToolCallStarted {
                agent_id: agent_id.clone(),
                tool: tool.clone(),
                args: args.clone(),
            }),
            IpcMessage::ToolResult {
                agent_id,
                tool,
                result_preview,
                duration_ms,
            } => Some(AgentEvent::ToolCallCompleted {
                agent_id: agent_id.clone(),
                tool: tool.clone(),
                result_preview: result_preview.clone(),
                duration_ms: *duration_ms,
            }),
            IpcMessage::LlmChunk { agent_id, chunk } => Some(AgentEvent::LlmStreamChunk {
                agent_id: agent_id.clone(),
                chunk: chunk.clone(),
            }),
            _ => None,
        }
    }
}

/// Convert a local [`AgentEvent`] produced by a worker's [`AgentRuntime`]
/// into the IPC message the worker sends to its coordinator.
///
/// Only events belonging to `agent_id` that have an IPC representation are
/// converted; everything else returns `None`.
pub fn agent_event_to_ipc(event: &AgentEvent, agent_id: &AgentId) -> Option<IpcMessage> {
    match event {
        AgentEvent::AgentStateChanged { id, state } if id == agent_id => {
            Some(IpcMessage::Progress {
                agent_id: id.clone(),
                state: state.clone(),
            })
        }
        AgentEvent::ToolCallStarted { agent_id: id, tool, args } if id == agent_id => {
            Some(IpcMessage::ToolCall {
                agent_id: id.clone(),
                tool: tool.clone(),
                args: args.clone(),
            })
        }
        AgentEvent::ToolCallCompleted {
            agent_id: id,
            tool,
            result_preview,
            duration_ms,
        } if id == agent_id => Some(IpcMessage::ToolResult {
            agent_id: id.clone(),
            tool: tool.clone(),
            result_preview: result_preview.clone(),
            duration_ms: *duration_ms,
        }),
        AgentEvent::LlmStreamChunk { agent_id: id, chunk } if id == agent_id => {
            Some(IpcMessage::LlmChunk {
                agent_id: id.clone(),
                chunk: chunk.clone(),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_roundtrip() {
        let msg = IpcMessage::Progress {
            agent_id: AgentId::new(),
            state: AgentState::Researching { sub_tasks: vec![] },
        };
        let line = msg.to_line();
        let parsed = IpcMessage::from_line(&line).unwrap();
        assert!(matches!(parsed, IpcMessage::Progress { .. }));
    }

    #[test]
    fn test_completed_message() {
        let msg = IpcMessage::Completed {
            agent_id: AgentId::new(),
            summary: "test summary".to_string(),
            tokens_used: 1000,
        };
        let line = msg.to_line();
        assert!(line.contains("completed"));
        assert!(line.contains("test summary"));
    }

    #[test]
    fn test_ipc_to_agent_event_progress() {
        let agent_id = AgentId::new();
        let msg = IpcMessage::Progress {
            agent_id: agent_id.clone(),
            state: AgentState::Researching { sub_tasks: vec![] },
        };
        let event = msg.to_agent_event().unwrap();
        assert!(matches!(event, AgentEvent::AgentStateChanged { id, .. } if id == agent_id));
    }

    #[test]
    fn test_ipc_to_agent_event_tool_roundtrip() {
        let agent_id = AgentId::new();
        let msg = IpcMessage::ToolCall {
            agent_id: agent_id.clone(),
            tool: "web_search".to_string(),
            args: serde_json::json!({"query": "rust"}),
        };
        let event = msg.to_agent_event().unwrap();
        assert!(matches!(event, AgentEvent::ToolCallStarted { tool, .. } if tool == "web_search"));

        let msg = IpcMessage::ToolResult {
            agent_id: agent_id.clone(),
            tool: "web_search".to_string(),
            result_preview: "ok".to_string(),
            duration_ms: 42,
        };
        let event = msg.to_agent_event().unwrap();
        assert!(matches!(event, AgentEvent::ToolCallCompleted { duration_ms: 42, .. }));
    }

    #[test]
    fn test_ipc_to_agent_event_llm_chunk() {
        let msg = IpcMessage::LlmChunk {
            agent_id: AgentId::new(),
            chunk: "hello".to_string(),
        };
        let event = msg.to_agent_event().unwrap();
        assert!(matches!(event, AgentEvent::LlmStreamChunk { chunk, .. } if chunk == "hello"));
    }

    #[test]
    fn test_terminal_messages_have_no_agent_event() {
        let agent_id = AgentId::new();
        assert!(IpcMessage::Completed {
            agent_id: agent_id.clone(),
            summary: "done".into(),
            tokens_used: 1,
        }
        .to_agent_event()
        .is_none());
        assert!(IpcMessage::Failed {
            agent_id: agent_id.clone(),
            error: "boom".into(),
        }
        .to_agent_event()
        .is_none());
        assert!(IpcMessage::Cancel.to_agent_event().is_none());
        assert!(IpcMessage::StartTask {
            task: "t".into(),
            role: AgentRole::Researcher,
            depth: 1,
        }
        .to_agent_event()
        .is_none());
    }

    #[test]
    fn test_agent_event_to_ipc_mapping() {
        let agent_id = AgentId::new();

        let event = AgentEvent::ToolCallStarted {
            agent_id: agent_id.clone(),
            tool: "shell".to_string(),
            args: serde_json::json!({}),
        };
        assert!(matches!(
            agent_event_to_ipc(&event, &agent_id),
            Some(IpcMessage::ToolCall { .. })
        ));

        let event = AgentEvent::LlmStreamChunk {
            agent_id: agent_id.clone(),
            chunk: "x".to_string(),
        };
        assert!(matches!(
            agent_event_to_ipc(&event, &agent_id),
            Some(IpcMessage::LlmChunk { .. })
        ));

        // Events from a different agent are not forwarded.
        let other = AgentId::new();
        let event = AgentEvent::LlmStreamChunk {
            agent_id: other.clone(),
            chunk: "x".to_string(),
        };
        assert!(agent_event_to_ipc(&event, &agent_id).is_none());

        // Terminal events are handled by the worker itself, not forwarded.
        let event = AgentEvent::AgentCompleted {
            id: agent_id.clone(),
            summary: "done".to_string(),
            tokens_used: 5,
        };
        assert!(agent_event_to_ipc(&event, &agent_id).is_none());
    }

    #[test]
    fn test_event_ipc_event_roundtrip() {
        let agent_id = AgentId::new();
        let event = AgentEvent::ToolCallStarted {
            agent_id: agent_id.clone(),
            tool: "web_fetch".to_string(),
            args: serde_json::json!({"url": "https://example.com"}),
        };
        let msg = agent_event_to_ipc(&event, &agent_id).unwrap();
        let line = msg.to_line();
        let parsed = IpcMessage::from_line(&line).unwrap();
        let back = parsed.to_agent_event().unwrap();
        assert!(matches!(
            back,
            AgentEvent::ToolCallStarted { tool, .. } if tool == "web_fetch"
        ));
    }
}
