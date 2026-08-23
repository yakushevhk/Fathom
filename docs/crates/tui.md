# Crate `tui` — detailed documentation

## Overview

The `pr-tui` crate is a terminal user interface (TUI) for the Fathom Agent. Built on the **ratatui** library (a fork of tui-rs) using **crossterm** as the backend. It implements an interactive console with query input, real-time agent progress display, LLM output streaming, session history, background job monitoring, long-term memory panel, and operator control (questions and approvals).

---

## File structure

| File | Purpose |
|------|---------|
| `lib.rs` | Module entry point, re-exports |
| `app.rs` | Application state, key handling, agent events |
| `event.rs` | EventHandler: terminal reading + broadcast channel subscription |
| `ui.rs` | Widget rendering (header, body, footer, overlays) |
| `streaming.rs` | StreamingBuffer — line-by-line streaming display optimization |

---

## 1. `lib.rs` — module entry point

```rust
pub mod app;
pub mod ui;
pub mod event;
pub mod streaming;

pub use app::*;
pub use streaming::*;
```

All modules are public. The main entry point is `App::new()` followed by the host loop.

---

## 2. `app.rs` — `App` — state and application logic

### 2.1 `InputMode` enum

```rust
pub enum InputMode {
    Normal,
    Insert,
    Paste,
}
```

- **Normal** — navigation, tab switching, commands
- **Insert** — typing a text query in the input field
- **Paste** — multi-line text insertion via bracketed paste (`Ctrl+V`)

### 2.2 `Panel` enum (replaces HistoryTab)

```rust
pub enum Panel {
    Agents,
    Output,
    Log,
    Jobs,
    Memory,
    Input,
}
```

Six navigable panels. `Tab`/`BackTab` cycles through them in order: `Agents → Output → Log → Jobs → Memory → Input → Agents`.

### 2.3 `Dialog` enum

```rust
pub enum Dialog {
    Help,              // Keymap overlay (`?` toggles)
    SessionBrowser,    // List/search past sessions
    Confirm(String),   // Confirm action (y/n)
    FilePicker,        // File reference picker for @ autocomplete
}
```

### 2.4 `App` struct

```rust
pub struct App {
    // --- Core state ---
    pub should_quit: bool,
    pub session_id: Option<SessionId>,
    pub query: String,
    pub input: String,
    pub input_cursor: usize,
    pub input_mode: InputMode,

    // --- Agent tracking ---
    pub agents: HashMap<AgentId, AgentInfo>,
    pub event_log: Vec<EventLogEntry>,
    pub total_tokens: u64,
    pub context_window: u64,        // default 128_000
    pub total_agents: u32,
    pub start_time: std::time::Instant,
    pub scroll_offset: u16,
    pub selected_panel: Panel,

    // --- Input history ---
    pub input_history: Vec<String>,
    pub history_index: Option<usize>,
    pub input_snapshot: String,

    // --- Mid-run steering ---
    pub steer_tx: Option<tokio::sync::mpsc::UnboundedSender<String>>,

    // --- Thinking/reasoning ---
    pub thinking: HashMap<AgentId, ThinkingState>,
    pub thinking_collapsed: bool,
    pub last_thinking_time: Option<std::time::Instant>,

    // --- Streaming ---
    pub streams: HashMap<AgentId, StreamingBuffer>,
    pub output_text: String,

    // --- Paste ---
    pub paste_buffer: String,
    pub in_paste: bool,

    // --- Tool calls ---
    pub tool_calls: Vec<ToolCallEntry>,
    pub active_tools: HashMap<AgentId, String>,

    // --- Background jobs ---
    pub jobs: Vec<pr_persistence::JobRow>,

    // --- Memory ---
    pub memory: Option<std::sync::Arc<pr_memory::Memory>>,
    pub memory_snapshot: MemorySnapshot,

    // --- Operator control plane ---
    pub pending_question: Option<PendingQuestion>,
    pub pending_approval: Option<PendingApproval>,

    // --- Agent tree ---
    pub collapsed: std::collections::HashSet<AgentId>,
    pub agents_cursor: usize,

    // --- Sparkline ---
    pub token_history: Vec<u64>,

    // --- UI state ---
    pub show_help: bool,
    pub replay_mode: bool,
    pub dialog: Option<Dialog>,

    // --- File references (@ autocomplete) ---
    pub file_refs: Vec<String>,
    pub in_file_ref: bool,
    pub file_ref_query: String,
    pub file_ref_selected: usize,

    // --- Session browser ---
    pub session_list: Vec<pr_persistence::SessionSummary>,

    // --- Mouse ---
    pub mouse_pos: (u16, u16),
}
```

