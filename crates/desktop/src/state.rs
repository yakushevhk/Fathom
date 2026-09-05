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

    /// Apply an incoming AgentEvent from the server SSE stream to reactive local UI state.
    pub fn apply_agent_event(&self, event: &pr_core::AgentEvent) {
        let now = chrono::Utc::now().format("%H:%M:%S").to_string();
        match event {
            pr_core::AgentEvent::SessionStarted { id, query } => {
                *self.active_session_id.write() = Some(id.0.clone());
                self.add_message(ChatMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    role: "system".to_string(),
                    content: format!("Session started: {}", query),
                    thinking: None,
                    tool_name: None,
                    tool_status: None,
                    tool_input: None,
                    tool_output: None,
                    question: None,
                    request_id: None,
                    timestamp: now,
                    expanded: false,
                });
            }
            pr_core::AgentEvent::ThinkingChunk { chunk, .. } => {
                let mut msgs = self.messages.write();
                if let Some(last) = msgs.iter_mut().rev().find(|m| m.role == "assistant") {
                    let t = last.thinking.get_or_insert_with(String::new);
                    t.push_str(chunk);
                } else {
                    msgs.push(ChatMessage {
                        id: uuid::Uuid::new_v4().to_string(),
                        role: "assistant".to_string(),
                        content: String::new(),
                        thinking: Some(chunk.clone()),
                        tool_name: None,
                        tool_status: None,
                        tool_input: None,
                        tool_output: None,
                        question: None,
                        request_id: None,
                        timestamp: now,
                        expanded: false,
                    });
                }
            }
            pr_core::AgentEvent::LlmStreamChunk { chunk, .. } => {
                let mut msgs = self.messages.write();
                if let Some(last) = msgs.iter_mut().rev().find(|m| m.role == "assistant" && m.tool_name.is_none()) {
                    last.content.push_str(chunk);
                } else {
                    msgs.push(ChatMessage {
                        id: uuid::Uuid::new_v4().to_string(),
                        role: "assistant".to_string(),
                        content: chunk.clone(),
                        thinking: None,
                        tool_name: None,
                        tool_status: None,
                        tool_input: None,
                        tool_output: None,
                        question: None,
                        request_id: None,
                        timestamp: now,
                        expanded: false,
                    });
                }
            }
            pr_core::AgentEvent::ToolCallStarted { tool, args, .. } => {
                self.add_message(ChatMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    role: "tool".to_string(),
                    content: format!("Running tool {}", tool),
                    thinking: None,
                    tool_name: Some(tool.clone()),
                    tool_status: Some("running".to_string()),
                    tool_input: Some(args.clone()),
                    tool_output: None,
                    question: None,
                    request_id: None,
                    timestamp: now.clone(),
                    expanded: false,
                });

                // If computer tool, record computer activity
                if tool.starts_with("computer_") {
                    self.computer_activities.write().push(ComputerActivity {
                        id: uuid::Uuid::new_v4().to_string(),
                        tool_name: tool.clone(),
                        intent: format!("Executing {}", tool),
                        target: args.get("url").or_else(|| args.get("ref")).and_then(|v| v.as_str()).unwrap_or("viewport").to_string(),
                        status: "running".to_string(),
                        timestamp: now,
                    });
                }
            }
            pr_core::AgentEvent::ToolCallCompleted { tool, result_preview, .. } => {
                let mut msgs = self.messages.write();
                if let Some(last_tool) = msgs.iter_mut().rev().find(|m| m.tool_name.as_deref() == Some(tool)) {
                    last_tool.tool_status = Some("completed".to_string());
                    last_tool.tool_output = Some(serde_json::Value::String(result_preview.clone()));
                }
            }
            pr_core::AgentEvent::QuestionAsked { request_id, question, .. } => {
                self.add_message(ChatMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    role: "assistant".to_string(),
                    content: question.clone(),
                    thinking: None,
                    tool_name: None,
                    tool_status: None,
                    tool_input: None,
                    tool_output: None,
                    question: Some(question.clone()),
                    request_id: Some(request_id.clone()),
                    timestamp: now,
                    expanded: false,
                });
            }
            pr_core::AgentEvent::ApprovalRequested { request_id, tool, args_preview, .. } => {
                self.add_message(ChatMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    role: "assistant".to_string(),
                    content: format!("Permission required to execute `{}`: {}", tool, args_preview),
                    thinking: None,
                    tool_name: Some(tool.clone()),
                    tool_status: Some("running".to_string()),
                    tool_input: Some(serde_json::Value::String(args_preview.clone())),
                    tool_output: None,
                    question: None,
                    request_id: Some(request_id.clone()),
                    timestamp: now,
                    expanded: true,
                });
            }
            pr_core::AgentEvent::SessionCompleted { output_dir, total_tokens, total_agents, .. } => {
                self.add_message(ChatMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    role: "system".to_string(),
                    content: format!("✓ Session completed. Tokens: {}, Agents: {}, Artifacts: {}", total_tokens, total_agents, output_dir),
                    thinking: None,
                    tool_name: None,
                    tool_status: None,
                    tool_input: None,
                    tool_output: None,
                    question: None,
                    request_id: None,
                    timestamp: now,
                    expanded: false,
                });
            }
            pr_core::AgentEvent::SessionFailed { error, .. } => {
                self.add_message(ChatMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    role: "system".to_string(),
                    content: format!("✗ Session failed: {}", error),
                    thinking: None,
                    tool_name: None,
                    tool_status: None,
                    tool_input: None,
                    tool_output: None,
                    question: None,
                    request_id: None,
                    timestamp: now,
                    expanded: false,
                });
            }
            _ => {}
        }
    }
}
