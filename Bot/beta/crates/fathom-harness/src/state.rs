//! Shared harness state: store, event bus, session table, engine registry.

use crate::drivers;
use crate::engine::*;
use crate::sessions::QueuedSend;
use fathom_core::types::*;
use fathom_core::Store;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex};

pub struct AppState {
    pub store: Store,
    /// SSE fan-out: every ServerEvent goes to every subscriber.
    pub bus: broadcast::Sender<ServerEvent>,
    /// session key (thread_id, or thread_id#bot_id in rooms) → live session.
    pub sessions: SessionMap,
    /// approval request_id → decision resolver.
    pub decisions: SharedDecisions,
    /// Session keys with a turn in flight.
    pub running: Mutex<HashSet<String>>,
    /// Sends waiting behind a running turn, per session key.
    pub queues: Mutex<HashMap<String, VecDeque<QueuedSend>>>,
    /// Live stdin steer channels, per session key.
    pub steers: Mutex<HashMap<String, mpsc::UnboundedSender<String>>>,
    drivers: HashMap<EngineKind, Arc<dyn EngineDriver>>,
}

impl AppState {
    pub fn new(store: Store) -> Arc<Self> {
        let mut drivers: HashMap<EngineKind, Arc<dyn EngineDriver>> = HashMap::new();
        drivers.insert(EngineKind::Claude, Arc::new(drivers::claude::ClaudeDriver));
        drivers.insert(EngineKind::Codex, Arc::new(drivers::codex::CodexDriver));
        drivers.insert(EngineKind::Fathom, Arc::new(drivers::fathom::FathomDriver));
        drivers.insert(
            EngineKind::Grok,
            Arc::new(drivers::openai_chat::grok_driver()),
        );
        drivers.insert(
            EngineKind::OpenAiCompat,
            Arc::new(drivers::openai_chat::openai_compat_driver()),
        );
        let (bus, _) = broadcast::channel(1024);
        Arc::new(Self {
            store,
            bus,
            sessions: new_session_map(),
            decisions: Arc::new(Mutex::new(HashMap::new())),
            running: Mutex::new(HashSet::new()),
            queues: Mutex::new(HashMap::new()),
            steers: Mutex::new(HashMap::new()),
            drivers,
        })
    }

    pub fn driver_for(&self, kind: EngineKind) -> Arc<dyn EngineDriver> {
        self.drivers
            .get(&kind)
            .expect("all engine kinds registered")
            .clone()
    }

    /// Engine config from the kv table (`engines.<kind>` JSON).
    pub fn engine_config(&self, kind: EngineKind) -> EngineConfig {
        self.store
            .get_kv(&format!("engines.{kind}"))
            .ok()
            .flatten()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn set_engine_config(&self, kind: EngineKind, cfg: &EngineConfig) -> anyhow::Result<()> {
        self.store
            .set_kv(&format!("engines.{kind}"), &serde_json::to_string(cfg)?)?;
        Ok(())
    }

    pub async fn engine_statuses(&self) -> Vec<EngineStatus> {
        let mut out = Vec::new();
        for kind in EngineKind::ALL {
            let cfg = self.engine_config(kind);
            out.push(self.driver_for(kind).probe(&cfg).await);
        }
        out
    }
}