#### Key field descriptions:

| Field | Type | Purpose |
|-------|------|---------|
| `agents` | `HashMap<AgentId, AgentInfo>` | All agents in current session (flat map, tree via parent links) |
| `event_log` | `Vec<EventLogEntry>` | Timestamped log entries for the Log panel |
| `total_tokens` | `u64` | Aggregate token count across all agents |
| `context_window` | `u64` | Context window size (default 128k) |
| `selected_panel` | `Panel` | Currently focused panel |
| `thinking` | `HashMap<AgentId, ThinkingState>` | Per-agent reasoning content |
| `streams` | `HashMap<AgentId, StreamingBuffer>` | Per-agent streaming display buffers |
| `tool_calls` | `Vec<ToolCallEntry>` | Tool call history with timing |
| `active_tools` | `HashMap<AgentId, String>` | Currently executing tool per agent |
| `jobs` | `Vec<JobRow>` | Durable background jobs (newest first) |
| `memory` | `Option<Arc<Memory>>` | Long-term memory store reference |
| `memory_snapshot` | `MemorySnapshot` | Periodic memory stats for display |
| `pending_question` | `Option<PendingQuestion>` | Question awaiting user input |
| `pending_approval` | `Option<PendingApproval>` | Side-effect awaiting y/n approval |
| `collapsed` | `HashSet<AgentId>` | Collapsed agent tree nodes |
| `agents_cursor` | `usize` | Cursor row in agents panel |
| `token_history` | `Vec<u64>` | Time series for header sparkline |
| `show_help` | `bool` | Help overlay visible |
| `replay_mode` | `bool` | Showing a stored session (`tui --replay`) |
| `dialog` | `Option<Dialog>` | Active modal dialog |
| `session_list` | `Vec<SessionSummary>` | Session browser data |

### 2.5 `AgentInfo` struct

```rust
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
```

| Field | Type | Purpose |
|-------|------|---------|
| `id` | `AgentId` | Unique agent identifier |
| `parent` | `Option<AgentId>` | Parent agent ID (for tree rendering) |
| `role` | `String` | Agent role description |
| `task` | `String` | Agent's assigned task |
| `state` | `AgentState` | Current state (Spawned/Running/Thinking/Completed/Failed/Cancelled) |
| `tokens` | `u64` | Tokens used by this agent |
| `depth` | `u32` | Nesting depth in agent tree |
| `tool_calls` | `Vec<String>` | Tools this agent has called |
| `start_time` | `Instant` | When the agent was spawned |

### 2.6 Supporting types

#### `PendingQuestion`

```rust
pub struct PendingQuestion {
    pub request_id: String,
    pub agent_id: AgentId,
    pub question: String,
    pub reply: tokio::sync::oneshot::Sender<String>,
}
```

#### `PendingApproval`

```rust
pub struct PendingApproval {
    pub request_id: String,
    pub agent_id: AgentId,
    pub tool: String,
    pub args_preview: String,
    pub reply: tokio::sync::oneshot::Sender<bool>,
}
```

#### `EventLogEntry`

```rust
pub struct EventLogEntry {
    pub time: chrono::DateTime<chrono::Local>,
    pub message: String,
    pub level: LogLevel,
}
```

#### `LogLevel`

```rust
pub enum LogLevel {
    Info,
    Success,
    Error,
    Tool,
}
```

#### `ToolCallEntry`

```rust
pub struct ToolCallEntry {
    pub agent_id: AgentId,
    pub tool: String,
    pub start_time: std::time::Instant,
    pub duration_ms: Option<u64>,
    pub result_preview: Option<String>,
}
```

#### `ThinkingState`

```rust
pub struct ThinkingState {
    pub content: String,
    pub last_update: std::time::Instant,
}
```

