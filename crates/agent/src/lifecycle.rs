//! Agent lifecycle management: park, revive, and release.
//!
//! After an agent finishes its task, [`AgentLifecycleManager`] can park it —
//! serialize its state (messages, tokens, config) to a JSON file and
//! unregister it from the IrcBus while keeping it in the AgentRegistry
//! as `Parked`. When a new message arrives for a parked agent, the
//! process-global [`IrcReviver`] hook (registered here) reloads the state
//! and creates a fresh [`AgentRuntime`] to handle the message.
//!
//! This is a facade over a simple directory of `.json` state files, plus
//! the global reviver callback.

use pr_core::agent::AgentRole;
use pr_core::irc::{AgentRegistry, IrcBus, IrcMessage, IrcReviver, PeerStatus, register_reviver};
use pr_core::ids::AgentId;
use pr_core::{AppConfig, Message, SessionId};
use pr_llm::LlmProvider;
use pr_persistence::Persistence;
use pr_tools::ToolRegistry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::runtime::AgentRuntime;

/// Saved state of a parked agent, serialized to JSON.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParkedAgentState {
    pub id: AgentId,
    pub session_id: String,
    pub parent_id: Option<AgentId>,
    pub role: AgentRole,
    pub task: String,
    pub depth: u32,
    pub messages: Vec<Message>,
    pub tokens_used: u64,
    pub descendant_tokens: u64,
    pub estimated_tokens: u32,
    pub config: AppConfig,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Handles checkpointing of agent state and the global reviver callback.
///
/// Singleton per process. Created once with all the shared infrastructure
/// needed to rebuild a runtime from parked state.
pub struct AgentLifecycleManager {
    park_dir: std::path::PathBuf,
    pub tools: Arc<ToolRegistry>,
    pub event_tx: broadcast::Sender<pr_core::AgentEvent>,
    pub db: Arc<Persistence>,
    pub default_llm: Arc<dyn LlmProvider>,
    pub role_llms: HashMap<String, Arc<dyn LlmProvider>>,
    pub cancel: CancellationToken,
}

impl AgentLifecycleManager {
    /// Create a new lifecycle manager and register the global reviver.
    pub fn new(
        tools: Arc<ToolRegistry>,
        event_tx: broadcast::Sender<pr_core::AgentEvent>,
        db: Arc<Persistence>,
        default_llm: Arc<dyn LlmProvider>,
        cancel: CancellationToken,
    ) -> Arc<Self> {
        let park_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
            .join(".fathom")
            .join("parked");
        let _ = std::fs::create_dir_all(&park_dir);

        let mgr = Arc::new(Self {
            park_dir,
            tools,
            event_tx,
            db,
            default_llm,
            role_llms: HashMap::new(),
            cancel,
        });

        // Register the global reviver so IrcBus can wake parked agents.
        let reviver: Arc<dyn IrcReviver> = mgr.clone();
        register_reviver(reviver);

        mgr
    }

    /// Set per-role LLM overrides.
    pub fn with_role_llms(mut self: Arc<Self>, map: HashMap<String, Arc<dyn LlmProvider>>) -> Arc<Self> {
        // Can't mutate through Arc, but we're the only early-stage builder.
        unsafe {
            let ptr = Arc::as_ptr(&self) as *mut Self;
            (*ptr).role_llms = map;
        }
        self
    }

    /// Park an agent: serialize its state, save to disk, unregister from
    /// IrcBus, update registry to Parked status.
    pub fn park(&self, runtime: &AgentRuntime) {
        let state = ParkedAgentState {
            id: runtime.id.clone(),
            session_id: runtime.session_id.0.clone(),
            parent_id: runtime.parent_id.clone(),
            role: runtime.role,
            task: runtime.task.clone(),
            depth: runtime.depth,
            messages: runtime.messages.clone(),
            tokens_used: runtime.tokens_used,
            descendant_tokens: runtime.descendant_tokens,
            estimated_tokens: runtime.estimated_tokens,
            config: runtime.config.clone(),
            created_at: chrono::Utc::now(),
        };

        let path = self.park_dir.join(format!("{}.json", runtime.id.0));
        if let Ok(json) = serde_json::to_string(&state) {
            if std::fs::write(&path, &json).is_ok() {
                tracing::debug!("parked agent {} to {}", runtime.id, path.display());
            }
        }

        IrcBus::global().unregister(&runtime.id);
        AgentRegistry::global().update_status(&runtime.id, PeerStatus::Parked);
    }

