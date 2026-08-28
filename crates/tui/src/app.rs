use pr_core::{AgentEvent, AgentId, AgentState, SessionId};
use std::collections::HashMap;
use crossterm::event::{KeyCode, KeyModifiers};
use crate::streaming::StreamingBuffer;

pub struct App {
    pub should_quit: bool,
    pub session_id: Option<SessionId>,
    pub query: String,
    pub input: String,
    pub input_cursor: usize,
    pub input_mode: InputMode,
    pub agents: HashMap<AgentId, AgentInfo>,
    pub event_log: Vec<EventLogEntry>,
    pub total_tokens: u64,
    pub context_window: u64,
    pub total_agents: u32,
    pub start_time: std::time::Instant,
    pub scroll_offset: u16,
    pub selected_panel: Panel,
    /// Input history for up/down arrow navigation
    pub input_history: Vec<String>,
    pub history_index: Option<usize>,
    /// Current input being edited (saved when navigating history)
    pub input_snapshot: String,
    /// Mid-run steering channel to the active session (fleet E1).
    pub steer_tx: Option<tokio::sync::mpsc::UnboundedSender<String>>,
    /// Thinking/reasoning content per agent
    pub thinking: HashMap<AgentId, ThinkingState>,
    /// Collapse state for thinking panel
    pub thinking_collapsed: bool,
    /// Timestamp of last thinking activity (for auto-hide)
    pub last_thinking_time: Option<std::time::Instant>,
    /// Streaming buffers per agent
    pub streams: HashMap<AgentId, StreamingBuffer>,
    /// Output text (final assembled output)
    pub output_text: String,
    /// Paste buffer for bracketed paste
    pub paste_buffer: String,
    /// Whether we are inside a bracketed paste sequence
    pub in_paste: bool,
    /// Tool calls with timing info
    pub tool_calls: Vec<ToolCallEntry>,
    /// Currently active tool calls (agent_id -> tool name)
    pub active_tools: HashMap<AgentId, String>,
    /// Durable background jobs (newest first), refreshed by the host loop.
    pub jobs: Vec<pr_persistence::JobRow>,
    /// Long-term semantic memory store (when `[memory]` is enabled); the
    /// panel shows `memory_snapshot`, refreshed by the host loop.
    pub memory: Option<std::sync::Arc<pr_memory::Memory>>,
    pub memory_snapshot: MemorySnapshot,
    /// Operator control plane state: a question awaiting the user's answer.
    pub pending_question: Option<PendingQuestion>,
    /// A side-effect tool call awaiting approval (y/n).
    pub pending_approval: Option<PendingApproval>,
    /// Collapsed nodes of the agent tree (Left/Right keys).
    pub collapsed: std::collections::HashSet<AgentId>,
    /// Cursor row in the agents panel (Up/Down while the panel is active).
    pub agents_cursor: usize,
    /// Time series of total_tokens for the header sparkline (oldest first).
    pub token_history: Vec<u64>,
    /// Help overlay with the keymap (`?` toggles).
    pub show_help: bool,
    /// Replay mode (`tui --replay`): the UI shows a stored session instead
    /// of a live run.
    pub replay_mode: bool,
    /// Active dialog/modal (None = no dialog)
    pub dialog: Option<Dialog>,
    /// File reference candidates for @file autocomplete
    pub file_refs: Vec<String>,
    /// Whether we are in file reference mode (after typing @)
    pub in_file_ref: bool,
    /// Current file reference query (text after @)
    pub file_ref_query: String,
    /// Selected file reference index
    pub file_ref_selected: usize,
    /// Session history for the session browser dialog
    pub session_list: Vec<pr_persistence::SessionSummary>,
    /// Mouse position (for hover detection)
    pub mouse_pos: (u16, u16),
}

/// A `question` tool round-trip waiting for the user's typed answer.
pub struct PendingQuestion {
    pub request_id: String,
    pub agent_id: AgentId,
    pub question: String,
    pub reply: tokio::sync::oneshot::Sender<String>,
}

/// An approval gate waiting for y/n.
pub struct PendingApproval {
    pub request_id: String,
    pub agent_id: AgentId,
    pub tool: String,
    pub args_preview: String,
    pub reply: tokio::sync::oneshot::Sender<bool>,
}

/// Periodically refreshed view of the long-term memory store (sync reads —
/// rusqlite is fast enough to run on the UI loop without jank).
#[derive(Default)]
pub struct MemorySnapshot {
    pub agent_active: usize,
    pub user_active: usize,
    pub run_active: usize,
    pub entity_nodes: i64,
    pub entity_edges: i64,
    /// Newest active memories across persistent scopes.
    pub recent: Vec<MemoryLine>,
    pub refreshed: bool,
}

pub struct MemoryLine {
    pub id: String,
    pub scope: String,
    pub content: String,
}