#### `MemorySnapshot`

```rust
pub struct MemorySnapshot {
    pub agent_active: usize,
    pub user_active: usize,
    pub run_active: usize,
    pub entity_nodes: i64,
    pub entity_edges: i64,
    pub recent: Vec<MemoryLine>,
    pub refreshed: bool,
}

pub struct MemoryLine {
    pub id: String,       // last 8 chars of memory id
    pub scope: String,
    pub content: String,
}
```

`MemorySnapshot::refresh(mem)` reloads counts + the 15 newest active memories from the store (synchronous reads — rusqlite is fast enough for the UI loop).

### 2.7 Constructor — `App::new()`

Takes **no arguments**. Initializes all fields to defaults:

- `context_window = 128_000`
- `input_mode = InputMode::Normal`
- `selected_panel = Panel::Input`
- All maps, vectors, and options empty/None

The host loop is responsible for connecting to the event bus and spawning sessions.

### 2.8 Key press handling — `handle_key(key)`

#### Dialog-specific keys (when `dialog` is Some):

Handled by `handle_dialog_key()` before other modes.

#### File reference mode (when `in_file_ref` is true):

Handled by `handle_file_ref_key()`.

#### Paste mode:

| Key | Action |
|-----|--------|
| `Esc` | Exit paste mode → Insert |
| `Enter` | Adds a line to `paste_buffer` |
| `Ctrl+V` | Finishes paste: merges buffer + input → input |
| Other | Adds character to `paste_buffer` (multi-line) |

#### Insert mode:

| Key | Action |
|-----|--------|
| `Esc` | Switch to Normal |
| `Enter` | Submit query |
| `Ctrl+V` | Switch to Paste mode |
| `Backspace` | Delete character before cursor |
| `Delete` | Delete character after cursor |
| `Left` / `Right` | Move cursor |
| `Home` / `End` | Beginning/end of line |
| `Up` / `Down` | Navigate input history |
| `@` | Enter file reference mode |
| Characters | Insert at cursor |

#### Normal mode:

| Key | Action |
|-----|--------|
| `i` | Enter Insert mode |
| `Tab` | Next panel (Agents→Output→Log→Jobs→Memory→Input) |
| `BackTab` | Previous panel |
| `Up` / `Down` | Scroll or navigate agents cursor |
| `Left` | Collapse agent node (Agents panel) |
| `Right` | Expand agent node (Agents panel) |
| `?` | Toggle help overlay |
| `Ctrl+C` | Quit |
| `q` | Quit |
| `Esc` | Return to Input panel |

### 2.9 Agent tree navigation

The `agents` field is a flat `HashMap<AgentId, AgentInfo>`. Tree structure is derived from `parent` links. `visible_agents()` computes the display order by walking the tree, respecting `collapsed` nodes. `agents_cursor` tracks the selected row; `Left`/`Right` toggle collapse state.

### 2.10 Agent event handling — `handle_agent_event(event)`

| Event | Action |
|-------|--------|
| `SessionStarted` | Saves session_id |
| `AgentSpawned` | Adds AgentInfo with parent/role/task/state/depth/tokens/start_time |
| `ToolCallStarted` | Records ToolCallEntry, updates active_tools, adds EventLogEntry |
| `ToolCallCompleted` | Sets duration_ms on entry, removes from active_tools, adds EventLogEntry |
| `AgentStateChanged` | Updates agent state, adds EventLogEntry |
| `AgentCompleted` | Sets Completed, updates tokens, adds EventLogEntry |
| `AgentFailed` | Sets Failed, adds EventLogEntry |
| `SessionCompleted` | Marks session done, adds EventLogEntry |
| `SessionFailed` | Marks session failed, adds EventLogEntry |
| `Finding` | Adds EventLogEntry |
| `StreamDelta` | Pushes to StreamingBuffer for the agent |
| `LlmStreamChunk` | Pushes to StreamingBuffer |
| `QuestionAsked` | Creates PendingQuestion |
| `ApprovalRequested` | Creates PendingApproval |
| `SessionForked` | Adds EventLogEntry |
| `FileChangeUndone` | Adds EventLogEntry |
| `TitleGenerated` | Adds EventLogEntry |