    /// Revive a parked agent: load state from disk, rebuild a minimal
    /// AgentRuntime, register on IrcBus, and return the runtime.
    /// Returns `None` if no parked state exists.
    pub fn revive(&self, id: &AgentId) -> Option<AgentRuntime> {
        let path = self.park_dir.join(format!("{}.json", id.0));
        let json = std::fs::read_to_string(&path).ok()?;
        let state: ParkedAgentState = serde_json::from_str(&json).ok()?;

        let role_key = match state.role {
            AgentRole::Coordinator => "coordinator",
            AgentRole::Researcher => "researcher",
            AgentRole::Analyst => "analyst",
            AgentRole::Verifier => "verifier",
            AgentRole::Writer => "writer",
        };
        let child_llm = self
            .role_llms
            .get(role_key)
            .cloned()
            .unwrap_or_else(|| self.default_llm.clone());

        let mut agent = AgentRuntime::new(
            state.id.clone(),
            SessionId(state.session_id.clone()),
            state.parent_id.clone(),
            state.role,
            state.task.clone(),
            state.depth,
            child_llm,
            self.tools.clone(),
            self.event_tx.clone(),
            self.db.clone(),
            // Use a temp working dir — the revived agent will be told its
            // exact task via the restored messages.
            self.park_dir.clone(),
            state.config,
        );

        // Restore conversation state
        agent.messages = state.messages;
        agent.tokens_used = state.tokens_used;
        agent.descendant_tokens = state.descendant_tokens;
        agent.estimated_tokens = state.estimated_tokens;

        // Register on the bus (replaces the mailbox entries)
        agent.register_with_bus();

        // Clean up the parked file
        let _ = std::fs::remove_file(&path);

        Some(agent)
    }

    /// Release a parked agent: remove state file, unregister entirely.
    pub fn release(&self, id: &AgentId) {
        let path = self.park_dir.join(format!("{}.json", id.0));
        let _ = std::fs::remove_file(&path);
        IrcBus::global().unregister(id);
        AgentRegistry::global().unregister(id);
    }

    /// Check if a parked state file exists for an agent.
    pub fn is_parked(&self, id: &AgentId) -> bool {
        self.park_dir.join(format!("{}.json", id.0)).exists()
    }

    /// List all parked agent ids.
    pub fn list_parked(&self) -> Vec<String> {
        let mut ids = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.park_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".json") {
                        ids.push(name.trim_end_matches(".json").to_string());
                    }
                }
            }
        }
        ids
    }
}

