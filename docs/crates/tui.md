# Crate `tui` — detailed documentation

## Overview

The `pr-tui` crate is a terminal user interface (TUI) for the Fathom Agent. Built on the **ratatui** library (a fork of tui-rs) using **crossterm** as the backend. It implements an interactive console with query input, real-time agent progress display, LLM output streaming, and session history.

---

## File structure

| File | Purpose |
|------|---------|
| `lib.rs` | Re-export of all modules |
| `app.rs` | Application state, key handling and agent events |
| `event.rs` | EventHandler: terminal reading + subscription to agent broadcast channel |
| `streaming.rs` | StreamingBuffer — line-by-line streaming buffer |
| `ui.rs` | Widget rendering (header, body, footer) |

---

## 1. `lib.rs` — module entry point

```rust
pub mod app;
pub mod event;
pub mod streaming;
pub mod ui;
```

All modules are public, but the main entry point is `App::run()`.

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

- **Normal** — navigation through the agent list, tab switching, commands
- **Insert** — typing a text query in the input field
- **Paste** — multi-line text insertion (supports `Ctrl+V`)

### 2.2 `HistoryTab` enum

```rust
pub enum HistoryTab {
    Current,
    Past,
    PastDetail,
}
```

- **Current** — current active session
- **Past** — list of past sessions
- **PastDetail** — detailed view of a selected past session

### 2.3 `App` struct

```rust
pub struct App {
    pub mode: InputMode,
    pub input: String,
    pub paste_buffer: Vec<String>,
    pub history_tab: HistoryTab,
    pub session_history: Vec<SessionSummary>,
    pub history_cursor: usize,
    pub agents: Vec<AgentInfo>,
    pub selected_agent: usize,
    pub output_text: String,
    pub status_message: String,
    pub is_running: bool,
    pub should_quit: bool,
    pub thinking: ThinkingState,
    pub config: AppConfig,
    pub scroll_offset: u16,
    pub output_scroll_offset: u16,
    pub history_scroll_offset: u16,
    pub past_detail_scroll_offset: u16,
    pub use_streaming: bool,
    pub streaming_buffers: HashMap<String, StreamingBuffer>,
    pub input_cursor: usize,
    pub input_scroll: usize,
    pub session_id: Option<SessionId>,
    pub session_details: Option<SessionDetails>,
    pub selected_finding_idx: Option<usize>,
    pub findings: Vec<pr_persistence::Finding>,
    pub event_rx: Option<broadcast::Receiver<AgentEvent>>,
    pub db: Option<Arc<Persistence>>,
    pub session_history_store: Option<SessionHistory>,
    pub steer_tx: Option<tokio::sync::mpsc::UnboundedSender<String>>,
    pub cancel_tx: Option<tokio_util::sync::CancellationToken>,
    pub completion_rx: Option<tokio::sync::mpsc::Receiver<SessionResult>>,
}
```

#### Key fields:
- `mode` — current input mode
- `input` — text in the input field
- `paste_buffer` — buffer for multi-line paste
- `agents` — current session agent list
- `selected_agent` — index of the selected agent

### 2.4 `AgentInfo` struct

```rust
pub struct AgentInfo {
    pub id: AgentId,
    pub task: String,
    pub status: pr_core::AgentStatus,
    pub thinking: String,
    pub summary: String,
    pub tokens_used: u64,
    pub findings_count: usize,
    pub tools_used: Vec<String>,
    pub children: Vec<AgentInfo>,
}
```

Tree-like structure: `children` allows rendering the agent hierarchy with indentation.

### 2.5 Constructor `App::new(query: String)`

1. Creates `AppState::new()` with configuration
2. Starts execution via `app_state.spawn_run(query.clone())`
3. Returns `App` with:
   - `is_running = true`
   - `status_message = "Agent started..."`
   - `mode = Insert`
   - populated `event_rx`, `db`, `session_history_store`, `steer_tx`, `cancel_tx`, `completion_rx`

### 2.6 Key press handling — `handle_key(key: KeyCode)`

#### Paste mode:

| Key | Action |
|-----|--------|
| `Esc` | Exit paste mode → Insert |
| `Enter` | Adds a line to `paste_buffer` |
| `Ctrl+V` | Finishes paste: merges `paste_buffer` + `input` → `input`, clears buffer → Insert |
| Other | Adds character to `input` (multi-line input) |

#### Insert mode:

| Key | Action |
|-----|--------|
| `Esc` | Switches to Normal |
| `Enter` | Submits query (`submit_query`) |
| `Ctrl+V` | Switches to Paste |
| `Backspace` | Deletes character before cursor |
| `Delete` | Deletes character after cursor |
| `Left` / `Right` | Moves cursor |
| `Home` / `End` | Beginning/end of line |
| Characters | Inserts character at cursor position |

#### Normal mode:

| Key | Action |
|-----|--------|
| `i` | → Insert |
| `Tab` | Next tab (Current → Past → PastDetail) |
| `Shift+Tab` | Previous tab |
| `j` / `Down` | Next agent |
| `k` / `Up` | Previous agent |
| `g` / `Home` | First agent |
| `G` / `End` | Last agent |
| `Ctrl+E` | Scroll output down |
| `Ctrl+Y` | Scroll output up |
| `f` | Toggle fullscreen output |
| `c` | Cancel session (with confirmation via status) |
| `Esc` | Reset selection or exit |
| `q` | Quit (only if session is not running) |

### 2.7 Session cancellation confirmation

`cancel_session()` uses a two-step confirmation:
1. First call → sets `status_message = "Press c again to cancel session"`
2. Second call (within 2 seconds, checked via `Instant::elapsed`) → sends cancel token, sets `is_running = false`
3. If more than 2 seconds have passed — resets confirmation

### 2.8 Agent event handling — `handle_agent_event(event: AgentEvent)`

Filters events by `session_id` (ignores foreign sessions).

#### Handling by variant:

| Event | Action |
|-------|--------|
| `SessionStarted { session_id }` | Saves `session_id` |
| `AgentSpawned` | Adds new `AgentInfo` to the list. Updates `session_id`. Loads previous agent messages from DB (batch of 50 for resume). Initializes `StreamingBuffer` |
| `AgentThinking { agent_id, thinking }` | Finds the agent, appends thinking to accumulated (`push_thinking`). If selected — updates `output_text`. Updates `status_message` with thinking preview |
| `ToolCallStarted` | Updates `status_message`: "🔧 Calling {tool_name}..." |
| `ToolCallCompleted` | Updates `status_message`: "✓ {tool_name} completed ({duration}ms)" |
| `MessageDelta` | Updates `output_text` via `update_output_text` |
| `MessageCompleted` | Replaces streaming buffer with final content |
| `AgentProgress` | Updates `status_message` |
| `AgentCompleted` | Sets agent status to `Completed`, updates `tokens_used` and `summary`. If streaming is off — updates `output_text` |
| `AgentFailed` | Sets status to `Failed`, updates `status_message` |
| `SessionCompleted` | Sets `is_running = false`, `status_message = "Session completed"` |
| `SessionFailed` | Sets `is_running = false`, `status_message = "Session failed"` |
| `FindingDiscovered` | Increments agent's `findings_count` |
| `StreamDelta` | Calls `push()` on the corresponding `StreamingBuffer` |
| `SubtaskSpawned` | Updates `status_message` |
| `SubtaskCompleted` | Updates `status_message` |
| `SteeringInjected` | Updates `status_message` |
| `Cancelled` | Updates `status_message` |

### 2.9 Thinking accumulation — `push_thinking(current, new)`

```rust
fn push_thinking(current: &mut String, new: &str) {
    if current.is_empty() {
        *current = new.to_string();
    } else {
        current.push_str("\n");
        current.push_str(new);
    }
}
```

Thinking is accumulated with newline separation. On each `AgentThinking` event the string is appended to the existing one.

### 2.10 Auto-hide thinking (30-second timeout)

In `tick()`:
```rust
if let Some(last) = self.thinking.last_update {
    if last.elapsed() > Duration::from_secs(30) {
        self.thinking.clear();
    }
}
```