### 2.11 Thinking accumulation

`ThinkingState` stores accumulated reasoning content per agent. Auto-hide: if `last_thinking_time` exceeds 30 seconds without a new thinking event, thinking is cleared and `output_text` is updated.

### 2.12 Agent navigation — `navigate_agents(delta)`

Bounds: `[0, visible_agents().len() - 1]`. Scrolls are reset on navigation.

---

## 3. `event.rs` — `EventHandler`

### 3.1 `AppEvent` enum

```rust
pub enum AppEvent {
    Terminal(CrosstermEvent),
    Agent(AgentEvent),
    Tick,
    Quit,
}
```

### 3.2 `EventHandler` struct

```rust
pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<AppEvent>,
    _tx: mpsc::UnboundedSender<AppEvent>,
}
```

### 3.3 Constructor — `EventHandler::new()`

Takes **no arguments** (no tick rate parameter). Returns `(Self, mpsc::UnboundedSender<AppEvent>)`.

Spawns two background tasks:
1. `spawn_terminal_reader(tx)` — polls crossterm events at **50ms** interval (`crossterm::event::poll(Duration::from_millis(50))`)
2. `spawn_agent_reader(tx, agent_rx)` — subscribes to `pr_core::event_bus()` broadcast channel

### 3.4 `spawn_terminal_reader(tx)` — terminal reading

Background task (`tokio::spawn`):
1. Polls `crossterm::event::poll(50ms)` — on timeout, sends `Tick`
2. On event: reads `crossterm::event::read()`
3. `Ctrl+C` / `Ctrl+D` → sends `Quit`
4. Key events → sends `AppEvent::Terminal(event)`
5. Paste events ignored (handled via `Ctrl+V` in Insert mode)

### 3.5 `spawn_agent_reader(tx, agent_rx)` — broadcast subscription

Subscribes to `pr_core::event_bus()` and forwards events as `AppEvent::Agent(event)`. Handles `Lagged(n)` gracefully.

### 3.6 `EventHandler::next() -> Option<AppEvent>`

Async wait on the mpsc receiver. Returns `None` when all senders are dropped.

---

## 4. `streaming.rs` — `StreamingBuffer`

### 4.1 Purpose

Buffers incoming LLM tokens and only publishes completed lines to the display. A trailing partial line is hidden until it completes (receives a newline), reducing re-renders during streaming.

### 4.2 Struct

```rust
pub struct StreamingBuffer {
    buffer: String,
    published_lines: Vec<String>,
    partial_line: String,
    has_new: bool,
}
```

### 4.3 Methods

| Method | Description |
|--------|-------------|
| `new()` | Empty buffer |
| `push(delta)` | Accumulates text; newline moves completed line to `published_lines` |
| `published_text()` | All published lines joined with newlines |
| `has_new_content()` | Whether new content is available since last ack |
| `ack_new_content()` | Resets the new-content flag |
| `partial_line()` | Current incomplete line (not yet published) |
| `flush()` | Force-publish the partial line |
| `line_count()` | Number of published lines |
| `clear()` | Reset all content |

---

## 5. `ui.rs` — widget rendering

### 5.1 Main layout — `draw(frame, app)`

```
┌─────────────────────────────────┐
│           Header                │  ← sparkline, session info
├─────────────────────────────────┤
│                                 │
│             Body                │  ← panel-dependent content
│                                 │
├─────────────────────────────────┤
│           Footer/Input          │  ← input field or status bar
└─────────────────────────────────┘
```

If `show_help` is true, a help overlay is drawn on top.
If `dialog` is Some, the appropriate dialog is drawn (SessionBrowser, Confirm, FilePicker).

### 5.2 `draw_header(frame, app, area)`

Shows session ID, agent count, token count, and a sparkline from `token_history`.

### 5.3 `draw_body(frame, app, area)`

Dispatches to the active panel's draw function:

| Panel | Rendered by |
|-------|-------------|
| `Agents` | `draw_agents_panel` — collapsible tree from `agents` HashMap |
| `Output` | `draw_output_panel` — output text, thinking sub-panel, streaming |
| `Log` | `draw_log_panel` — event log entries with level coloring |
| `Jobs` | `draw_jobs_panel` — background job list |
| `Memory` | `draw_memory_panel` — memory snapshot stats + recent entries |
| `Input` | Inline in footer |

