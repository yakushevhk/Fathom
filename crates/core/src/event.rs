use serde::{Deserialize, Serialize};
use crate::ids::{AgentId, SessionId};
use crate::agent::AgentState;
use crate::finding::Finding;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentEvent {
    #[serde(rename = "session_started")]
    SessionStarted {
        id: SessionId,
        query: String,
    },
    #[serde(rename = "agent_spawned")]
    AgentSpawned {
        id: AgentId,
        parent: Option<AgentId>,
        role: String,
        task: String,
        depth: u32,
    },
    #[serde(rename = "agent_state_changed")]
    AgentStateChanged {
        id: AgentId,
        state: AgentState,
    },
    #[serde(rename = "finding")]
    Finding {
        agent_id: AgentId,
        finding: Finding,
    },
    #[serde(rename = "tool_call_started")]
    ToolCallStarted {
        agent_id: AgentId,
        tool: String,
        args: serde_json::Value,
    },
    #[serde(rename = "tool_call_completed")]
    ToolCallCompleted {
        agent_id: AgentId,
        tool: String,
        result_preview: String,
        duration_ms: u64,
    },
    #[serde(rename = "llm_stream_chunk")]
    LlmStreamChunk {
        agent_id: AgentId,
        chunk: String,
    },
    #[serde(rename = "agent_completed")]
    AgentCompleted {
        id: AgentId,
        summary: String,
        tokens_used: u64,
    },
    #[serde(rename = "agent_failed")]
    AgentFailed {
        id: AgentId,
        error: String,
    },
    #[serde(rename = "session_completed")]
    SessionCompleted {
        id: SessionId,
        output_dir: String,
        total_tokens: u64,
        total_agents: u32,
    },
    #[serde(rename = "session_failed")]
    SessionFailed {
        id: SessionId,
        error: String,
    },
    /// An agent invoked the `question` tool and is waiting for an answer.
    /// Hosts (TUI/HTTP) answer through the control plane using `request_id`.
    #[serde(rename = "question_asked")]
    QuestionAsked {
        agent_id: AgentId,
        request_id: String,
        question: String,
    },
    /// An agent wants to run a side-effect tool that requires approval.
    /// Hosts allow/deny through the control plane using `request_id`.
    #[serde(rename = "approval_requested")]
    ApprovalRequested {
        agent_id: AgentId,
        request_id: String,
        tool: String,
        args_preview: String,
    },
    /// A session was forked from another session.
    #[serde(rename = "session_forked")]
    SessionForked {
        parent_id: SessionId,
        child_id: SessionId,
        query: String,
    },
    /// A file change was undone.
    #[serde(rename = "file_change_undone")]
    FileChangeUndone {
        session_id: SessionId,
        file_path: String,
        operation: String,
    },
    /// A session title was auto-generated.
    #[serde(rename = "title_generated")]
    TitleGenerated {
        session_id: SessionId,
        title: String,
    },
}

impl AgentEvent {
    /// The agent this event belongs to, if it is agent-scoped.
    pub fn agent_id(&self) -> Option<&AgentId> {
        match self {
            AgentEvent::AgentSpawned { id, .. }
            | AgentEvent::AgentStateChanged { id, .. }
            | AgentEvent::AgentCompleted { id, .. }
            | AgentEvent::AgentFailed { id, .. } => Some(id),
            AgentEvent::Finding { agent_id, .. }
            | AgentEvent::ToolCallStarted { agent_id, .. }
            | AgentEvent::ToolCallCompleted { agent_id, .. }
            | AgentEvent::LlmStreamChunk { agent_id, .. }
            | AgentEvent::QuestionAsked { agent_id, .. }
            | AgentEvent::ApprovalRequested { agent_id, .. } => Some(agent_id),
            _ => None,
        }
    }