impl IrcReviver for AgentLifecycleManager {
    fn revive(&self, id: &AgentId, msg: IrcMessage) -> bool {
        if !self.is_parked(id) {
            return false;
        }

        tracing::info!("reviving parked agent {} to deliver message", id);

        // Rebuild the runtime
        let Some(mut agent) = self.revive(id) else {
            return false;
        };

        // Push the message into the agent's message history
        agent.messages.push(Message::user(format!(
            "[INBOX from agent {}] {}",
            msg.from, msg.content
        )));

        // Spawn a new tokio task to run the revived agent
        let tools = self.tools.clone();
        let event_tx = self.event_tx.clone();
        let db = self.db.clone();
        let cancel = self.cancel.clone();
        let id = id.clone();

        tokio::spawn(async move {
            // Run the agent for one more turn to process the message
            match agent.run().await {
                Ok(output) => {
                    tracing::info!("revived agent {} completed: {}", id, output.summary.chars().take(100).collect::<String>());
                }
                Err(e) => {
                    tracing::warn!("revived agent {} failed: {}", id, e);
                }
            }
        });

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pr_core::{AgentRole, SessionId};
    use pr_persistence::Persistence;
    use pr_llm::{CompletionRequest, CompletionResponse, LlmProvider, StreamChunk};
    use pr_core::{PrError, PrResult};
    use async_trait::async_trait;
    use futures::Stream;

    struct MockProvider;
    #[async_trait]
    impl LlmProvider for MockProvider {
        fn name(&self) -> &str { "mock" }
        fn model(&self) -> &str { "mock" }
        async fn complete(&self, _: &CompletionRequest) -> PrResult<CompletionResponse> {
            Ok(CompletionResponse {
                message: pr_core::Message::assistant("ok"),
                usage: None,
                finish_reason: Some("stop".to_string()),
            })
        }
        async fn stream(&self, _: &CompletionRequest) -> PrResult<Box<dyn Stream<Item=PrResult<StreamChunk>> + Send + Unpin>> {
            Err(PrError::Llm("unused".into()))
        }
    }

    #[test]
    fn test_parked_state_serde() {
        let state = ParkedAgentState {
            id: AgentId::new(),
            session_id: "sess-test".to_string(),
            parent_id: None,
            role: AgentRole::Researcher,
            task: "test".to_string(),
            depth: 0,
            messages: vec![
                Message::system("system prompt"),
                Message::user("user query"),
            ],
            tokens_used: 100,
            descendant_tokens: 50,
            estimated_tokens: 30,
            config: AppConfig::default(),
            created_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&state).unwrap();
        let restored: ParkedAgentState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id.0, state.id.0);
        assert_eq!(restored.messages.len(), 2);
        assert_eq!(restored.tokens_used, 100);
    }

    #[tokio::test]
    async fn test_park_and_revive_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let park_dir = tmp.path().join("parked");
        std::fs::create_dir_all(&park_dir).unwrap();

        let db = Arc::new(Persistence::in_memory().unwrap());
        let (tx, _) = broadcast::channel(64);
        let tools = Arc::new(ToolRegistry::new());
        let llm = Arc::new(MockProvider);
        let cancel = CancellationToken::new();

        // Create a minimal runtime
        let agent_id = AgentId::new();
        let session_id = SessionId::new();
        db.create_session(&session_id, "test").unwrap();

        let mut agent = AgentRuntime::new(
            agent_id.clone(),
            session_id.clone(),
            None,
            AgentRole::Researcher,
            "test task".to_string(),
            0,
            Arc::new(MockProvider),
            Arc::new(ToolRegistry::new()),
            broadcast::channel(64).0,
            db.clone(),
            tmp.path().to_path_buf(),
            AppConfig::default(),
        );
        agent.messages.push(Message::system("test"));
        agent.register_with_bus();

        // Park it manually via the manager
        let mgr = AgentLifecycleManager::new(
            tools,
            tx,
            db,
            llm,
            cancel,
        );

        // Override park dir for testing
        unsafe {
            let ptr = Arc::as_ptr(&mgr) as *mut AgentLifecycleManager;
            (*ptr).park_dir = park_dir.clone();
        }

        // Must be registered before park
        assert!(IrcBus::global().is_registered(&agent_id));

        mgr.park(&agent);
        // After park, no longer on live bus
        assert!(!IrcBus::global().is_registered(&agent_id));
        // But registry shows Parked
        let ref_ = AgentRegistry::global().get(&agent_id);
        assert!(ref_.is_some());
        assert_eq!(ref_.unwrap().status, PeerStatus::Parked);

        // Can revive
        assert!(mgr.is_parked(&agent_id));
        let revived = mgr.revive(&agent_id);
        assert!(revived.is_some());
        let revived = revived.unwrap();
        // Revived agent is registered again
        assert!(IrcBus::global().is_registered(&revived.id));
        // State was restored
        assert_eq!(revived.messages.len(), 1);
        assert_eq!(revived.task, "test task");

        // Clean up
        mgr.release(&agent_id);
        assert!(!mgr.is_parked(&agent_id));
    }
}