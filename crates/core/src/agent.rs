use serde::{Deserialize, Serialize};
use crate::ids::AgentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentRole {
    Coordinator,
    Researcher,
    Analyst,
    Verifier,
    Writer,
}

impl std::fmt::Display for AgentRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Coordinator => write!(f, "coordinator"),
            Self::Researcher => write!(f, "researcher"),
            Self::Analyst => write!(f, "analyst"),
            Self::Verifier => write!(f, "verifier"),
            Self::Writer => write!(f, "writer"),
        }
    }
}

impl AgentRole {
    pub fn can_spawn_children(&self) -> bool {
        matches!(self, Self::Coordinator | Self::Researcher | Self::Analyst)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state")]
pub enum AgentState {
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "planning")]
    Planning { query: String },
    #[serde(rename = "researching")]
    Researching { sub_tasks: Vec<String> },
    #[serde(rename = "analyzing")]
    Analyzing,
    #[serde(rename = "synthesizing")]
    Synthesizing,
    #[serde(rename = "writing")]
    Writing,
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Spawned,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRecord {
    pub id: AgentId,
    pub session_id: String,
    pub parent_id: Option<AgentId>,
    pub role: AgentRole,
    pub task: String,
    pub status: AgentStatus,
    pub depth: u32,
    pub tokens_used: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_role_display() {
        assert_eq!(AgentRole::Coordinator.to_string(), "coordinator");
        assert_eq!(AgentRole::Researcher.to_string(), "researcher");
        assert_eq!(AgentRole::Analyst.to_string(), "analyst");
        assert_eq!(AgentRole::Verifier.to_string(), "verifier");
        assert_eq!(AgentRole::Writer.to_string(), "writer");
    }

    #[test]
    fn can_spawn_children() {
        assert!(AgentRole::Coordinator.can_spawn_children());
        assert!(AgentRole::Researcher.can_spawn_children());
        assert!(AgentRole::Analyst.can_spawn_children());
        assert!(!AgentRole::Verifier.can_spawn_children());
        assert!(!AgentRole::Writer.can_spawn_children());
    }

    #[test]
    fn agent_role_serde_roundtrip() {
        for role in [AgentRole::Coordinator, AgentRole::Researcher, AgentRole::Analyst, AgentRole::Verifier, AgentRole::Writer] {
            let json = serde_json::to_string(&role).unwrap();
            let back: AgentRole = serde_json::from_str(&json).unwrap();
            assert_eq!(role, back);
        }
    }

    #[test]
    fn agent_role_serde_values() {
        assert_eq!(serde_json::to_string(&AgentRole::Coordinator).unwrap(), "\"coordinator\"");
        assert_eq!(serde_json::to_string(&AgentRole::Writer).unwrap(), "\"writer\"");
    }

    #[test]
    fn agent_status_serde_roundtrip() {
        for status in [AgentStatus::Spawned, AgentStatus::Running, AgentStatus::Completed, AgentStatus::Failed, AgentStatus::Cancelled] {
            let json = serde_json::to_string(&status).unwrap();
            let back: AgentStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, back);
        }
    }

    #[test]
    fn agent_state_serde_idle() {
        let json = serde_json::to_string(&AgentState::Idle).unwrap();
        assert!(json.contains("idle"));
        let back: AgentState = serde_json::from_str(&json).unwrap();
        matches!(back, AgentState::Idle);
    }

    #[test]
    fn agent_state_serde_planning() {
        let state = AgentState::Planning { query: "test query".into() };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("planning"));
        assert!(json.contains("test query"));
        let back: AgentState = serde_json::from_str(&json).unwrap();
        match back {
            AgentState::Planning { query } => assert_eq!(query, "test query"),
            _ => panic!("expected Planning"),
        }
    }

    #[test]
    fn agent_state_serde_error() {
        let state = AgentState::Error { message: "something broke".into() };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("error"));
        let back: AgentState = serde_json::from_str(&json).unwrap();
        match back {
            AgentState::Error { message } => assert_eq!(message, "something broke"),
            _ => panic!("expected Error"),
        }
    }

    #[test]
    fn agent_state_serde_researching() {
        let state = AgentState::Researching { sub_tasks: vec!["a".into(), "b".into()] };
        let json = serde_json::to_string(&state).unwrap();
        let back: AgentState = serde_json::from_str(&json).unwrap();
        match back {
            AgentState::Researching { sub_tasks } => assert_eq!(sub_tasks, vec!["a", "b"]),
            _ => panic!("expected Researching"),
        }
    }

    #[test]
    fn agent_record_serde() {
        let record = AgentRecord {
            id: AgentId::new(),
            session_id: "sess-1".into(),
            parent_id: None,
            role: AgentRole::Researcher,
            task: "find stuff".into(),
            status: AgentStatus::Running,
            depth: 1,
            tokens_used: 1000,
            created_at: chrono::Utc::now(),
            completed_at: None,
        };
        let json = serde_json::to_string(&record).unwrap();
        let back: AgentRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, record.id);
        assert_eq!(back.role, AgentRole::Researcher);
        assert_eq!(back.status, AgentStatus::Running);
        assert!(back.completed_at.is_none());
    }
}