### 5.4 `draw_agents_panel(frame, app, area)`

Renders the agent tree using `parent` links and `depth`. Collapsed nodes hide their children. Status icons per state:

| State | Icon |
|-------|------|
| Spawned | ⏳ |
| Running | ⚡ |
| Thinking | 💭 |
| Completed | ✅ |
| Failed | ❌ |
| Cancelled | 🚫 |

### 5.5 `draw_output_panel(frame, app, area)`

Two sub-panels:
- **Thinking** (top): shows `thinking` content for selected agent (if any)
- **Output** (bottom): shows `output_text` (streaming buffer or summary)

If `pending_question` or `pending_approval` is present, a control banner is rendered.

### 5.6 `draw_log_panel(frame, app, area)`

Lists `event_log` entries with timestamp, level-colored icon, and message.

### 5.7 `draw_jobs_panel(frame, app, area)`

Lists `jobs` with status, query, and timestamps.

### 5.8 `draw_memory_panel(frame, app, area)`

Shows `memory_snapshot` stats (agent/user/run counts, entity nodes/edges) and the 15 most recent active memories.

### 5.9 `draw_footer(frame, app, area)`

- **Insert mode**: `"> "` prefix + input text with cursor
- **Normal mode**: Key hints
- **Pending question**: Question text + input field
- **Pending approval**: Tool name + args preview + y/n prompt

### 5.10 Overlays

- `draw_help_overlay` — modal keymap reference
- `draw_session_browser` — list/search past sessions
- `draw_confirm_dialog` — confirm action
- `draw_file_picker` — @ file reference autocomplete

### 5.11 Helpers

- `format_tokens(n)` — human-readable ("12.5k", "1.2M")
- `format_elapsed_short(d)` — short duration ("1.2s", "3m 12s")

---

## 6. Data flow

```
User Input (Terminal)
       │
       ▼
 EventHandler ──── 50ms poll ──── Tick
       │
       ├── TerminalEvent → App::handle_key()
       │                        │
       │                        ▼
       │                  submit_query()
       │                        │
       │                  spawn session via server API
       │
       ├── AgentEvent → App::handle_agent_event()
       │                    │
       │                    ├── Update agents HashMap
       │                    ├── Update streams (StreamingBuffer)
       │                    ├── Update thinking
       │                    ├── Update tool_calls
       │                    ├── Update event_log
       │                    ├── Handle pending_question/pending_approval
       │                    └── Update output_text
       │
       └── Tick → refresh UI state
                              │
                              ▼
                    Terminal (ratatui)
                    ui::draw()
```

---

## 7. Key features

### 7.1 Thread safety
- `EventHandler` uses `mpsc::UnboundedChannel` for event delivery
- `broadcast::channel` for multicast agent events (multiple subscribers)
- All `App` state is modified in a single thread (main loop)

### 7.2 Graceful degradation
- If broadcast channel is not initialized — `spawn_agent_reader` logs warning and exits
- If database is unavailable — history not loaded, sessions work without persistence
- If streaming disabled — falls back to summary

### 7.3 Collapsible agent tree
- `collapsed: HashSet<AgentId>` tracks which nodes are folded
- `Left`/`Right` keys toggle collapse in the Agents panel
- `visible_agents()` computes display order respecting collapse state

### 7.4 Operator control plane
- `PendingQuestion` / `PendingApproval` enable mid-run user interaction
- Questions and approvals block the agent until answered via `reply` oneshot channel

### 7.5 Memory panel
- `MemorySnapshot::refresh()` runs synchronous SQLite reads (fast enough for UI loop)
- Shows agent/user/run scope counts, entity graph stats, and 15 newest active memories

### 7.6 Replay mode
- `replay_mode: bool` — set by `tui --replay` flag
- UI shows a stored session instead of a live run

### 7.7 File references (@ autocomplete)
- Typing `@` enters file reference mode
- `file_refs`, `file_ref_query`, `file_ref_selected` manage the autocomplete state
- `FilePicker` dialog renders the selection