//! Process-global registry of agent steering channels.
//!
//! Each live agent registers its steering receiver (an mpsc channel the
//! agent drains at its next turn boundary, see fleet E1 in
//! `AgentRuntime::run`). The [`SteerRegistry`] lets any component — the
//! HTTP API, another agent via the `hub` tool, or a supervisor — deliver a
//! mid-run instruction to a running agent by id.
//!
//! This complements the IrcBus: IRC messages are *conversational* (the
//! agent reads them from its inbox and may ignore them), while steering is
//! *directive* (injected with a `[USER INSTRUCTION]` marker so the model
//! treats it as a command).

use crate::ids::AgentId;
use std::collections::HashMap;
use std::sync::LazyLock;
use parking_lot::Mutex;
use tokio::sync::mpsc;

/// Registry mapping agent ids to their steering sender half.
pub struct SteerRegistry {
    channels: Mutex<HashMap<String, mpsc::UnboundedSender<String>>>,
}

impl SteerRegistry {
    fn new() -> Self {
        Self {
            channels: Mutex::new(HashMap::new()),
        }
    }

    /// Access the process-global singleton.
    pub fn global() -> &'static Self {
        static REG: LazyLock<SteerRegistry> = LazyLock::new(SteerRegistry::new);
        &REG
    }

    /// Register an agent's steering sender. Returns nothing; the receiver
    /// half was already handed to the agent by the runtime.
    pub fn register(&self, id: &AgentId, tx: mpsc::UnboundedSender<String>) {
        self.channels.lock().insert(id.0.clone(), tx);
    }

    /// Unregister an agent.
    pub fn unregister(&self, id: &AgentId) {
        self.channels.lock().remove(&id.0);
    }

    /// Deliver a mid-run instruction to an agent. Returns `true` when the
    /// channel was found and the message enqueued; `false` when the agent
    /// is not registered (or its channel is closed).
    pub fn steer(&self, id: &AgentId, instruction: String) -> bool {
        let Some(tx) = self.channels.lock().get(&id.0).cloned() else {
            return false;
        };
        tx.send(instruction).is_ok()
    }

    /// Whether an agent has a registered steering channel (i.e. is alive
    /// and draining instructions at turn boundaries).
    pub fn is_registered(&self, id: &AgentId) -> bool {
        self.channels.lock().contains_key(&id.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_steer_delivers() {
        let reg = SteerRegistry::new();
        let id = AgentId::new();
        let (tx, mut rx) = mpsc::unbounded_channel();
        reg.register(&id, tx);

        assert!(reg.is_registered(&id));
        assert!(reg.steer(&id, "change direction".to_string()));

        let msg = rx.try_recv().unwrap();
        assert_eq!(msg, "change direction");

        reg.unregister(&id);
        assert!(!reg.is_registered(&id));
        assert!(!reg.steer(&id, "gone".to_string()));
    }

    #[test]
    fn test_steer_unknown_agent_fails() {
        let reg = SteerRegistry::new();
        let id = AgentId::new();
        assert!(!reg.steer(&id, "nobody".to_string()));
    }
}