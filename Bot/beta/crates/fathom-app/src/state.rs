//! Shared app state — the in-process `AppState` (store + bus + sessions) plus
//! UI-side caches updated from `ServerEvent`s.

use fathom_core::types::*;
use fathom_harness::AppState;
use std::collections::HashMap;
use std::sync::Arc;

/// What the chat pane is showing: a bot DM or a room.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatTarget {
    Bot(String),
    Room(String),
}

/// Root data holder — one entity; views read through it.
pub struct AppData {
    pub harness: Arc<AppState>,
    pub tokio: tokio::runtime::Handle,
    pub bots: Vec<Bot>,
    pub archived: Vec<Bot>,
    pub rooms: Vec<Room>,
    /// thread_id → loaded messages
    pub messages: HashMap<String, Vec<Message>>,
    /// bot_id → currently selected task thread id
    pub threads: HashMap<String, String>,
    /// bot_id → that bot's task threads (lazy-loaded)
    pub bot_threads: HashMap<String, Vec<Thread>>,
    /// thread_id → thread record (titles/pins)
    pub thread_meta: HashMap<String, Thread>,
    pub engines: Vec<EngineStatus>,
    pub approvals: Vec<ApprovalRequest>,
    /// The roster entry currently open in the chat pane.
    pub active: Option<ChatTarget>,
    /// Active modal dialog.
    pub dialog: Option<Dialog>,
    /// Fresh-data flag: set by the SSE/broadcast pump, consumed per render.
    pub dirty: bool,
}

#[derive(Debug, Clone)]
pub enum Dialog {
    NewBot,
    NewRoom,
    Settings,
    /// Edit a bot's persona/model. Field is bot id.
    BotProfile(String),
    /// Computer panel for a bot.
    Computer(String),
    /// Archived bots panel.
    Archived,
    /// Global search.
    Search,
    /// Attach a file by path to the composer.
    Attach,
    /// Rename a thread (task). Field is thread id.
    RenameThread(String),
}

impl AppData {
    pub fn new(harness: Arc<AppState>, tokio: tokio::runtime::Handle) -> Self {
        Self {
            harness,
            tokio,
            bots: vec![],
            archived: vec![],
            rooms: vec![],
            messages: HashMap::new(),
            threads: HashMap::new(),
            bot_threads: HashMap::new(),
            thread_meta: HashMap::new(),
            engines: vec![],
            approvals: vec![],
            active: None,
            dialog: None,
            dirty: false,
        }
    }

    pub fn bot(&self, id: &str) -> Option<&Bot> {
        self.bots.iter().find(|b| b.id == id)
    }

    pub fn room(&self, id: &str) -> Option<&Room> {
        self.rooms.iter().find(|r| r.id == id)
    }

    pub fn active_thread(&self) -> Option<String> {
        match self.active.as_ref()? {
            ChatTarget::Bot(b) => self.threads.get(b).cloned(),
            ChatTarget::Room(r) => self.room(r).map(|r| r.thread_id.clone()),
        }
    }

    /// Pending approvals for the active thread's bot.
    #[allow(dead_code)]
    pub fn active_approvals(&self) -> Vec<&ApprovalRequest> {
        let Some(tid) = self.active_thread() else {
            return vec![];
        };
        self.approvals
            .iter()
            .filter(|a| a.thread_id == tid)
            .collect()
    }

    /// Refresh roster + engines from the store (call after connecting or on
    /// coarse invalidations).
    pub fn reload(&mut self) {
        self.bots = self.harness.store.list_bots().unwrap_or_default();
        self.archived = self
            .harness
            .store
            .list_bots_filtered(true)
            .unwrap_or_default();
        self.rooms = self.harness.store.list_rooms().unwrap_or_default();
        for b in &self.bots {
            if !self.threads.contains_key(&b.id) {
                if let Ok(t) = self.harness.store.direct_thread(&b.id) {
                    self.threads.insert(b.id.clone(), t.id.clone());
                    self.thread_meta.insert(t.id.clone(), t);
                }
            }
        }
        self.approvals = self
            .harness
            .store
            .pending_approvals(None)
            .unwrap_or_default();
    }

    /// (Re)load the task list for a bot from the store.
    pub fn reload_threads(&mut self, bot_id: &str) {
        if let Ok(ts) = self.harness.store.bot_threads(bot_id) {
            for t in &ts {
                self.thread_meta.insert(t.id.clone(), t.clone());
            }
            self.bot_threads.insert(bot_id.to_string(), ts);
        }
    }

    /// Load a thread's messages into the cache.
    pub fn load_thread(&mut self, thread_id: &str) {
        if let Ok(msgs) = self.harness.store.list_messages(thread_id, 200, None) {
            self.messages.insert(thread_id.to_string(), msgs);
        }
        if let Ok(Some(t)) = self.harness.store.get_thread(thread_id) {
            self.thread_meta.insert(t.id.clone(), t);
        }
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
            ServerEvent::MessageDeleted { id, thread_id } => {
                if let Some(entry) = self.messages.get_mut(thread_id) {
                    entry.retain(|m| &m.id != id);
                }
            }
            ServerEvent::BotUpsert { bot } => {
                if bot.archived || bot.hidden {
                    match self.archived.iter_mut().find(|b| b.id == bot.id) {
                        Some(b) => *b = bot.clone(),
                        None => self.archived.push(bot.clone()),
                    }
                    self.bots.retain(|b| b.id != bot.id);
                } else {
                    match self.bots.iter_mut().find(|b| b.id == bot.id) {
                        Some(b) => *b = bot.clone(),
                        None => self.bots.push(bot.clone()),
                    }
                    self.archived.retain(|b| b.id != bot.id);
                    // pinned first, then recent activity.
                    self.bots.sort_by(|a, b| {
                        b.pinned.cmp(&a.pinned).then(
                            b.last_activity_at
                                .unwrap_or(b.created_at)
                                .cmp(&a.last_activity_at.unwrap_or(a.created_at)),
                        )
                    });
                }
            }
            ServerEvent::RoomUpsert { room } => {
                match self.rooms.iter_mut().find(|r| r.id == room.id) {
                    Some(r) => *r = room.clone(),
                    None => self.rooms.push(room.clone()),
                }
            }
            ServerEvent::ThreadUpsert { thread } => {
                self.thread_meta.insert(thread.id.clone(), thread.clone());
                if let Some(bid) = &thread.bot_id {
                    self.reload_threads(bid);
                }
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
