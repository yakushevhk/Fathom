//! Shared app state — the in-process `AppState` (store + bus + sessions) plus
//! UI-side caches updated from `ServerEvent`s.

use fathom_core::types::*;
use fathom_harness::AppState;
use std::collections::HashMap;
use std::sync::Arc;

/// Root data holder — one entity; views read through it.
pub struct AppData {
    pub harness: Arc<AppState>,
    pub tokio: tokio::runtime::Handle,
    pub bots: Vec<Bot>,
    /// thread_id → loaded messages
    pub messages: HashMap<String, Vec<Message>>,
    /// bot_id → direct thread id
    pub threads: HashMap<String, String>,
    pub engines: Vec<EngineStatus>,
    pub approvals: Vec<ApprovalRequest>,
    /// The roster entry currently open in the chat pane.
    pub active_bot: Option<String>,
    /// Active modal dialog.
    pub dialog: Option<Dialog>,
    /// Fresh-data flag: set by the SSE/broadcast pump, consumed per render.
    pub dirty: bool,
}

#[derive(Debug, Clone)]
pub enum Dialog {
    NewBot,
    Settings,
    /// Edit a bot's persona/model. Field is bot id.
    BotProfile(String),
    /// Computer panel for a bot.
    Computer(String),
}

impl AppData {
    pub fn new(harness: Arc<AppState>, tokio: tokio::runtime::Handle) -> Self {
        Self {
            harness,
            tokio,
            bots: vec![],
            messages: HashMap::new(),
            threads: HashMap::new(),
            engines: vec![],
            approvals: vec![],
            active_bot: None,
            dialog: None,
            dirty: false,
        }
    }

    pub fn bot(&self, id: &str) -> Option<&Bot> {
        self.bots.iter().find(|b| b.id == id)
    }

    pub fn active_thread(&self) -> Option<String> {
        self.active_bot
            .as_ref()
            .and_then(|b| self.threads.get(b).cloned())
    }

    /// Refresh roster + engines from the store (call after connecting or on
    /// coarse invalidations).
    pub fn reload(&mut self) {
        self.bots = self.harness.store.list_bots().unwrap_or_default();
        for b in &self.bots {
            if !self.threads.contains_key(&b.id) {
                if let Ok(t) = self.harness.store.direct_thread(&b.id) {
                    self.threads.insert(b.id.clone(), t.id);
                }
            }
        }
        self.approvals = self
            .harness
            .store
            .pending_approvals(None)
            .unwrap_or_default();
    }

    /// Apply one harness event.
    pub fn apply(&mut self, ev: &ServerEvent) {
        match ev {
            ServerEvent::MessageUpsert { message } => {
                let entry = self.messages.entry(message.thread_id.clone()).or_default();
                match entry.iter_mut().find(|m| m.id == message.id) {
                    Some(m) => *m = message.clone(),
                    None => entry.push(message.clone()),
                }
                entry.sort_by_key(|m| m.created_at);
            }
            ServerEvent::BotUpsert { bot } => {
                match self.bots.iter_mut().find(|b| b.id == bot.id) {
                    Some(b) => *b = bot.clone(),
                    None => self.bots.push(bot.clone()),
                }
                self.bots.retain(|b| !b.archived);
            }
            ServerEvent::ApprovalUpsert { approval } => {
                match self.approvals.iter_mut().find(|a| a.id == approval.id) {
                    Some(a) => *a = approval.clone(),
                    None => self.approvals.push(approval.clone()),
                }
                self.approvals
                    .retain(|a| a.status == ApprovalStatus::Pending);
            }
            ServerEvent::Engines { engines } => {
                self.engines = engines.clone();
            }
            _ => {}
        }
        self.dirty = true;
    }
}
