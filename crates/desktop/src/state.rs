//! Desktop application state management and data models.

use crate::api::{
    ApiClient, AuditEntry, Channel, Coworker, Credential,
    PolicyDocument, Routine, SessionSummary, Skill,
};
use crate::daemon::DaemonManager;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationTab {
    Channels,
    Computer,
    Governance,
    Routines,
    Skills,
    Vault,
    Settings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: String, // "user" | "assistant" | "system" | "tool"
    pub content: String,
    pub thinking: Option<String>,
    pub tool_name: Option<String>,
    pub tool_status: Option<String>, // "running" | "completed" | "failed"
    pub tool_input: Option<serde_json::Value>,
    pub tool_output: Option<serde_json::Value>,
    pub question: Option<String>,
    pub request_id: Option<String>,
    pub timestamp: String,
    pub expanded: bool,
}

#[derive(Debug, Clone)]
pub struct ComputerActivity {
    pub id: String,
    pub tool_name: String,
    pub intent: String,
    pub target: String,
    pub status: String,
    pub timestamp: String,
}

pub struct AppState {
    pub daemon: Arc<DaemonManager>,
    pub api: Arc<ApiClient>,
    pub active_tab: RwLock<NavigationTab>,
    pub is_engine_running: RwLock<bool>,
    pub sessions: RwLock<Vec<SessionSummary>>,
    pub active_session_id: RwLock<Option<String>>,
    pub coworkers: RwLock<Vec<Coworker>>,
    pub channels: RwLock<Vec<Channel>>,
    pub active_channel_id: RwLock<Option<String>>,
    pub messages: RwLock<Vec<ChatMessage>>,
    pub thinking_drawer_open: RwLock<bool>,
    // Computer Use
    pub computer_url: RwLock<String>,
    pub computer_screenshot: RwLock<Option<String>>,
    pub computer_human_control: RwLock<bool>,
    pub computer_activities: RwLock<Vec<ComputerActivity>>,
    // Governance
    pub policy: RwLock<Option<PolicyDocument>>,
    pub audit_log: RwLock<Vec<AuditEntry>>,
    pub audit_filter: RwLock<String>,
    // Routines & Skills & Vault
    pub routines: RwLock<Vec<Routine>>,
    pub skills: RwLock<Vec<Skill>>,
    pub credentials: RwLock<Vec<Credential>>,
    // Search / Filter
    pub search_query: RwLock<String>,
}

impl AppState {
    pub fn new() -> Self {
        let daemon = Arc::new(DaemonManager::new(8080));
        let api = Arc::new(ApiClient::new(daemon.url()));

        Self {
            daemon,
            api,
            active_tab: RwLock::new(NavigationTab::Channels),
            is_engine_running: RwLock::new(false),
            sessions: RwLock::new(Vec::new()),
            active_session_id: RwLock::new(None),
            coworkers: RwLock::new(Vec::new()),
            channels: RwLock::new(Vec::new()),
            active_channel_id: RwLock::new(None),
            messages: RwLock::new(Vec::new()),
            thinking_drawer_open: RwLock::new(false),
            computer_url: RwLock::new("https://github.com".to_string()),
            computer_screenshot: RwLock::new(None),
            computer_human_control: RwLock::new(false),
            computer_activities: RwLock::new(Vec::new()),
            policy: RwLock::new(None),
            audit_log: RwLock::new(Vec::new()),
            audit_filter: RwLock::new("all".to_string()),
            routines: RwLock::new(Vec::new()),
            skills: RwLock::new(Vec::new()),
            credentials: RwLock::new(Vec::new()),
            search_query: RwLock::new(String::new()),
        }
    }

    pub fn set_active_tab(&self, tab: NavigationTab) {
        *self.active_tab.write() = tab;
    }

    pub fn active_tab(&self) -> NavigationTab {
        *self.active_tab.read()
    }

    pub fn set_active_channel(&self, channel_id: Option<String>) {
        *self.active_channel_id.write() = channel_id;
    }

    pub fn toggle_thinking_drawer(&self) {
        let mut val = self.thinking_drawer_open.write();
        *val = !*val;
    }

    pub fn add_message(&self, msg: ChatMessage) {
        self.messages.write().push(msg);
    }
}