If more than 30 seconds have passed without a new thinking event, thinking is cleared and `output_text` is updated.

### 2.11 `update_output_text()`

Recalculates `output_text` based on the selected agent:
1. If `use_streaming = true` and a streaming buffer exists for the agent → shows `buffer.text()`
2. If the agent has a `summary` → shows the summary
3. If there is thinking → shows thinking with the "💭 " prefix
4. Otherwise → "Waiting for output..."

### 2.12 Agent navigation — `navigate_agents(delta)`

```rust
fn navigate_agents(&mut self, delta: isize) {
    if self.agents.is_empty() { return; }
    let new = self.selected_agent as isize + delta;
    self.selected_agent = new.max(0).min(self.agents.len() as isize - 1) as usize;
    self.update_output_text();
    self.scroll_offset = 0;
    self.output_scroll_offset = 0;
}
```

Bounds: `[0, len-1]`. Scrolls are reset on navigation.

### 2.13 Main loop — `run(terminal)`

```rust
pub async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()>
```

Algorithm:
1. Creates `EventHandler::new(250)` (250ms tick rate)
2. Loop `loop`:
   a. Renders UI via `ui::draw(terminal, self)`
   b. Gets the next event: `self.events.next().await?`
   c. Matches by event type:
      - `Tick` → calls `self.tick()`
      - `Key(key)` → calls `self.handle_key(key.code)`
      - `AgentEvent(event)` → calls `self.handle_agent_event(event)`
      - `Quit` → `break`
   d. If `should_quit` → `break`
3. Returns `Ok(())`

### 2.14 `tick()` — periodic update

1. Checks session completion via `completion_rx.try_recv()`
   - `SessionResult::Success` → `is_running = false`, `status_message = "Session completed"`
   - `SessionResult::Failure(e)` → `is_running = false`, `status_message = "Error: {e}"`
2. Auto-hide thinking (30 seconds)
3. Updates `output_text` if session is completed (final summary)

### 2.15 Query submission — `submit_query()`

Algorithm:
1. Concatenates `input.trim()` (if non-empty)
2. If `paste_buffer` exists — merges it with input via `\n`
3. If query is empty — returns
4. Clears input and paste_buffer
5. Switches to Normal mode
6. Calls `App::new(query)` — starts a new session

---

## 3. `event.rs` — `EventHandler`

### 3.1 `Event` enum

```rust
pub enum Event {
    Tick,
    Key(KeyEvent),
    AgentEvent(AgentEvent),
    Quit,
}
```

### 3.2 `EventHandler` struct

```rust
pub struct EventHandler {
    rx: mpsc::Receiver<Event>,
    _terminal_handle: JoinHandle<()>,
    _agent_handle: JoinHandle<()>,
}
```

Stores the receiver and two JoinHandles for background tasks (automatically aborted on drop).

### 3.3 Constructor `EventHandler::new(tick_rate: Duration)`

1. Creates `mpsc::channel::<Event>(100)` — buffer of 100 events
2. Clones the sender for two tasks
3. Spawns `spawn_terminal_reader(tx.clone(), tick_rate)`
4. Spawns `spawn_agent_reader(tx.clone())`
5. Returns `EventHandler { rx, _terminal_handle, _agent_handle }`

### 3.4 `spawn_terminal_reader(tx, tick_rate)` — terminal reading

Background task `tokio::task::spawn_blocking`:
1. Sets `keyboard::set_cursor_shape(CursorShape::SteadyBlock)` (best effort)
2. Loop `loop`:
   a. **Polling with timeout**: `crossterm::event::poll(tick_rate)` every `tick_rate`
   b. If `poll` returned `true` — reads `crossterm::event::read()`
   c. Handles:
      - `Key(k)` if `Ctrl+C` → sends `Event::Quit`, break
      - `Key(k)` if `Ctrl+D` → sends `Event::Quit`, break
      - `Key(k)` → sends `Event::Key(k)`
      - Paste events are ignored (paste is handled via `Ctrl+V` in Insert mode)
   d. If `poll` returned `false` (timeout) → sends `Event::Tick`