    /// The session this event belongs to, if it is session-scoped.
    pub fn session_id(&self) -> Option<&SessionId> {
        match self {
            AgentEvent::SessionStarted { id, .. }
            | AgentEvent::SessionCompleted { id, .. }
            | AgentEvent::SessionFailed { id, .. } => Some(id),
            AgentEvent::SessionForked { child_id, .. } => Some(child_id),
            AgentEvent::FileChangeUndone { session_id, .. } => Some(session_id),
            AgentEvent::TitleGenerated { session_id, .. } => Some(session_id),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{AgentId, SessionId};

    #[test]
    fn agent_id_on_agent_scoped_events() {
        let aid = AgentId::new();
        let sid = SessionId::new();

        let finding = crate::finding::Finding {
            id: crate::ids::FindingId::new(),
            agent_id: aid.clone(),
            title: "f".into(),
            content: "c".into(),
            sources: vec![],
            confidence: 0.9,
            created_at: chrono::Utc::now(),
        };

        // Events that have an agent_id field
        let events = vec![
            AgentEvent::AgentSpawned { id: aid.clone(), parent: None, role: "r".into(), task: "t".into(), depth: 0 },
            AgentEvent::AgentStateChanged { id: aid.clone(), state: AgentState::Idle },
            AgentEvent::AgentCompleted { id: aid.clone(), summary: "done".into(), tokens_used: 100 },
            AgentEvent::AgentFailed { id: aid.clone(), error: "err".into() },
            AgentEvent::Finding { agent_id: aid.clone(), finding },
            AgentEvent::ToolCallStarted { agent_id: aid.clone(), tool: "t".into(), args: serde_json::json!({}) },
            AgentEvent::ToolCallCompleted { agent_id: aid.clone(), tool: "t".into(), result_preview: "ok".into(), duration_ms: 10 },
            AgentEvent::LlmStreamChunk { agent_id: aid.clone(), chunk: "hi".into() },
        ];

        for ev in &events {
            assert!(ev.agent_id().is_some(), "expected agent_id for {:?}", ev);
        }

        // Session-scoped events have no agent_id
        let session_events = vec![
            AgentEvent::SessionStarted { id: sid.clone(), query: "q".into() },
            AgentEvent::SessionCompleted { id: sid.clone(), output_dir: "/tmp".into(), total_tokens: 0, total_agents: 0 },
            AgentEvent::SessionFailed { id: sid.clone(), error: "e".into() },
        ];
        for ev in &session_events {
            assert!(ev.agent_id().is_none(), "expected no agent_id for {:?}", ev);
        }
    }

    #[test]
    fn session_id_on_session_scoped_events() {
        let sid = SessionId::new();
        let aid = AgentId::new();

        let session_events = vec![
            AgentEvent::SessionStarted { id: sid.clone(), query: "q".into() },
            AgentEvent::SessionCompleted { id: sid.clone(), output_dir: "/tmp".into(), total_tokens: 0, total_agents: 0 },
            AgentEvent::SessionFailed { id: sid.clone(), error: "e".into() },
        ];
        for ev in &session_events {
            assert!(ev.session_id().is_some(), "expected session_id for {:?}", ev);
        }

        let agent_events = vec![
            AgentEvent::AgentSpawned { id: aid.clone(), parent: None, role: "r".into(), task: "t".into(), depth: 0 },
            AgentEvent::AgentCompleted { id: aid.clone(), summary: "s".into(), tokens_used: 0 },
        ];
        for ev in &agent_events {
            assert!(ev.session_id().is_none(), "expected no session_id for {:?}", ev);
        }
    }

    #[test]
    fn serde_roundtrip_session_started() {
        let ev = AgentEvent::SessionStarted { id: SessionId::new(), query: "test".into() };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains("session_started"));
        let back: AgentEvent = serde_json::from_str(&json).unwrap();
        match back {
            AgentEvent::SessionStarted { query, .. } => assert_eq!(query, "test"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn serde_roundtrip_agent_spawned() {
        let ev = AgentEvent::AgentSpawned {
            id: AgentId::new(),
            parent: Some(AgentId::new()),
            role: "researcher".into(),
            task: "find info".into(),
            depth: 2,
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains("agent_spawned"));
        let back: AgentEvent = serde_json::from_str(&json).unwrap();
        match back {
            AgentEvent::AgentSpawned { role, task, depth, .. } => {
                assert_eq!(role, "researcher");
                assert_eq!(task, "find info");
                assert_eq!(depth, 2);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn serde_roundtrip_tool_call_completed() {
        let ev = AgentEvent::ToolCallCompleted {
            agent_id: AgentId::new(),
            tool: "web_search".into(),
            result_preview: "Found 10 results".into(),
            duration_ms: 1234,
        };
        let json = serde_json::to_string(&ev).unwrap();
        let back: AgentEvent = serde_json::from_str(&json).unwrap();
        match back {
            AgentEvent::ToolCallCompleted { tool, duration_ms, .. } => {
                assert_eq!(tool, "web_search");
                assert_eq!(duration_ms, 1234);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn serde_all_variants() {
        let sid = SessionId::new();
        let aid = AgentId::new();
        let finding = crate::finding::Finding {
            id: crate::ids::FindingId::new(),
            agent_id: aid.clone(),
            title: "t".into(),
            content: "c".into(),
            sources: vec![],
            confidence: 0.5,
            created_at: chrono::Utc::now(),
        };
        let variants = vec![
            AgentEvent::SessionStarted { id: sid.clone(), query: "q".into() },
            AgentEvent::AgentSpawned { id: aid.clone(), parent: None, role: "r".into(), task: "t".into(), depth: 0 },
            AgentEvent::AgentStateChanged { id: aid.clone(), state: AgentState::Idle },
            AgentEvent::Finding { agent_id: aid.clone(), finding },
            AgentEvent::ToolCallStarted { agent_id: aid.clone(), tool: "t".into(), args: serde_json::json!({}) },
            AgentEvent::ToolCallCompleted { agent_id: aid.clone(), tool: "t".into(), result_preview: "r".into(), duration_ms: 0 },
            AgentEvent::LlmStreamChunk { agent_id: aid.clone(), chunk: "c".into() },
            AgentEvent::AgentCompleted { id: aid.clone(), summary: "s".into(), tokens_used: 0 },
            AgentEvent::AgentFailed { id: aid.clone(), error: "e".into() },
            AgentEvent::SessionCompleted { id: sid.clone(), output_dir: "/tmp".into(), total_tokens: 0, total_agents: 0 },
            AgentEvent::SessionFailed { id: sid.clone(), error: "e".into() },
            AgentEvent::SessionForked { parent_id: sid.clone(), child_id: SessionId::new(), query: "forked".into() },
            AgentEvent::FileChangeUndone { session_id: sid.clone(), file_path: "/tmp/f.rs".into(), operation: "edit".into() },
            AgentEvent::TitleGenerated { session_id: sid.clone(), title: "New Title".into() },
        ];
        for ev in variants {
            let json = serde_json::to_string(&ev).unwrap();
            let back: AgentEvent = serde_json::from_str(&json).unwrap();
            // Just verify roundtrip doesn't panic
            let _ = serde_json::to_string(&back).unwrap();
        }
    }
}
