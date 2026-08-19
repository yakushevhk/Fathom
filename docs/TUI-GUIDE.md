# TUI Guide

Interactive terminal UI for Fathom, built with [ratatui](https://ratatui.rs/) and [crossterm](https://github.com/crossterm-rs/crossterm). Provides a live tree view of the agent hierarchy, streaming output, event log, and operator control for approvals and questions.

---

## Launching

```bash
# Start the TUI interactively
fathom tui

# Start with an initial query pre-filled in the input
fathom tui "research the latest developments in Rust"

# Use a persona/profile
fathom tui --profile researcher

# Replay a stored session (prefix match accepted)
fathom tui --replay <session-id>
```

The TUI switches the terminal to an alternate screen, enables raw mode and mouse capture, and restores everything on exit (`q` or `Ctrl+C`).

---

## Layout

The screen is divided into three vertical strips:

```
┌─────────────────────────────────────────────────────────────┐
│  Status bar │ Token gauge │ Token sparkline                │  ← Header
├──────────────────────────────┬──────────────────────────────┤
│  Agents (60%)                │  Output / Thinking (60%)    │
│  ── agent tree               │  ── streaming output        │  ← Body
│                              │  ── thinking/reasoning      │
│  ────────────────────────    │  ────────────────────────    │
│  Tools / Jobs / Memory (40%) │  Event Log (40%)            │
├──────────────────────────────┴──────────────────────────────┤
│  Query Input / Status bar / Help text                       │  ← Footer
└─────────────────────────────────────────────────────────────┘
```

### Header

Three regions:

1. **Status bar** — "Fathom" label, active session ID (first 8 hex chars), elapsed time (`MM:SS`). Shows `[REPLAY]` when in replay mode.
2. **Token gauge** — progress bar of `total_tokens / context_window`. Color:
   - Green (< 75%)
   - Yellow (75–90%)
   - Red (> 90%)
3. **Token sparkline** — time-series chart of token consumption sampled on each agent/session completion. Cyan, capped at 120 samples.

### Body — Left panel

A vertical split (60/40). The top pane is always the **Agents** tree. The bottom pane switches between **Tools**, **Jobs**, or **Memory** depending on which panel is selected via `Tab`.

#### Agents panel

Displays the agent spawn tree in **DFS order** (depth-first traversal). Each row shows:

- **Indentation** — `"  "` per depth level
- **Branch icon** — `▸` (collapsed, has children) / `▾` (expanded, has children) / `  ` (leaf node)
- **State icon** — colored glyph:
  | Icon | State                | Color     |
  |------|----------------------|-----------|
  | `○`  | Idle                 | DarkGray  |
  | `◑`  | Planning             | Yellow    |
  | `◐`  | Researching          | Cyan      |
  | `◑`  | Analyzing            | Blue      |
  | `◕`  | Synthesizing         | Magenta   |
  | `✎`  | Writing              | White     |
  | `✓`  | Complete             | Green     |
  | `✗`  | Error                | Red       |
- **Role** — bold magenta (e.g. `coordinator`, `researcher`, `writer`)
- **Token count** — `[555tk]`, gray
- **Elapsed time** — e.g. `1.2s`, `3m 15s`
- **Task preview** — truncated to 40 chars, gray
- **Active tool** — `→ web_search`, blue italic
- **Last 2 tool calls** — gray, reversed

A summary line at the top shows `Active: N` (yellow) and `Done: N` (green).

**Navigation** (when the Agents panel is selected — `Tab` to it):
- `Up` / `Down` — move the cursor through the visible agent list
- `Left` — collapse the subtree under the agent under the cursor
- `Right` — expand the subtree under the agent under the cursor

Collapsed nodes hide their descendants from the DFS traversal, keeping the tree compact. The cursor is a highlighted row (`bg: DarkGray, bold`).

#### Tools panel

Shows tool call activity:

1. **Active tools** — agents currently running a tool, with `→ tool_name (role)`.
2. **Recent tool calls** — last 10 completed calls, with duration (e.g. `✓ web_search (1.2s)`).

#### Jobs panel

Lists durable background jobs (from the `fathom jobs` subsystem). Each row:

- Status glyph: `·` queued, `▶` running, `✓` completed, `✗` failed, `⊘` cancelled, `?` unknown
- Short job ID (first 8 chars)
- Status text (padded 9-wide)
- Attempt counter: `attempt/max_attempts`
- Task preview (30 chars)

Detects **stale** jobs (status `running` but the PID is dead) and displays them as `stale`.

If no jobs exist, shows: `no jobs — submit via CLI or server API`.

#### Memory panel

Shows long-term semantic memory statistics (when `[memory] enabled = true` is configured). Layout:

- **Stats line** — `agent:N user:N run:N graph:Nn/Ne`
- **Recent memories** — up to 15 newest active memories from persistent scopes. Each row:
  - Scope prefix: `a` (agent), `u` (user), `r` (run)
  - Short memory ID (last 8 chars of reversed UUID)
  - Content preview

If memory is disabled, shows: `memory disabled — set [memory] enabled = true`.

### Body — Right panel

A vertical split (60/40). The top pane is the **Output** panel (with optional **Thinking** sub-panel). The bottom pane is the **Event Log**.

#### Output panel

Shows the assembled output text — the published content from the `writer` or `coordinator` agent's streaming buffer. When no output exists, placeholder text is shown:

> Output will appear here when agents produce results.
> Press 'i' to enter a query and start a research session.

The display auto-scrolls to show the last `visible_height` lines.

#### Thinking sub-panel

When the LLM emits reasoning/thinking tokens, the output panel splits into a **Thinking** section (top 8 rows) and the **Output** section (remaining space). The thinking panel has a dark background (`Rgb(20, 25, 35)`) and italic gray text. Each agent's thinking content is prefixed with `[role]`.

- **Auto-hide** — hides after 30 seconds of inactivity
- **Toggle** — press `t` to collapse/expand manually
- The thinking panel is never shown when collapsed

#### Event Log panel

A scrollable list of timestamped events. Each entry has:

- **Level prefix** — `•` Info (white), `✓` Success (green), `✗` Error (red), `→` Tool (blue)
- **Timestamp** — `HH:MM:SS`, gray
- **Message** — truncated to 80 chars

The log is capped at 1000 entries; when exceeded, the oldest 100 are dropped.

Events logged include:
- Session started / completed / failed / forked
- Agent spawned / state changed / completed / failed
- Tool call started / completed
- LLM stream chunks (NOT logged — too noisy)
- Findings discovered
- File changes undone
- Titles generated
- Question asked / approval requested
- Steering instructions sent

### Footer / Input

The bottom bar shows the **Query Input** with three modes:

| Mode    | Prompt     | Behavior                                                      |
|---------|------------|---------------------------------------------------------------|
| Normal  | *(none)*   | Shows keybinding hints. No text input.                        |
| Insert  | `> `       | Type a query. `Enter` submits, `Shift+Enter` adds newline.    |
| Paste   | `Paste> `  | Buffered text paste (`Ctrl+V`). `Esc` inserts and returns.    |

When a **pending question** exists, the hint area shows: `❓ AGENT QUESTION — type the answer and press Enter`.

When a **pending approval** exists, the hint area shows: `🔐 APPROVAL NEEDED — y: allow | n: deny`.

---

## Keybindings

| Key               | Context        | Action                                                      |
|-------------------|----------------|-------------------------------------------------------------|
| `q`               | Normal         | Quit the TUI                                                |
| `Ctrl+C`          | Normal         | Quit the TUI                                                |
| `i`               | Normal         | Enter insert mode (type a query)                            |
| `Enter`           | Insert         | Submit query / send answer to pending question              |
| `Shift+Enter`     | Insert         | Insert newline in input (multi-line)                        |
| `Esc`             | Insert         | Return to normal mode                                       |
| `Esc`             | Normal         | Switch focus to the Input panel                             |
| `Tab`             | Any            | Cycle panels forward: Input → Agents → Output → Log → Jobs → Memory → Input |
| `BackTab`         | Any            | Cycle panels backward                                       |
| `Up` / `Down`     | Agents panel   | Move agent cursor up/down in the tree                       |
| `Up` / `Down`     | Other panels   | Scroll the event log / output vertically                    |
| `Left`            | Agents panel   | Collapse the agent subtree under the cursor                 |
| `Right`           | Agents panel   | Expand the agent subtree under the cursor                   |
| `t`               | Normal         | Toggle thinking panel visibility                            |
| `c`               | Normal         | Clear output text                                           |
| `b`               | Normal         | Open session browser dialog                                 |
| `y`               | Normal         | Approve a pending side-effect tool call                     |
| `n`               | Normal         | Deny a pending side-effect tool call                        |
| `?`               | Normal         | Toggle help overlay                                         |
| `@`               | Insert         | Enter file reference autocomplete mode                      |
| `Ctrl+V`          | Insert         | Enter paste mode (bracketed paste)                          |
| `Ctrl+Z`          | Normal         | Request undo of last file change (logged to event log)      |
| `Backspace`       | Insert / Paste | Delete character before cursor                              |
| `Delete`          | Insert         | Delete character at cursor                                  |
| `Left` / `Right`  | Insert         | Move cursor left/right in input                             |
| `Home` / `End`    | Insert         | Jump to start/end of input                                  |
| `Up` / `Down`     | Insert         | Navigate input history (previous/next submitted query)      |
| Mouse scroll up   | Any            | Scroll up (offset +3)                                       |
| Mouse scroll down | Any            | Scroll down (offset −3)                                     |
| Mouse click       | Normal         | Click bottom area → enter insert mode, focus Input panel    |

---

## Approval Flow

When an agent wants to run a side-effect tool (one that modifies files, sends emails, etc.), it emits an `ApprovalRequested` event. The TUI surfaces this as:

1. The footer hint changes to: `🔐 APPROVAL NEEDED — y: allow | n: deny`
2. An event log entry is created: `Agent <id> wants to run '<tool>' [<args_preview>] — press y to allow, n to deny`
3. The `App.pending_approval` struct holds the request ID, agent ID, tool name, args preview, and a oneshot channel for the reply.

Pressing `y` sends `true` (allowing the tool call); pressing `n` sends `false` (denying it). The event log records the decision.

---

## Question Tool Input

When an agent uses the `question` tool to ask the operator for input, the TUI surfaces this as:

1. The footer hint changes to: `❓ AGENT QUESTION — type the answer and press Enter`
2. An event log entry is created: `Agent <id> asks: <question> — type the answer and press Enter`
3. The `App.pending_question` struct holds the request ID, agent ID, question text, and a oneshot channel for the reply.

The user types their answer in insert mode and presses `Enter`. The answer is sent to the agent. If the input is empty/whitespace-only, `"(no answer)"` is sent instead.

---

## Dialog Types

### Help overlay (`?`)

A centered modal listing all keybindings. 56×22 box. Any key, or pressing `?` again, closes it.

### Session browser (`b`)

A centered modal listing past sessions. Navigate with `Up`/`Down`, select with `Enter`, close with `Esc` or `q`.

Each row shows status icon, short session ID (first 8 chars), and query preview (40 chars).

### Confirm dialog

A small centered modal with a message and "Press any key to continue". Used for general confirmations.

### File picker (`@` in insert mode)

Type `@` followed by a filename prefix to trigger file reference autocomplete. The TUI scans the working directory for matching files (up to 20 matches).

- Continues typing alphanumeric characters, `.`, `/`, `_`, `-` to refine the search
- `Tab` accepts the first suggestion
- `Enter` selects the highlighted file
- `Esc` cancels
- If only one match, auto-completes immediately

The file picker dialog shows a list of matching files with `📁` (directory) or `📄` (file) icons.

---

## Streaming Mode

The TUI uses a `StreamingBuffer` per agent to render LLM output smoothly:

- **Line-buffered**: Tokens (chunks) are accumulated. Only completed lines (ending with `\n`) are published to the display. A trailing partial line is hidden until it completes.
- **Reduces re-renders**: By not publishing partial lines, the TUI avoids flickering on every token.
- **Flush on completion**: When an agent completes, `flush()` publishes the final partial line.
- **Output assembly**: The assembled `output_text` is built from the `writer` or `coordinator` agent's stream.

The `StreamingBuffer` tracks:
- `buffer` — raw accumulated text
- `published_lines` — lines that ended with `\n`, joined for display
- `partial_line` — the current incomplete line
- `has_new` — flag consumed by `ack_new_content()`

---

## Replay Mode

Launch with `fathom tui --replay <session-id>` (prefix match accepted). The TUI:

1. Loads the session from the history database
2. Sets `app.replay_mode = true` (shows `[REPLAY]` in the header)
3. Populates all agents with their final state (`completed`, `failed`, `cancelled`, or `running`)
4. Loads findings into the event log
5. Displays agent tree and stats as a static snapshot

No live research session is started. The footer shows normal keybinding hints without the insert/steer functionality.

---

## Agent Tree Navigation

The agent tree is rendered in **DFS (depth-first) order**, respecting collapsed nodes:

1. **Roots** are agents without a parent (or whose parent is unknown). Sorted by spawn time, then by ID.
2. **Traversal** pushes children onto a stack in reverse spawn order, so earlier siblings appear first.
3. **Collapsed nodes** (`collapsed` set) — when a node is collapsed, its children are skipped during traversal. The collapsed node itself is still shown with a `▸` icon.
4. **Expanded nodes** show a `▾` icon and their full subtree.

The visible agent list is indexed by `agents_cursor`. The `Up`/`Down` keys move the cursor through this list. The cursor is highlighted when the Agents panel is selected.

**Collapse/expand via keyboard:**
- `Left` — inserts the agent ID into `collapsed`, hiding its subtree
- `Right` — removes the agent ID from `collapsed`, revealing its subtree

---

## Input History

The TUI maintains an input history buffer (`input_history: Vec<String>`). In insert mode:

- `Up` — cycles backward through submitted queries. The current in-progress input is saved as a snapshot and restored when cycling back down.
- `Down` — cycles forward, eventually returning to the snapshot.

History is preserved for the lifetime of the TUI session.

---

## Session Steering (Mid-Run Input)

When a research session is already running and the user submits another query (via `Enter` in insert mode), the new input is sent through the **steering channel** (`steer_tx`). This allows the user to:

- Ask follow-up questions mid-run
- Redirect the agent's focus
- Provide additional context

The event log records: `steering instruction sent to the running session`.

When the research task finishes, the steering channel is closed and the next query starts a fresh session.

---

## Mouse Support

The TUI enables mouse capture on startup. Supported interactions:

- **Scroll up** — scrolls the event log/output up (+3 offset)
- **Scroll down** — scrolls the event log/output down (−3 offset)
- **Left click** on the bottom area — switches to insert mode and focuses the Input panel

Mouse position is tracked in `app.mouse_pos` for potential hover interactions.

---

## Architecture Overview

### Crate structure (`crates/tui/src/`)

| File           | Responsibility                                                    |
|----------------|-------------------------------------------------------------------|
| `lib.rs`       | Re-exports `app`, `ui`, `event`, `streaming` modules              |
| `app.rs`       | `App` struct, state management, key/mouse/event handling          |
| `ui.rs`        | ratatui rendering: layout, panels, dialogs, helpers               |
| `event.rs`     | `EventHandler`, `AppEvent` enum, terminal/agent event readers     |
| `streaming.rs` | `StreamingBuffer` — line-buffered LLM token display               |

### Event flow

```
Terminal (crossterm)
  └─ spawn_terminal_reader() ──→ mpsc channel ──→ EventHandler
                                                      │
AgentEvent (broadcast from pr_core)                    │
  └─ spawn_agent_reader() ────────────────────────────→┘
                                                      │
                                              AppEvent loop
                                                      │
                                              app.handle_key()
                                              app.handle_agent_event()
                                              app.handle_mouse()
                                                      │
                                              terminal.draw(ui::draw)
```

### Key data structures

- **`App`** — holds all UI state: agents, event log, streams, thinking, tool calls, jobs, memory, dialogs, input
- **`PendingQuestion`** — oneshot channel + metadata for the `question` tool
- **`PendingApproval`** — oneshot channel + metadata for side-effect approval gates
- **`MemorySnapshot`** — periodic refresh of memory store counts and recent entries
- **`StreamingBuffer`** — per-agent line-buffered text accumulator
- **`AgentInfo`** — per-agent metadata: id, parent, role, task, state, tokens, depth, tool calls, start time
- **`ToolCallEntry`** — tool call record with agent ID, tool name, start time, duration, result preview