3. If channel is closed (`send` returned `Err`) — break

**Polling every 100 ms** — standard interval for terminal TUIs. Too frequent polling loads the CPU, too rare polling makes the UI unresponsive.

### 3.5 `spawn_agent_reader(tx)` — subscription to agent broadcast channel

Background task `tokio::spawn`:
1. Attempts to subscribe to `pr_core::event_bus()` (broadcast channel)
2. If broadcast channel is not initialized — logs a warning and exits
3. Loop `loop`:
   a. `rx.recv().await` — receives the next event
   b. Matches:
      - `Ok(event)` → `tx.send(Event::AgentEvent(event)).await`
      - `Err(Lagged(n))` → logs warning (missed events)
      - `Err(RecvError::Closed)` → break (channel closed)
4. If mpsc channel is closed — break

**Channel conversion**: broadcast → mpsc. The broadcast channel can lose slow subscribers (Lagged), mpsc guarantees delivery.

### 3.6 `EventHandler::next() -> Result<Event>`

```rust
pub async fn next(&self) -> Result<Event> {
    self.rx.recv().await.ok_or(anyhow::anyhow!("Event channel closed"))
}
```

Asynchronously waits for the next event. Returns an error if all senders are disconnected.

---

## 4. `streaming.rs` — `StreamingBuffer`

### 4.1 Struct

```rust
pub struct StreamingBuffer {
    partial_line: String,
    lines: Vec<String>,
    last_published: Instant,
}
```

- `partial_line` — current accumulated line (does not yet contain `\n`)
- `lines` — array of completed lines
- `last_published` — timestamp of the last publication

### 4.2 Constructor `StreamingBuffer::new()`

```rust
pub fn new() -> Self {
    Self {
        partial_line: String::new(),
        lines: Vec::new(),
        last_published: Instant::now(),
    }
}
```

### 4.3 `push(delta: &str)` — accumulation algorithm

```rust
pub fn push(&mut self, delta: &str) {
    for ch in delta.chars() {
        if ch == '\n' {
            self.lines.push(std::mem::take(&mut self.partial_line));
        } else {
            self.partial_line.push(ch);
        }
    }
    self.last_published = Instant::now();
}
```

**Step-by-step algorithm:**
1. Iterates over characters of the incoming `delta`
2. If character is `\n` — current `partial_line` is moved to `lines` (via `mem::take`, zero-copy move), `partial_line` becomes an empty string
3. If character is not `\n` — it is appended to `partial_line`
4. Updates `last_published`

This guarantees that lines are published only when a complete newline is received. Intermediate tokens accumulate in `partial_line`.

### 4.4 `text() -> String` — getting the full text

```rust
pub fn text(&self) -> String {
    let mut result = self.lines.join("\n");
    if !self.partial_line.is_empty() {
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(&self.partial_line);
    }
    result
}
```

Joins all completed lines via `\n`, appends `partial_line` (if non-empty) with a preceding newline.

### 4.5 `flush()` — forced publication of partial_line

```rust
pub fn flush(&mut self) {
    if !self.partial_line.is_empty() {
        self.lines.push(std::mem::take(&mut self.partial_line));
    }
}
```

Moves `partial_line` to `lines` even without `\n`. Used when streaming completes.

### 4.6 `line_count() -> usize`

```rust
pub fn line_count(&self) -> usize {
    self.lines.len() + if self.partial_line.is_empty() { 0 } else { 1 }
}
```

Accounts for both completed lines and the incomplete partial_line.

### 4.7 `last_updated() -> Instant`

Returns `last_published` for external timeout checks.

---

## 5. `ui.rs` — widget rendering

### 5.1 Main function `draw(frame, app)`

Builds a layout of 3 vertical blocks:

```
┌─────────────────────────────────┐
│           Header                │  ← draw_header()
├─────────────────────────────────┤
│                                 │
│             Body                │  ← draw_body()
│                                 │
├─────────────────────────────────┤
│           Footer                │  ← draw_footer()
└─────────────────────────────────┘
```

