use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// In-process and persistent subprocess evaluation kernel context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelState {
    pub session_id: String,
    pub language: String,
    pub variables: HashMap<String, serde_json::Value>,
}

/// DAG Task Stage representation for pipeline and parallel execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagStage {
    pub stage_name: String,
    pub tasks: Vec<DagTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagTask {
    pub task_id: String,
    pub prompt: String,
    pub agent_role: Option<String>,
    pub handle_output: bool,
}

/// Persistent Evaluation Kernel Manager across Python and Node.js.
pub struct EvalKernelManager {
    states: Arc<Mutex<HashMap<String, KernelState>>>,
}

impl Default for EvalKernelManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EvalKernelManager {
    pub fn new() -> Self {
        Self {
            states: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn get_or_create_state(&self, session_id: &str, language: &str) -> KernelState {
        let mut guard = self.states.lock().await;
        guard
            .entry(session_id.to_string())
            .or_insert_with(|| KernelState {
                session_id: session_id.to_string(),
                language: language.to_string(),
                variables: HashMap::new(),
            })
            .clone()
    }

    pub async fn update_variable(&self, session_id: &str, key: &str, value: serde_json::Value) {
        let mut guard = self.states.lock().await;
        if let Some(state) = guard.get_mut(session_id) {
            state.variables.insert(key.to_string(), value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_or_create_returns_new_state() {
        let mgr = EvalKernelManager::new();
        let s = mgr.get_or_create_state("sess-1", "python").await;
        assert_eq!(s.session_id, "sess-1");
        assert_eq!(s.language, "python");
        assert!(s.variables.is_empty());
    }

    #[tokio::test]
    async fn get_or_create_is_idempotent() {
        let mgr = EvalKernelManager::new();
        let a = mgr.get_or_create_state("sess-1", "python").await;
        let b = mgr.get_or_create_state("sess-1", "nodejs").await;
        // Second call returns the SAME state (original language kept).
        assert_eq!(b.language, "python");
        assert_eq!(a.session_id, b.session_id);
    }

    #[tokio::test]
    async fn update_variable_persists_across_get() {
        let mgr = EvalKernelManager::new();
        mgr.get_or_create_state("s", "python").await;
        mgr.update_variable("s", "x", serde_json::json!(42)).await;
        mgr.update_variable("s", "y", serde_json::json!({"k": [1, 2]}))
            .await;
        let s = mgr.get_or_create_state("s", "python").await;
        assert_eq!(s.variables["x"], serde_json::json!(42));
        assert_eq!(s.variables["y"], serde_json::json!({"k": [1, 2]}));
    }

    #[tokio::test]
    async fn update_variable_on_missing_session_is_noop() {
        let mgr = EvalKernelManager::new();
        mgr.update_variable("ghost", "x", serde_json::json!(1))
            .await;
        // No panic; the session simply does not exist.
        let s = mgr.get_or_create_state("ghost", "python").await;
        assert!(s.variables.is_empty());
    }

    #[tokio::test]
    async fn independent_sessions_are_isolated() {
        let mgr = EvalKernelManager::new();
        mgr.get_or_create_state("a", "python").await;
        mgr.get_or_create_state("b", "python").await;
        mgr.update_variable("a", "k", serde_json::json!("only-a"))
            .await;
        let sa = mgr.get_or_create_state("a", "python").await;
        let sb = mgr.get_or_create_state("b", "python").await;
        assert_eq!(sa.variables.len(), 1);
        assert!(sb.variables.is_empty());
    }

    #[test]
    fn dag_types_serde_roundtrip() {
        let stage = DagStage {
            stage_name: "build".into(),
            tasks: vec![DagTask {
                task_id: "t1".into(),
                prompt: "do x".into(),
                agent_role: Some("coder".into()),
                handle_output: true,
            }],
        };
        let json = serde_json::to_string(&stage).unwrap();
        let back: DagStage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.stage_name, "build");
        assert_eq!(back.tasks[0].task_id, "t1");
        assert_eq!(back.tasks[0].agent_role.as_deref(), Some("coder"));
    }
}