impl MemorySnapshot {
    /// Reload counts + the newest memories from the store.
    pub fn refresh(mem: &pr_memory::Memory) -> Self {
        let mut snap = MemorySnapshot {
            refreshed: true,
            ..Default::default()
        };
        let count = |scope: pr_memory::Scope| {
            let filter = pr_memory::ScopeFilter::new().add(scope, "");
            mem.db
                .list(&filter, Some("active"), usize::MAX)
                .map(|v| v.len())
                .unwrap_or(0)
        };
        snap.agent_active = count(pr_memory::Scope::Agent);
        snap.user_active = count(pr_memory::Scope::User);
        snap.run_active = count(pr_memory::Scope::Run);
        if let Ok((n, e)) = mem.db.count_entities() {
            snap.entity_nodes = n;
            snap.entity_edges = e;
        }
        if let Ok(rows) = mem.db.list(&pr_memory::ScopeFilter::persistent(), Some("active"), 15) {
            snap.recent = rows
                .into_iter()
                .map(|r| MemoryLine {
                    id: r.id.chars().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect(),
                    scope: r.scope,
                    content: r.content,
                })
                .collect();
        }
        snap
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputMode {
    Normal,
    Insert,
    Paste,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Panel {
    Agents,
    Output,
    Log,
    Jobs,
    Memory,
    Input,
}

/// Modal dialog types for the TUI.
#[derive(Debug, Clone, PartialEq)]
pub enum Dialog {
    /// Help overlay (keymap reference)
    Help,
    /// Session browser (list/search past sessions)
    SessionBrowser,
    /// Confirm action (y/n)
    Confirm(String),
    /// File reference picker
    FilePicker,
}

pub struct AgentInfo {
    pub id: AgentId,
    pub parent: Option<AgentId>,
    pub role: String,
    pub task: String,
    pub state: AgentState,
    pub tokens: u64,
    pub depth: u32,
    pub tool_calls: Vec<String>,
    pub start_time: std::time::Instant,
}

pub struct EventLogEntry {
    pub time: chrono::DateTime<chrono::Local>,
    pub message: String,
    pub level: LogLevel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogLevel {
    Info,
    Success,
    Error,
    Tool,
}

pub struct ThinkingState {
    pub content: String,
    pub last_update: std::time::Instant,
}

pub struct ToolCallEntry {
    pub agent_id: AgentId,
    pub tool: String,
    pub start_time: std::time::Instant,
    pub duration_ms: Option<u64>,
    pub result_preview: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            session_id: None,
            steer_tx: None,
            query: String::new(),
            input: String::new(),
            input_cursor: 0,
            input_mode: InputMode::Normal,
            agents: HashMap::new(),
            event_log: Vec::new(),
            total_tokens: 0,
            context_window: 128_000, // default context window
            total_agents: 0,
            start_time: std::time::Instant::now(),
            scroll_offset: 0,
            selected_panel: Panel::Input,
            input_history: Vec::new(),
            history_index: None,
            input_snapshot: String::new(),
            thinking: HashMap::new(),
            thinking_collapsed: false,
            last_thinking_time: None,
            streams: HashMap::new(),
            output_text: String::new(),
            paste_buffer: String::new(),
            in_paste: false,
            tool_calls: Vec::new(),
            active_tools: HashMap::new(),
            jobs: Vec::new(),
            memory: None,
            memory_snapshot: MemorySnapshot::default(),
            pending_question: None,
            pending_approval: None,
            collapsed: std::collections::HashSet::new(),
            agents_cursor: 0,
            token_history: Vec::new(),
            show_help: false,
            replay_mode: false,
            dialog: None,
            file_refs: Vec::new(),
            in_file_ref: false,
            file_ref_query: String::new(),
            file_ref_selected: 0,
            session_list: Vec::new(),
            mouse_pos: (0, 0),
        }
    }

    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        // Handle bracketed paste start/end markers
        if self.input_mode == InputMode::Paste {
            self.handle_paste_key(key);
            return;
        }

        // Handle dialog-specific keys first
        if let Some(ref dialog) = self.dialog.clone() {
            self.handle_dialog_key(key, dialog);
            return;
        }

        // Handle file reference mode
        if self.in_file_ref {
            self.handle_file_ref_key(key);
            return;
        }

        match self.input_mode {
            InputMode::Normal => match key.code {
                KeyCode::Char('q') => self.should_quit = true,
                KeyCode::Char('i') => self.input_mode = InputMode::Insert,
                KeyCode::Char('?') => {
                    if self.dialog == Some(Dialog::Help) {
                        self.dialog = None;
                    } else {
                        self.dialog = Some(Dialog::Help);
                    }
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.should_quit = true;
                }
                KeyCode::Esc => self.selected_panel = Panel::Input,
                KeyCode::Tab => {
                    self.selected_panel = match self.selected_panel {
                        Panel::Agents => Panel::Output,
                        Panel::Output => Panel::Log,
                        Panel::Log => Panel::Jobs,
                        Panel::Jobs => Panel::Memory,
                        Panel::Memory => Panel::Input,
                        Panel::Input => Panel::Agents,
                    };
                }
                KeyCode::BackTab => {
                    self.selected_panel = match self.selected_panel {
                        Panel::Agents => Panel::Input,
                        Panel::Output => Panel::Agents,
                        Panel::Log => Panel::Output,
                        Panel::Jobs => Panel::Log,
                        Panel::Memory => Panel::Jobs,
                        Panel::Input => Panel::Memory,
                    };
                }
                KeyCode::Up => {
                    if self.selected_panel == Panel::Agents {
                        self.agents_cursor = self.agents_cursor.saturating_sub(1);
                    } else {
                        self.scroll_offset = self.scroll_offset.saturating_sub(1);
                    }
                }
                KeyCode::Down => {
                    if self.selected_panel == Panel::Agents {
                        let visible = self.visible_agents();
                        if !visible.is_empty() {
                            self.agents_cursor =
                                (self.agents_cursor + 1).min(visible.len() - 1);
                        }
                    } else {
                        self.scroll_offset = self.scroll_offset.saturating_add(1);
                    }
                }
                KeyCode::Left => {
                    // Collapse the agent under the cursor (Agents panel).
                    if self.selected_panel == Panel::Agents {
                        if let Some(id) = self.visible_agents().get(self.agents_cursor) {
                            self.collapsed.insert(id.clone());
                        }
                    }
                }
                KeyCode::Right => {
                    // Expand the agent under the cursor (Agents panel).
                    if self.selected_panel == Panel::Agents {
                        if let Some(id) = self.visible_agents().get(self.agents_cursor) {
                            self.collapsed.remove(id);
                        }
                    }
                }
                KeyCode::Char('t') => {
                    // Toggle thinking panel
                    self.thinking_collapsed = !self.thinking_collapsed;
                }
                KeyCode::Char('y') => {
                    // Approve the pending side-effect tool call.
                    if let Some(pa) = self.pending_approval.take() {
                        let _ = pa.reply.send(true);
                        self.event_log.push(EventLogEntry {
                            time: chrono::Local::now(),
                            message: format!("approved tool call '{}'", pa.tool),
                            level: LogLevel::Success,
                        });
                    }
                }
                KeyCode::Char('n') => {
                    // Deny the pending side-effect tool call.
                    if let Some(pa) = self.pending_approval.take() {
                        let _ = pa.reply.send(false);
                        self.event_log.push(EventLogEntry {
                            time: chrono::Local::now(),
                            message: format!("denied tool call '{}'", pa.tool),
                            level: LogLevel::Error,
                        });
                    }
                }
                KeyCode::Char('c') => {
                    // Clear output
                    self.output_text.clear();
                    for buf in self.streams.values_mut() {
                        buf.clear();
                    }
                }
                KeyCode::Char('b') => {
                    // Open session browser (would need db access in real impl)
                    self.dialog = Some(Dialog::SessionBrowser);
                }
                KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Undo last file change
                    self.event_log.push(EventLogEntry {
                        time: chrono::Local::now(),
                        message: "Undo requested (use `undo` tool in agent)".to_string(),
                        level: LogLevel::Info,
                    });
                }
                _ => {}
            },
            InputMode::Insert => match key.code {
                KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                }
                KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    // Shift+Enter: insert newline (multi-line input)
                    self.input.insert(self.input_cursor, '\n');
                    self.input_cursor += 1;
                }
                KeyCode::Enter => {
                    // Operator answer to a pending `question` takes priority
                    // over starting/steering a session.
                    if let Some(pq) = self.pending_question.take() {
                        let answer = if self.input.trim().is_empty() {
                            "(no answer)".to_string()
                        } else {
                            self.input.clone()
                        };
                        let _ = pq.reply.send(answer);
                        self.input.clear();
                        self.input_cursor = 0;
                        self.input_mode = InputMode::Normal;
                        self.event_log.push(EventLogEntry {
                            time: chrono::Local::now(),
                            message: "answer sent to the agent".to_string(),
                            level: LogLevel::Success,
                        });
                        return;
                    }
                    // Regular Enter: submit
                    if !self.input.trim().is_empty() {
                        self.query = self.input.clone();
                        self.input_history.push(self.input.clone());
                        self.input.clear();
                        self.input_cursor = 0;
                        self.history_index = None;
                        self.input_mode = InputMode::Normal;
                    }
                }
                KeyCode::Backspace => {
                    if self.input_cursor > 0 {
                        self.input_cursor -= 1;
                        self.input.remove(self.input_cursor);
                    }
                }
                KeyCode::Delete => {
                    if self.input_cursor < self.input.len() {
                        self.input.remove(self.input_cursor);
                    }
                }
                KeyCode::Left => {
                    self.input_cursor = self.input_cursor.saturating_sub(1);
                }
                KeyCode::Right => {
                    if self.input_cursor < self.input.len() {
                        self.input_cursor += 1;
                    }
                }
                KeyCode::Home => {
                    self.input_cursor = 0;
                }
                KeyCode::End => {
                    self.input_cursor = self.input.len();
                }
                KeyCode::Up => {
                    // History navigation: previous
                    self.navigate_history_up();
                }
                KeyCode::Down => {
                    // History navigation: next
                    self.navigate_history_down();
                }
                KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Ctrl+V: enter paste mode
                    self.input_mode = InputMode::Paste;
                    self.paste_buffer.clear();
                    self.in_paste = true;
                }
                KeyCode::Char('@') => {
                    // Start file reference mode
                    self.input.insert(self.input_cursor, '@');
                    self.input_cursor += 1;
                    self.in_file_ref = true;
                    self.file_ref_query.clear();
                    self.file_refs.clear();
                    self.file_ref_selected = 0;
                }
                KeyCode::Char(c) => {
                    self.input.insert(self.input_cursor, c);
                    self.input_cursor += 1;
                }
                _ => {}
            },
            InputMode::Paste => {
                // Should not reach here due to top-level check
            }
        }
    }

    fn handle_paste_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                // End paste: insert accumulated text
                let text = self.paste_buffer.clone();
                self.input.insert_str(self.input_cursor, &text);
                self.input_cursor += text.len();
                self.paste_buffer.clear();
                self.in_paste = false;
                self.input_mode = InputMode::Insert;
            }
            KeyCode::Char(c) => {
                self.paste_buffer.push(c);
            }
            _ => {}
        }
    }

    fn navigate_history_up(&mut self) {
        if self.input_history.is_empty() {
            return;
        }
        match self.history_index {
            None => {
                // Save current input before navigating
                self.input_snapshot = self.input.clone();
                self.history_index = Some(self.input_history.len() - 1);
                self.input = self.input_history[self.input_history.len() - 1].clone();
                self.input_cursor = self.input.len();
            }
            Some(idx) if idx > 0 => {
                self.history_index = Some(idx - 1);
                self.input = self.input_history[idx - 1].clone();
                self.input_cursor = self.input.len();
            }
            _ => {}
        }
    }

    fn navigate_history_down(&mut self) {
        match self.history_index {
            Some(idx) if idx < self.input_history.len() - 1 => {
                self.history_index = Some(idx + 1);
                self.input = self.input_history[idx + 1].clone();
                self.input_cursor = self.input.len();
            }
            Some(_) => {
                // Back to current input
                self.history_index = None;
                self.input = self.input_snapshot.clone();
                self.input_cursor = self.input.len();
            }
            None => {}
        }
    }

    // ── Dialog handling ─────────────────────────────────────────────────

    fn handle_dialog_key(&mut self, key: crossterm::event::KeyEvent, dialog: &Dialog) {
        match dialog {
            Dialog::Help | Dialog::Confirm(_) => {
                // Any key closes help/confirm
                self.dialog = None;
            }
            Dialog::SessionBrowser => {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => self.dialog = None,
                    KeyCode::Up => {
                        if self.file_ref_selected > 0 {
                            self.file_ref_selected -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if self.file_ref_selected < self.session_list.len().saturating_sub(1) {
                            self.file_ref_selected += 1;
                        }
                    }
                    KeyCode::Enter => {
                        // Select session (placeholder - would load session details)
                        self.dialog = None;
                    }
                    _ => {}
                }
            }
            Dialog::FilePicker => {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        self.dialog = None;
                        self.in_file_ref = false;
                    }
                    KeyCode::Up => {
                        if self.file_ref_selected > 0 {
                            self.file_ref_selected -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if self.file_ref_selected < self.file_refs.len().saturating_sub(1) {
                            self.file_ref_selected += 1;
                        }
                    }
                    KeyCode::Enter => {
                        // Insert selected file reference
                        if let Some(file) = self.file_refs.get(self.file_ref_selected).cloned() {
                            // Remove the @query from input
                            let at_pos = self.input[..self.input_cursor].rfind('@').unwrap_or(0);
                            self.input.drain(at_pos..self.input_cursor);
                            self.input_cursor = at_pos;
                            // Insert the file path
                            self.input.insert_str(self.input_cursor, &file);
                            self.input_cursor += file.len();
                        }
                        self.dialog = None;
                        self.in_file_ref = false;
                    }
                    KeyCode::Char(c) => {
                        self.file_ref_query.push(c);
                        self.update_file_refs();
                    }
                    KeyCode::Backspace => {
                        self.file_ref_query.pop();
                        if self.file_ref_query.is_empty() && !self.input.contains('@') {
                            self.dialog = None;
                            self.in_file_ref = false;
                        } else {
                            self.update_file_refs();
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // ── File reference handling ─────────────────────────────────────────

    fn handle_file_ref_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.in_file_ref = false;
                self.dialog = None;
            }
            KeyCode::Char(c) if c.is_alphanumeric() || c == '.' || c == '/' || c == '_' || c == '-' => {
                self.file_ref_query.push(c);
                self.update_file_refs();
                if self.file_refs.len() <= 1 {
                    // Auto-complete if only one match
                    if let Some(file) = self.file_refs.first().cloned() {
                        let at_pos = self.input[..self.input_cursor].rfind('@').unwrap_or(0);
                        self.input.drain(at_pos..self.input_cursor);
                        self.input_cursor = at_pos;
                        self.input.insert_str(self.input_cursor, &file);
                        self.input_cursor += file.len();
                        self.in_file_ref = false;
                    }
                } else {
                    self.dialog = Some(Dialog::FilePicker);
                }
            }
            KeyCode::Backspace => {
                self.file_ref_query.pop();
                if self.file_ref_query.is_empty() {
                    self.in_file_ref = false;
                    self.dialog = None;
                } else {
                    self.update_file_refs();
                }
            }
            KeyCode::Tab => {
                // Accept first suggestion
                if let Some(file) = self.file_refs.first().cloned() {
                    let at_pos = self.input[..self.input_cursor].rfind('@').unwrap_or(0);
                    self.input.drain(at_pos..self.input_cursor);
                    self.input_cursor = at_pos;
                    self.input.insert_str(self.input_cursor, &file);
                    self.input_cursor += file.len();
                    self.in_file_ref = false;
                    self.dialog = None;
                }
            }
            _ => {
                // Any other key exits file ref mode and passes through
                self.in_file_ref = false;
                self.dialog = None;
            }
        }
    }

    /// Update file reference candidates based on current query.
    fn update_file_refs(&mut self) {
        self.file_refs.clear();
        self.file_ref_selected = 0;

        let query = self.file_ref_query.to_lowercase();
        if query.is_empty() {
            return;
        }

        // Scan working directory for matching files
        if let Ok(entries) = std::fs::read_dir(".") {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.to_lowercase().contains(&query) {
                    let display = if path.is_dir() {
                        format!("{}/", name)
                    } else {
                        name.to_string()
                    };
                    self.file_refs.push(display);
                    if self.file_refs.len() >= 20 {
                        break;
                    }
                }
            }
        }
    }

    /// Handle mouse events (clicks, scroll).
    pub fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) {
        self.mouse_pos = (mouse.column, mouse.row);

        match mouse.kind {
            crossterm::event::MouseEventKind::ScrollUp => {
                self.scroll_offset = self.scroll_offset.saturating_add(3);
            }
            crossterm::event::MouseEventKind::ScrollDown => {
                self.scroll_offset = self.scroll_offset.saturating_sub(3);
            }
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                // Click on input area (bottom) -> enter insert mode
                // This is approximate - actual hit testing depends on layout
                if self.input_mode == InputMode::Normal {
                    self.input_mode = InputMode::Insert;
                    self.selected_panel = Panel::Input;
                }
            }
            _ => {}
        }
    }

    /// Open the session browser dialog.
    pub fn open_session_browser(&mut self, sessions: Vec<pr_persistence::SessionSummary>) {
        self.session_list = sessions;
        self.file_ref_selected = 0;
        self.dialog = Some(Dialog::SessionBrowser);
    }

    pub fn handle_agent_event(&mut self, event: AgentEvent) {
        let log_msg = match &event {
            AgentEvent::SessionStarted { id, query, .. } => {
                self.start_time = std::time::Instant::now();
                self.session_id = Some(id.clone());
                format!("Session started: {}", query.chars().take(50).collect::<String>())
            }
            AgentEvent::AgentSpawned { id, parent, role, task, depth, .. } => {
                self.total_agents += 1;
                self.agents.insert(id.clone(), AgentInfo {
                    id: id.clone(),
                    parent: parent.clone(),
                    role: role.clone(),
                    task: task.clone(),
                    state: AgentState::Idle,
                    tokens: 0,
                    depth: *depth,
                    tool_calls: Vec::new(),
                    start_time: std::time::Instant::now(),
                });
                self.streams.insert(id.clone(), StreamingBuffer::new());
                format!("Agent {} spawned ({}): {}", id, role, task.chars().take(40).collect::<String>())
            }
            AgentEvent::AgentStateChanged { id, state } => {
                if let Some(agent) = self.agents.get_mut(id) {
                    agent.state = state.clone();
                }
                format!("Agent {} state: {:?}", id, state)
            }
            AgentEvent::ToolCallStarted { agent_id, tool, .. } => {
                if let Some(agent) = self.agents.get_mut(agent_id) {
                    agent.tool_calls.push(format!("→ {tool}"));
                }
                self.active_tools.insert(agent_id.clone(), tool.clone());
                self.tool_calls.push(ToolCallEntry {
                    agent_id: agent_id.clone(),
                    tool: tool.clone(),
                    start_time: std::time::Instant::now(),
                    duration_ms: None,
                    result_preview: None,
                });
                format!("[{}] calling: {}", agent_id, tool)
            }
            AgentEvent::ToolCallCompleted { agent_id, tool, result_preview, duration_ms, .. } => {
                self.active_tools.remove(agent_id);
                // Update the last matching tool call entry
                if let Some(entry) = self.tool_calls.iter_mut().rev().find(|e| {
                    e.agent_id == *agent_id && e.tool == *tool && e.duration_ms.is_none()
                }) {
                    entry.duration_ms = Some(*duration_ms);
                    entry.result_preview = Some(result_preview.clone());
                }
                format!("✓ {} ({}ms)", tool, duration_ms)
            }
            AgentEvent::AgentCompleted { id, tokens_used, .. } => {
                self.total_tokens += tokens_used;
                if let Some(agent) = self.agents.get_mut(id) {
                    agent.tokens = *tokens_used;
                }
                // Flush streaming buffer
                if let Some(buf) = self.streams.get_mut(id) {
                    buf.flush();
                }
                self.active_tools.remove(id);
                self.sample_token_history();
                format!("Agent {} completed ({} tokens)", id, tokens_used)
            }
            AgentEvent::AgentFailed { id, error } => {
                if let Some(agent) = self.agents.get_mut(id) {
                    agent.state = AgentState::Error { message: error.clone() };
                }
                self.active_tools.remove(id);
                format!("Agent {} FAILED: {}", id, error.chars().take(50).collect::<String>())
            }
            AgentEvent::SessionCompleted { total_tokens, total_agents, .. } => {
                self.total_tokens = *total_tokens;
                self.total_agents = *total_agents;
                self.sample_token_history();
                format!("Session completed! {} agents, {} tokens", total_agents, total_tokens)
            }
            AgentEvent::ThinkingChunk { agent_id, chunk } => {
                self.handle_thinking_chunk(agent_id, chunk);
                return;
            }
            AgentEvent::LlmStreamChunk { agent_id, chunk } => {
                if let Some(buf) = self.streams.get_mut(agent_id) {
                    buf.push(chunk);
                }
                self.update_output_text();
                return;
            }
            AgentEvent::Finding { agent_id, finding } => {
                format!(
                    "Agent {} finding: {} (confidence {:.2})",
                    agent_id, finding.title, finding.confidence
                )
            }
            AgentEvent::SessionFailed { id, error } => {
                if self.session_id.as_ref() == Some(id) {
                    self.session_id = None;
                }
                format!("Session {} FAILED: {}", id, error.chars().take(60).collect::<String>())
            }
            AgentEvent::QuestionAsked { agent_id, question, .. } => {
                format!(
                    "❓ Agent {} asks: {} — type the answer and press Enter",
                    agent_id,
                    question.chars().take(80).collect::<String>()
                )
            }
            AgentEvent::ApprovalRequested { agent_id, tool, args_preview, .. } => {
                format!(
                    "🔐 Agent {} wants to run '{}' [{}] — press y to allow, n to deny",
                    agent_id,
                    tool,
                    args_preview.chars().take(60).collect::<String>()
                )
            }
            AgentEvent::SessionForked { parent_id, child_id, query } => {
                format!(
                    "🔀 Session forked: {} → {} ({})",
                    &parent_id.0[..8.min(parent_id.0.len())],
                    &child_id.0[..8.min(child_id.0.len())],
                    query.chars().take(40).collect::<String>()
                )
            }
            AgentEvent::FileChangeUndone { file_path, operation, .. } => {
                format!("↩ Undone {} on {}", operation, file_path)
            }
            AgentEvent::TitleGenerated { title, .. } => {
                format!("📌 Title: {}", title)
            }
        };

        let level = match &event {
            AgentEvent::AgentFailed { .. } | AgentEvent::SessionFailed { .. } => LogLevel::Error,
            AgentEvent::ToolCallStarted { .. } | AgentEvent::ToolCallCompleted { .. } => LogLevel::Tool,
            AgentEvent::AgentCompleted { .. } | AgentEvent::SessionCompleted { .. } => LogLevel::Success,
            AgentEvent::SessionForked { .. } | AgentEvent::TitleGenerated { .. } => LogLevel::Success,
            _ => LogLevel::Info,
        };

        self.event_log.push(EventLogEntry {
            time: chrono::Local::now(),
            message: log_msg,
            level,
        });

        // Keep only last 1000 entries
        if self.event_log.len() > 1000 {
            self.event_log.drain(0..100);
        }
    }

    /// Handle a thinking/reasoning chunk from the LLM
    pub fn handle_thinking_chunk(&mut self, agent_id: &AgentId, chunk: &str) {
        let state = self.thinking.entry(agent_id.clone()).or_insert_with(|| ThinkingState {
            content: String::new(),
            last_update: std::time::Instant::now(),
        });
        state.content.push_str(chunk);
        state.last_update = std::time::Instant::now();
        self.last_thinking_time = Some(std::time::Instant::now());
        if self.thinking_collapsed {
            self.thinking_collapsed = false;
        }
    }

    /// Check if thinking panel should auto-hide (30s since last activity)
    pub fn should_auto_hide_thinking(&self) -> bool {
        if self.thinking_collapsed {
            return true;
        }
        match self.last_thinking_time {
            Some(t) => t.elapsed().as_secs() > 30,
            None => true,
        }
    }

    /// Get assembled output text from all streams
    fn update_output_text(&mut self) {
        // Use the first agent's stream as the main output
        // In practice, the coordinator or writer agent's stream is the output
        let mut parts = Vec::new();
        for (id, buf) in &self.streams {
            if let Some(agent) = self.agents.get(id) {
                if agent.role == "writer" || agent.role == "coordinator" {
                    let text = buf.published_text();
                    if !text.is_empty() {
                        parts.push(text);
                    }
                }
            }
        }
        if !parts.is_empty() {
            self.output_text = parts.join("\n\n");
        }
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    /// Get agent elapsed time
    pub fn agent_elapsed(&self, id: &AgentId) -> std::time::Duration {
        self.agents
            .get(id)
            .map(|a| a.start_time.elapsed())
            .unwrap_or_default()
    }

    /// Count active (non-complete, non-error) agents
    pub fn active_agent_count(&self) -> u32 {
        self.agents.values().filter(|a| {
            !matches!(a.state, AgentState::Complete | AgentState::Error { .. })
        }).count() as u32
    }

    /// Count completed agents
    pub fn completed_agent_count(&self) -> u32 {
        self.agents.values().filter(|a| {
            matches!(a.state, AgentState::Complete)
        }).count() as u32
    }

    /// Token usage ratio (0.0..=1.0)
    pub fn token_usage_ratio(&self) -> f64 {
        if self.context_window == 0 {
            return 0.0;
        }
        (self.total_tokens as f64 / self.context_window as f64).min(1.0)
    }

    /// Whether the agent has children in the spawn tree.
    pub fn has_children(&self, id: &AgentId) -> bool {
        self.agents.values().any(|a| a.parent.as_ref() == Some(id))
    }

    /// DFS order of the agent tree, honouring collapsed nodes. Roots are
    /// agents without a parent (or whose parent is unknown, e.g. after a
    /// replay of partial data). Order is stable: spawn order per level.
    pub fn visible_agents(&self) -> Vec<AgentId> {
        let mut roots: Vec<&AgentInfo> = self
            .agents
            .values()
            .filter(|a| {
                a.parent
                    .as_ref()
                    .map(|p| !self.agents.contains_key(p))
                    .unwrap_or(true)
            })
            .collect();
        roots.sort_by(|a, b| a.start_time.cmp(&b.start_time).then_with(|| a.id.0.cmp(&b.id.0)));

        let children_of = |id: &AgentId| {
            let mut kids: Vec<&AgentInfo> = self
                .agents
                .values()
                .filter(|a| a.parent.as_ref() == Some(id))
                .collect();
            kids.sort_by(|a, b| {
                a.start_time.cmp(&b.start_time).then_with(|| a.id.0.cmp(&b.id.0))
            });
            kids
        };

        let mut out: Vec<AgentId> = Vec::with_capacity(self.agents.len());
        let mut stack: Vec<AgentId> = roots.iter().rev().map(|a| a.id.clone()).collect();
        while let Some(id) = stack.pop() {
            out.push(id.clone());
            if self.collapsed.contains(&id) {
                continue;
            }
            for kid in children_of(&id).into_iter().rev() {
                stack.push(kid.id.clone());
            }
        }
        out
    }

    /// Record a token-usage sample for the header sparkline (bounded).
    pub fn sample_token_history(&mut self) {
        const CAP: usize = 120;
        self.token_history.push(self.total_tokens);
        if self.token_history.len() > CAP {
            let drop = self.token_history.len() - CAP;
            self.token_history.drain(..drop);
        }
    }

    /// Populate the UI from a stored session (`tui --replay`). Agents are
    /// shown in their final recorded state; findings land in the log.
    pub fn load_replay(&mut self, details: &pr_persistence::SessionDetails) {
        self.replay_mode = true;
        self.session_id = Some(details.session.id.clone());
        self.query = details.session.query.clone();
        self.total_tokens = details.session.total_tokens.max(0) as u64;
        self.total_agents = details.session.total_agents.max(0) as u32;
        for row in &details.agents {
            let state = match row.status.as_str() {
                "completed" => AgentState::Complete,
                "failed" => AgentState::Error {
                    message: "failed".into(),
                },
                "cancelled" => AgentState::Error {
                    message: "cancelled".into(),
                },
                "running" => AgentState::Researching {
                    sub_tasks: Vec::new(),
                },
                _ => AgentState::Idle,
            };
            let id = AgentId(row.id.clone());
            self.agents.insert(
                id.clone(),
                AgentInfo {
                    id,
                    parent: row.parent_id.clone().map(AgentId),
                    role: row.role.clone(),
                    task: row.task.clone(),
                    state,
                    tokens: row.tokens_used.max(0) as u64,
                    depth: row.depth.max(0) as u32,
                    tool_calls: Vec::new(),
                    start_time: std::time::Instant::now(),
                },
            );
        }
        for f in &details.findings {
            self.event_log.push(EventLogEntry {
                time: chrono::Local::now(),
                message: format!("finding: {} (confidence {:.2})", f.title, f.confidence),
                level: LogLevel::Info,
            });
        }
        self.event_log.push(EventLogEntry {
            time: chrono::Local::now(),
            message: format!(
                "replay of session {} — \"{}\"",
                details.session.id,
                details.session.query.chars().take(60).collect::<String>()
            ),
            level: LogLevel::Success,
        });
        self.sample_token_history();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_new() {
        let app = App::new();
        assert!(!app.should_quit);
        assert!(app.session_id.is_none());
        assert!(app.input.is_empty());
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app.agents.is_empty());
        assert_eq!(app.total_tokens, 0);
        assert_eq!(app.context_window, 128_000);
        assert_eq!(app.selected_panel, Panel::Input);
    }

    #[test]
    fn test_input_insert_and_submit() {
        let mut app = App::new();

        // Enter insert mode
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Char('i'),
            KeyModifiers::NONE,
        ));
        assert_eq!(app.input_mode, InputMode::Insert);

        // Type characters
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Char('h'),
            KeyModifiers::NONE,
        ));
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Char('i'),
            KeyModifiers::NONE,
        ));
        assert_eq!(app.input, "hi");

        // Submit
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        ));
        assert_eq!(app.query, "hi");
        assert!(app.input.is_empty());
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_shift_enter_newline() {
        let mut app = App::new();
        app.input_mode = InputMode::Insert;

        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Char('a'),
            KeyModifiers::NONE,
        ));
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::SHIFT,
        ));
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Char('b'),
            KeyModifiers::NONE,
        ));

        assert_eq!(app.input, "a\nb");
    }

    #[test]
    fn test_input_history() {
        let mut app = App::new();
        app.input_mode = InputMode::Insert;

        // Type and submit first query
        app.input = "first query".to_string();
        app.input_cursor = app.input.len();
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        ));
        assert_eq!(app.input_history.len(), 1);

        // Enter insert mode again
        app.input_mode = InputMode::Insert;

        // Up arrow should recall history
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Up,
            KeyModifiers::NONE,
        ));
        assert_eq!(app.input, "first query");
    }

    #[test]
    fn test_tab_panel_cycling() {
        let mut app = App::new();
        assert_eq!(app.selected_panel, Panel::Input);

        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Tab,
            KeyModifiers::NONE,
        ));
        assert_eq!(app.selected_panel, Panel::Agents);

        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Tab,
            KeyModifiers::NONE,
        ));
        assert_eq!(app.selected_panel, Panel::Output);

        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Tab,
            KeyModifiers::NONE,
        ));
        assert_eq!(app.selected_panel, Panel::Log);

        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Tab,
            KeyModifiers::NONE,
        ));
        assert_eq!(app.selected_panel, Panel::Jobs);

        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Tab,
            KeyModifiers::NONE,
        ));
        assert_eq!(app.selected_panel, Panel::Memory);

        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Tab,
            KeyModifiers::NONE,
        ));
        assert_eq!(app.selected_panel, Panel::Input);
    }

    #[test]
    fn test_memory_snapshot_default_empty() {
        let app = App::new();
        assert!(app.memory.is_none());
        assert!(!app.memory_snapshot.refreshed);
        assert_eq!(app.memory_snapshot.agent_active, 0);
        assert!(app.memory_snapshot.recent.is_empty());
    }

    #[tokio::test]
    async fn test_memory_snapshot_refresh_reads_store() {
        let mem = pr_memory::Memory::in_memory(pr_core::MemoryConfig::default()).unwrap();
        mem.pipeline()
            .absorb(pr_memory::AbsorbRequest {
                facts: vec![pr_memory::AbsorbFact {
                    content: "the tui shows long term memory statistics in a side panel".into(),
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    confidence: Some(0.8),
                    memory_class: None,
                }],
                source: "unit".into(),
                scope: pr_memory::Scope::Agent,
                scope_key: String::new(),
                context: None,
                dry_run: false,
            })
            .await
            .unwrap();

        let snap = MemorySnapshot::refresh(&mem);
        assert!(snap.refreshed);
        assert_eq!(snap.agent_active, 1);
        assert_eq!(snap.user_active, 0);
        assert_eq!(snap.recent.len(), 1);
        assert!(snap.recent[0].content.contains("tui"));
        assert_eq!(snap.recent[0].scope, "agent");
    }

    #[test]
    fn test_token_usage_ratio() {
        let mut app = App::new();
        assert_eq!(app.token_usage_ratio(), 0.0);

        app.total_tokens = 64_000;
        assert!((app.token_usage_ratio() - 0.5).abs() < 0.001);

        app.total_tokens = 200_000;
        assert!((app.token_usage_ratio() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_active_agent_count() {
        let mut app = App::new();
        assert_eq!(app.active_agent_count(), 0);

        app.agents.insert(AgentId("a1".to_string()), AgentInfo {
            id: AgentId("a1".to_string()),
            parent: None,
            role: "coordinator".to_string(),
            task: "test".to_string(),
            state: AgentState::Researching { sub_tasks: vec![] },
            tokens: 0,
            depth: 0,
            tool_calls: Vec::new(),
            start_time: std::time::Instant::now(),
        });
        app.agents.insert(AgentId("a2".to_string()), AgentInfo {
            id: AgentId("a2".to_string()),
            parent: Some(AgentId("a1".to_string())),
            role: "researcher".to_string(),
            task: "test2".to_string(),
            state: AgentState::Complete,
            tokens: 100,
            depth: 1,
            tool_calls: Vec::new(),
            start_time: std::time::Instant::now(),
        });

        assert_eq!(app.active_agent_count(), 1);
        assert_eq!(app.completed_agent_count(), 1);
    }

    #[test]
    fn test_thinking_state() {
        let mut app = App::new();
        let agent_id = AgentId("a1".to_string());

        app.handle_thinking_chunk(&agent_id, "Let me think ");
        app.handle_thinking_chunk(&agent_id, "about this.");

        let thinking = app.thinking.get(&agent_id).unwrap();
        assert_eq!(thinking.content, "Let me think about this.");
    }

    #[test]
    fn test_quit_key() {
        let mut app = App::new();
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
        ));
        assert!(app.should_quit);
    }

    // ── Agent event wiring ─────────────────────────────────────────────

    fn spawn_event(id: &str, role: &str) -> AgentEvent {
        AgentEvent::AgentSpawned {
            id: AgentId(id.to_string()),
            parent: None,
            role: role.to_string(),
            task: "test task".to_string(),
            depth: 1,
        }
    }

    fn spawn_child_event(id: &str, parent: &str, depth: u32) -> AgentEvent {
        AgentEvent::AgentSpawned {
            id: AgentId(id.to_string()),
            parent: Some(AgentId(parent.to_string())),
            role: "researcher".to_string(),
            task: "child task".to_string(),
            depth,
        }
    }

    #[test]
    fn test_visible_agents_dfs_order() {
        let mut app = App::new();
        app.handle_agent_event(spawn_event("root", "coordinator"));
        app.handle_agent_event(spawn_child_event("c1", "root", 2));
        app.handle_agent_event(spawn_child_event("c2", "root", 2));
        app.handle_agent_event(spawn_child_event("gc1", "c1", 3));

        let visible = app.visible_agents();
        let ids: Vec<String> = visible.into_iter().map(|id| id.0).collect();
        assert_eq!(ids, vec!["root", "c1", "gc1", "c2"]);
        assert!(app.has_children(&AgentId("root".to_string())));
        assert!(app.has_children(&AgentId("c1".to_string())));
        assert!(!app.has_children(&AgentId("c2".to_string())));
    }

    #[test]
    fn test_collapsed_node_hides_subtree() {
        let mut app = App::new();
        app.handle_agent_event(spawn_event("root", "coordinator"));
        app.handle_agent_event(spawn_child_event("c1", "root", 2));
        app.handle_agent_event(spawn_child_event("gc1", "c1", 3));
        app.handle_agent_event(spawn_child_event("c2", "root", 2));

        app.collapsed.insert(AgentId("c1".to_string()));
        let ids: Vec<String> = app
            .visible_agents()
            .into_iter()
            .map(|id| id.0)
            .collect();
        assert_eq!(ids, vec!["root", "c1", "c2"], "gc1 hidden under collapsed c1");

        app.collapsed.remove(&AgentId("c1".to_string()));
        assert_eq!(app.visible_agents().len(), 4);
    }

    #[test]
    fn test_token_history_bounded_and_sampled() {
        let mut app = App::new();
        for _ in 0..200 {
            app.total_tokens += 100;
            app.sample_token_history();
        }
        assert_eq!(app.token_history.len(), 120);
        assert_eq!(*app.token_history.last().unwrap(), app.total_tokens);
    }

    #[test]
    fn test_agent_spawned_populates_tree() {
        let mut app = App::new();
        app.handle_agent_event(spawn_event("a1", "researcher"));

        assert_eq!(app.total_agents, 1);
        let agent = app.agents.get(&AgentId("a1".to_string())).unwrap();
        assert_eq!(agent.role, "researcher");
        assert_eq!(agent.task, "test task");
        assert_eq!(agent.depth, 1);
        assert!(app.streams.contains_key(&AgentId("a1".to_string())));
        assert_eq!(app.event_log.len(), 1);
    }

    #[test]
    fn test_tool_call_lifecycle_updates_panel() {
        let mut app = App::new();
        app.handle_agent_event(spawn_event("a1", "researcher"));

        app.handle_agent_event(AgentEvent::ToolCallStarted {
            agent_id: AgentId("a1".to_string()),
            tool: "web_search".to_string(),
            args: serde_json::json!({"query": "rust"}),
        });
        assert_eq!(app.active_tools.get(&AgentId("a1".to_string())).map(String::as_str), Some("web_search"));
        assert_eq!(app.tool_calls.len(), 1);
        assert!(app.tool_calls[0].duration_ms.is_none());

        app.handle_agent_event(AgentEvent::ToolCallCompleted {
            agent_id: AgentId("a1".to_string()),
            tool: "web_search".to_string(),
            result_preview: "results...".to_string(),
            duration_ms: 123,
        });
        assert!(app.active_tools.is_empty());
        assert_eq!(app.tool_calls[0].duration_ms, Some(123));
        assert_eq!(app.tool_calls[0].result_preview.as_deref(), Some("results..."));
        assert!(app.agents[&AgentId("a1".to_string())].tool_calls.iter().any(|t| t.contains("web_search")));
    }

    #[test]
    fn test_agent_completed_updates_tokens_and_state() {
        let mut app = App::new();
        app.handle_agent_event(spawn_event("a1", "researcher"));

        app.handle_agent_event(AgentEvent::AgentStateChanged {
            id: AgentId("a1".to_string()),
            state: AgentState::Complete,
        });
        assert!(matches!(
            app.agents[&AgentId("a1".to_string())].state,
            AgentState::Complete
        ));

        app.handle_agent_event(AgentEvent::AgentCompleted {
            id: AgentId("a1".to_string()),
            summary: "done".to_string(),
            tokens_used: 555,
        });
        assert_eq!(app.total_tokens, 555);
        assert_eq!(app.agents[&AgentId("a1".to_string())].tokens, 555);
    }

    #[test]
    fn test_llm_stream_chunk_feeds_output_for_writer() {
        let mut app = App::new();
        app.handle_agent_event(spawn_event("w1", "writer"));
        let log_before = app.event_log.len();

        app.handle_agent_event(AgentEvent::LlmStreamChunk {
            agent_id: AgentId("w1".to_string()),
            chunk: "Hello report\n".to_string(),
        });

        // Chunk lands in the streaming buffer and the assembled output.
        assert!(app.streams[&AgentId("w1".to_string())].published_text().contains("Hello report"));
        assert!(app.output_text.contains("Hello report"));
        // Stream chunks are not logged.
        assert_eq!(app.event_log.len(), log_before);
    }

    #[test]
    fn test_finding_event_is_logged() {
        let mut app = App::new();
        app.handle_agent_event(spawn_event("a1", "researcher"));
        let before = app.event_log.len();

        app.handle_agent_event(AgentEvent::Finding {
            agent_id: AgentId("a1".to_string()),
            finding: pr_core::Finding {
                id: pr_core::FindingId::new(),
                agent_id: AgentId("a1".to_string()),
                title: "Key fact".to_string(),
                content: "content".to_string(),
                sources: vec![],
                confidence: 0.9,
                created_at: chrono::Utc::now(),
            },
        });

        assert_eq!(app.event_log.len(), before + 1);
        assert!(app.event_log.last().unwrap().message.contains("Key fact"));
    }

    #[test]
    fn test_session_failed_is_logged_as_error() {
        let mut app = App::new();
        app.handle_agent_event(AgentEvent::SessionStarted {
            id: SessionId("s1".to_string()),
            query: "q".to_string(),
        });
        assert_eq!(app.session_id.as_ref().unwrap().0, "s1");

        app.handle_agent_event(AgentEvent::SessionFailed {
            id: SessionId("s1".to_string()),
            error: "boom".to_string(),
        });

        let entry = app.event_log.last().unwrap();
        assert_eq!(entry.level, LogLevel::Error);
        assert!(entry.message.contains("boom"));
        assert!(app.session_id.is_none());
    }

    #[test]
    fn test_agent_failed_marks_error_state() {
        let mut app = App::new();
        app.handle_agent_event(spawn_event("a1", "researcher"));

        app.handle_agent_event(AgentEvent::AgentFailed {
            id: AgentId("a1".to_string()),
            error: "LLM timeout".to_string(),
        });

        assert!(matches!(
            app.agents[&AgentId("a1".to_string())].state,
            AgentState::Error { .. }
        ));
        assert_eq!(app.event_log.last().unwrap().level, LogLevel::Error);
    }

    // ── Dialog system tests ─────────────────────────────────────────────

    #[test]
    fn test_dialog_help_toggle() {
        let mut app = App::new();
        assert!(app.dialog.is_none());

        // Open help
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Char('?'),
            KeyModifiers::NONE,
        ));
        assert_eq!(app.dialog, Some(Dialog::Help));

        // Close help
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Char('?'),
            KeyModifiers::NONE,
        ));
        assert!(app.dialog.is_none());
    }

    #[test]
    fn test_dialog_session_browser() {
        let mut app = App::new();

        // Open session browser
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Char('b'),
            KeyModifiers::NONE,
        ));
        assert_eq!(app.dialog, Some(Dialog::SessionBrowser));

        // Close with Esc
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        ));
        assert!(app.dialog.is_none());
    }

    #[test]
    fn test_dialog_confirm() {
        let mut app = App::new();
        app.dialog = Some(Dialog::Confirm("Are you sure?".to_string()));
        assert_eq!(app.dialog, Some(Dialog::Confirm("Are you sure?".to_string())));

        // Any key closes confirm
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Char('y'),
            KeyModifiers::NONE,
        ));
        assert!(app.dialog.is_none());
    }

    #[test]
    fn test_file_ref_mode_trigger() {
        let mut app = App::new();
        app.input_mode = InputMode::Insert;

        // Type '@' to enter file reference mode
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Char('@'),
            KeyModifiers::NONE,
        ));
        assert!(app.in_file_ref);
        assert_eq!(app.input, "@");
    }

    #[test]
    fn test_file_ref_mode_cancel() {
        let mut app = App::new();
        app.input_mode = InputMode::Insert;
        app.in_file_ref = true;
        app.file_ref_query = "test".to_string();

        // Esc cancels file reference mode
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        ));
        assert!(!app.in_file_ref);
    }

    #[test]
    fn test_mouse_scroll_up() {
        let mut app = App::new();
        app.scroll_offset = 5;

        app.handle_mouse(crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::ScrollUp,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.scroll_offset, 8); // 5 + 3
    }

    #[test]
    fn test_mouse_scroll_down() {
        let mut app = App::new();
        app.scroll_offset = 5;

        app.handle_mouse(crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::ScrollDown,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.scroll_offset, 2); // 5 - 3
    }

    #[test]
    fn test_session_forked_event() {
        let mut app = App::new();
        let parent = SessionId("parent123".to_string());
        let child = SessionId("child45678".to_string());

        app.handle_agent_event(AgentEvent::SessionForked {
            parent_id: parent,
            child_id: child,
            query: "extended research".to_string(),
        });

        let entry = app.event_log.last().unwrap();
        assert_eq!(entry.level, LogLevel::Success);
        assert!(entry.message.contains("forked"));
    }

    #[test]
    fn test_file_change_undone_event() {
        let mut app = App::new();

        app.handle_agent_event(AgentEvent::FileChangeUndone {
            session_id: SessionId("s1".to_string()),
            file_path: "/tmp/test.rs".to_string(),
            operation: "edit".to_string(),
        });

        let entry = app.event_log.last().unwrap();
        assert!(entry.message.contains("Undone"));
        assert!(entry.message.contains("test.rs"));
    }

    #[test]
    fn test_title_generated_event() {
        let mut app = App::new();

        app.handle_agent_event(AgentEvent::TitleGenerated {
            session_id: SessionId("s1".to_string()),
            title: "Research on Quantum Computing".to_string(),
        });

        let entry = app.event_log.last().unwrap();
        assert_eq!(entry.level, LogLevel::Success);
        assert!(entry.message.contains("Research on Quantum Computing"));
    }

    #[test]
    fn test_open_session_browser() {
        let mut app = App::new();
        let sessions = vec![
            pr_persistence::SessionSummary {
                id: SessionId("s1".to_string()),
                query: "test query".to_string(),
                status: "completed".to_string(),
                output_dir: None,
                total_tokens: 100,
                total_agents: 1,
                created_at: "2024-01-01".to_string(),
                updated_at: "2024-01-01".to_string(),
            },
        ];

        app.open_session_browser(sessions);
        assert_eq!(app.dialog, Some(Dialog::SessionBrowser));
        assert_eq!(app.session_list.len(), 1);
    }

    #[test]
    fn test_dialog_blocks_key_handling() {
        let mut app = App::new();
        app.dialog = Some(Dialog::Help);
        app.input_mode = InputMode::Insert;

        // Keys should be handled by dialog, not by input
        app.handle_key(crossterm::event::KeyEvent::new(
            KeyCode::Char('a'),
            KeyModifiers::NONE,
        ));
        // Dialog closed (any key closes help)
        assert!(app.dialog.is_none());
        // Input should NOT have 'a' appended because dialog consumed it
        assert_eq!(app.input, "");
    }
}