**Sizes**:
- Header: `Length(3)`
- Body: `Min(0)` (takes remaining space)
- Footer: `Length(3)`

### 5.2 `draw_header(frame, app, area)` — header

Panel styled as `" Fathom "` with white text on blue background. `Span::styled` with `Style::default().fg(Color::White).bg(Color::Blue).add_modifier(Modifier::BOLD)`.

### 5.3 `draw_body(frame, app, area)` — main body

Two display variants:

#### Current session (`HistoryTab::Current`):
```
┌────────────┬──────────────────────────┐
│   Agent    │         Output           │
│   Tree     │   (streaming/thinking/   │
│   (30%)    │    summary) (70%)        │
└────────────┴──────────────────────────┘
```
- Left panel (30%): `draw_agent_tree` — agent tree
- Right panel (70%): `draw_output_panel` — selected agent's output

#### Past sessions list (`HistoryTab::Past`):
```
┌─────────────────────────────────────┐
│         Past Sessions               │
│  Session 1: "query..." (completed)  │
│  Session 2: "query..." (failed)     │
│  ...                                │
└─────────────────────────────────────┘
```

#### Past session details (`HistoryTab::PastDetail`):
```
┌────────────┬──────────────────────────┐
│   Agent    │         Findings         │
│   Tree     │                          │
│   (30%)    │   (70%)                  │
└────────────┴──────────────────────────┘
```

### 5.4 `draw_agent_tree(frame, app, area)` — agent tree

#### If no agents:
Shows `Line::from("No agents yet...")` with gray color.

#### If agents exist:
For each agent calls `render_agent_recursive` with initial `depth = 0`.

### 5.5 `render_agent_recursive(agent, lines, depth)` — recursive rendering

**Algorithm:**
1. Creates indentation: `" ".repeat(depth * 3)` (3 spaces per level)
2. Builds status icon:
   - `Spawned` → `"⏳"`
   - `Running` → `"⚡"`
   - `Thinking` → `"💭"`
   - `Completed` → `"✅"`
   - `Failed` → `"❌"`
   - `Cancelled` → `"🚫"`
3. Truncates agent ID: `&agent.id.0[..8.min(agent.id.0.len())]`
4. Truncates task to `max_task_len = (area_width - depth*3 - 16).max(10)` characters
5. Adds `Line::from(vec![
       Span::raw(indent),
       Span::raw(icon),
       Span::styled(short_id, Style::default().fg(Color::DarkGray)),
       Span::raw(" "),
       Span::raw(truncated_task),
   ])`
6. Recursively calls `render_agent_recursive` for each `child` with `depth + 1`

#### Example output:
```
⏳ abc12345 Research quantum computing
  ⚡ def67890 Analyze recent papers
    ✅ ghi11111 Summarize findings
  ⏳ jkl22222 Search patents
```

### 5.6 `draw_output_panel(frame, app, area)` — output panel

#### If `fullscreen_output`:
Renders `output_text` in a `" Output "` block spanning the entire available area.

#### Otherwise:
Two vertical blocks:
- **Output** (`Min(0)`): `output_text` in a `" Output "` block
- **Status** (`Length(3)`): `status_message` in a `" Status "` block

#### Output scroll:
For `output_scroll_offset`:
```rust
let visible_lines = output_area.height as usize;
let total_lines = output_text.lines().count().max(1);
let scroll = app.output_scroll_offset.min(total_lines.saturating_sub(visible_lines) as u16);
let text = output_text.lines().skip(scroll as usize).collect::<Vec<_>>().join("\n");
```

### 5.7 `draw_footer(frame, app, area)` — footer panel

#### Insert mode:
```
┌────────────────────────────────────────┐
│ > [input text]                         │
│ Mode: INSERT │ Enter: submit │ Esc: normal │ Ctrl+V: paste │ Tab: switch │
└────────────────────────────────────────┘
```

- First line: `"> "` prefix + cursor
- Second line: key hints

#### Normal mode:
```
┌────────────────────────────────────────┐
│ i: input │ Tab: switch │ j/k: navigate │ c: cancel │ q: quit │
└────────────────────────────────────────┘
```

