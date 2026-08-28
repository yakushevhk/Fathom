use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};
use crate::{PrError, PrResult};

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

impl EvalKernelManager {
    pub fn new() -> Self {
        Self {
            states: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn get_or_create_state(&self, session_id: &str, language: &str) -> KernelState {
        let mut guard = self.states.lock().await;
        guard.entry(session_id.to_string()).or_insert_with(|| KernelState {
            session_id: session_id.to_string(),
            language: language.to_string(),
            variables: HashMap::new(),
        }).clone()
    }

    pub async fn update_variable(&self, session_id: &str, key: &str, value: serde_json::Value) {
        let mut guard = self.states.lock().await;
        if let Some(state) = guard.get_mut(session_id) {
            state.variables.insert(key.to_string(), value);
        }
    }
}