#### Paste mode:
```
┌────────────────────────────────────────┐
│ Paste mode (Ctrl+V to finish): [text]  │
│ Lines: N │ Enter: new line │ Esc: cancel │ Ctrl+V: finish │
└────────────────────────────────────────┘
```

### 5.8 Agent tree rendering with depth indentation

Recursive traversal of `AgentInfo::children` tree:
- Each depth level adds 3 spaces to indentation
- Child agents always follow directly after their parent
- This creates a visual hierarchy:
  ```
  ⚡ root_agent    Research topic
    ⚡ child_1     Analyze papers
      ✅ grandchild Summarize
    ⏳ child_2     Search patents
  ```

---

## 6. `AppState` — internal state (helper struct)

### 6.1 `AppState` struct

```rust
pub struct AppState {
    pub config: AppConfig,
    pub db: Arc<Persistence>,
    pub llm: Arc<dyn LlmProvider>,
    pub tools: Arc<ToolRegistry>,
    pub events: broadcast::Sender<AgentEvent>,
    pub steer_tx: tokio::sync::mpsc::UnboundedSender<String>,
    pub steer_rx: tokio::sync::mpsc::UnboundedReceiver<String>,
    pub cancel: tokio_util::sync::CancellationToken,
    pub completion_tx: tokio::sync::mpsc::Sender<SessionResult>,
}
```

### 6.2 `AppState::new()` — constructor

1. Loads `AppConfig::load()`
2. Creates output directory
3. Opens `Persistence::open`
4. Creates LLM provider (falls back to `DeepSeekProvider`)
5. Creates `ToolRegistry::with_builtins()`
6. Initializes broadcast channel (capacity 1024)
7. Creates steer channel (unbounded mpsc)
8. Creates completion channel (capacity 1)
9. Creates `CancellationToken`

### 6.3 `spawn_run(query)` — execution startup

Background task `tokio::spawn`:
1. Creates `Coordinator::new(...)` with full dependencies
2. Connects steer, cancel, completion_tx
3. Calls `coordinator.execute().await`
4. On success — sends `SessionResult::Success`
5. On error — sends `SessionResult::Failure`

---

## 7. Data flow

```
User Input (Terminal)
       │
       ▼
 EventHandler ──── tick_rate: 250ms ──── Tick
       │
       ├── KeyEvent → App::handle_key()
       │                    │
       │                    ▼
       │              submit_query()
       │                    │
       │                    ▼
       │              AppState::spawn_run()
       │                    │
       │                    ▼
       │              Coordinator::execute()
       │                    │
       │                    ├── broadcast::Sender<AgentEvent>
       │                    │         │
       │                    │         ▼
       │                    │   spawn_agent_reader()
       │                    │         │
       │                    │         ▼
       │              EventHandler::AgentEvent
       │                    │
       │                    ▼
       │              App::handle_agent_event()
       │                    │
       │                    ├── Update agents[]
       │                    ├── Update streaming_buffers
       │                    ├── Update thinking
       │                    └── Update output_text
       │
       └──────────────────────────────────────────────
                              │
                              ▼
                    Terminal (ratatui)
                    ui::draw()
```

---

## 8. Key features

### 8.1 Thread safety
- `EventHandler` uses `mpsc::channel` to pass events from background threads to the main thread
- `broadcast::channel` for multicast agent events (multiple subscribers)
- All `App` state is modified in a single thread (main loop)

### 8.2 Graceful degradation
- If the broadcast channel is not initialized — `spawn_agent_reader` logs a warning and exits
- If the database is unavailable — history is not loaded, sessions work without persistence
- If streaming is disabled — falls back to summary

### 8.3 Resume support
On `AgentSpawned`, previous agent messages are loaded from the DB (up to 50), so context can continue after a restart.

### 8.4 Memory management
- `StreamingBuffer` uses `mem::take` for zero-copy string moves
- `paste_buffer` is cleared after submission
- The `agents` list grows linearly (no automatic cleanup of completed agents)