# Full Architectural Documentation of `pr-agent`

> Every function is described step by step, down to individual lines of code.

---

## Table of Contents

1. [lib.rs — Module Entry Point](#1-librs)
2. [runtime.rs — Agent Runtime Core](#2-runtimers)
3. [coordinator.rs — Session Coordinator](#3-coordinatorrs)
4. [compaction.rs — Context Compaction](#4-compactionrs)
5. [prompt.rs — System Prompt Construction](#5-promptrs)
6. [prompts/ — Prompt Text Templates](#6-prompts)
7. [hooks.rs — Lifecycle Hooks](#7-hooksrs)
8. [ipc.rs — Inter-Process Protocol](#8-ipcrs)
9. [process_manager.rs — Worker Process Management](#9-process_managerrs)
10. [background.rs — Background Tasks](#10-backgroundrs)
11. [budget.rs — Result Budget](#11-budgetrs)
12. [doom_loop.rs — Loop Detection](#12-doom_looprs)
13. [recovery.rs — Crash Recovery](#13-recoveryrs)
14. [resume.rs — Session Resumption](#14-resumers)
15. [tool_executor.rs — Parallel Tool Executor](#15-tool_executorrs)

---

## 1. lib.rs

**File:** `src/lib.rs` — entry point of the `pr-agent` module.

### Purpose

Declares all public submodules of the crate and re-exports their contents via `pub use ...::*`, so external consumers can write `use pr_agent::AgentRuntime` instead of `use pr_agent::runtime::AgentRuntime`.

### Design Decisions

The module structure is designed around a **layered runtime architecture**. The `runtime` module provides the core agent loop, `coordinator` orchestrates multi-agent sessions, and the remaining modules provide supporting infrastructure (compaction, hooks, IPC, process management, tool execution, budget, and recovery). The `ipc` and `process_manager` modules are intentionally kept private — they are only used by `coordinator.rs` for multiprocess fan-out, and exposing them would create an unnecessary public API surface. This encapsulation ensures that the multiprocess communication protocol can evolve without affecting external consumers.

### Declared Modules (14 total)

```
runtime, coordinator, compaction, ipc, process_manager, prompt,
tool_executor, budget, background, resume, doom_loop, recovery, hooks
```

### Re-exports (11 total)

All modules except `ipc` and `process_manager` re-export their public API to the crate root. The `ipc` and `process_manager` modules remain internal — they are used only inside `coordinator.rs`.

---

## 2. runtime.rs

**File:** `src/runtime.rs` — agent runtime core. Contains the main agent loop.

### 2.1. Structure `AgentRuntime`

The main working unit of the system. Each instance is one LLM agent operating in its own "think → call tool → get result" loop.

**Architecture overview:**

`AgentRuntime` is the heart of the entire system. It owns the complete conversational state of a single agent — the message history, the token accounting, the compaction engine, the tool budget, the loop guard, and the registries for memory and skills. Because agents are spawned recursively (the `spawn_agent` tool creates child runtimes), the runtime is fully self-contained: a child agent inherits exactly what it needs (contact database, CRM, cancellation token, role LLM assignment) and runs independently until it produces an `AgentOutput`.

The runtime is designed around a few core architectural principles:

- **Single ownership of context.** All context management (messages, token estimation, compaction) lives in one place, so there is a single well-defined notion of "how full is the context" at any moment.
- **Turn-based execution.** The loop is strictly sequential per agent: drain external inputs → one LLM call → execute the requested tools → feed results back. This makes the agent's behavior deterministic and debuggable.
- **Layered safety rails.** Tools are gated by role permissions, PreToolUse hooks, doom-loop detection, and cascading cancellation, then bounded by turn and result budgets. Failures are degraded gracefully rather than aborting the run.
- **Observability by construction.** Every meaningful transition (state change, tool call started/completed, spawn, completion, failure) is emitted on the event bus, so the TUI, logging, and the session coordinator all see the same stream of events.

**Event bus and external coordination:** the `event_tx` broadcast channel fans events out to any number of subscribers (TUI, stall monitor, coordinator). The runtime itself treats the bus as fire-and-forget: send errors are ignored because the system must keep running even if no one is listening. The same event stream is what allows the multiprocess mode to mirror worker progress into the parent coordinator's event bus via the IPC `Event` variant (see [section 8](#8-ipcrs)).

**Fields:**

| Field | Type | Purpose |
|------|------|---------|
| `id` | `AgentId` | Unique agent identifier (UUID) |
| `session_id` | `SessionId` | Session identifier this agent belongs to |
| `parent_id` | `Option<AgentId>` | Parent agent ID (for nested spawns) |
| `role` | `AgentRole` | Role: Coordinator, Researcher, Analyst, Verifier, Writer |
| `task` | `String` | Text description of the agent's task |
| `depth` | `u32` | Current nesting depth in the agent tree |
| `llm` | `Arc<dyn LlmProvider>` | LLM provider (GPT-4, Claude, DeepSeek, etc.) |
| `tools` | `Arc<ToolRegistry>` | Registry of available tools |
| `event_tx` | `broadcast::Sender<AgentEvent>` | Event bus for notifications (TUI, logging) |
| `db` | `Arc<Persistence>` | Persistence layer (SQLite) |
| `working_dir` | `PathBuf` | Working directory (for file operations) |
| `max_iterations` | `u32` | Maximum number of main loop iterations |
| `config` | `AppConfig` | Full application configuration |
| `messages` | `Vec<Message>` | Message history (system, user, assistant, tool) |
| `tokens_used` | `u64` | Token counter (own) |
| `descendant_tokens` | `u64` | Tokens spent by child agents |
| `estimated_tokens` | `u32` | Estimated current context size in tokens |
| `contact_db` | `Option<Arc<dyn ContactStore>>` | Contact database (inherited by child agents) |
| `crm` | `Option<Arc<CrmSync>>` | CRM synchronization |
| `compaction_engine` | `CompactionEngine` | Context compaction engine |
| `turn_budget` | `TurnBudget` | Tool output budget per turn |
| `memory_store` | `MemoryStore` | Agent persistent memory (cross-session facts) |
| `skill_registry` | `SkillRegistry` | Registry of discovered skills |
| `doom_loop` | `DoomLoopDetector` | Loop detection |
| `doom_nudged` | `bool` | Whether a "nudge" (first warning) has been sent |
| `harvested_findings` | `Vec<Finding>` | Structured findings extracted from tool metadata |
| `cancel` | `CancellationToken` | Cooperative cancellation token |
| `denied_tools` | `HashSet<String>` | Tools prohibited for this role |
| `steer_rx` | `Option<Arc<Mutex<UnboundedReceiver<String>>>>` | Mid-run instruction channel from user |
| `bg_results` | `Arc<Mutex<Vec<(String, Result<String, String>, u64)>>>` | Results from background child agents |
| `stop_continuations` | `u32` | Counter of forced continuations by Stop hooks |
| `role_llms` | `HashMap<String, Arc<dyn LlmProvider>>` | LLM providers by role (fleet E8) |

### 2.2. `denied_tools_for_role(config, role) -> HashSet<String>`

**Algorithm:**
1. Maps the `AgentRole` value to a string key: `"coordinator"`, `"researcher"`, `"analyst"`, `"verifier"`, `"writer"`.
2. Looks up `config.agent.deny_tools` for an entry with that key.
3. If found — collects tools into a `HashSet`, converting each name to lowercase.
4. If not found — returns an empty `HashSet` (`.unwrap_or_default()`).

### 2.3. `AgentRuntime::new(...)`

**Algorithm:**
1. Extracts `max_iterations` from configuration.
2. Calls `denied_tools_for_role()` to build the set of prohibited tools.
3. Creates a `TurnBudget` with the limit `config.context.turn_budget_bytes`.
4. Creates a `CompactionEngine` from context configuration.
5. Determines the home directory (`dirs::home_dir()`, fallback — `/tmp`).
6. Creates a `MemoryStore` from the home directory.
7. Creates a `SkillRegistry` from the home directory.
8. Calls `skill_registry.discover()` in best-effort mode (error is logged, not fatal).
9. Initializes all struct fields with default values: empty vectors, zeros, `None`, empty `HashSet`, new `CancellationToken`.

### 2.4. Builder Methods

#### `with_role_llms(mut self, map) -> Self`
Replaces `self.role_llms` with the provided map. Allows the coordinator to assign different LLMs for different roles (fleet E8).

#### `with_steer_rx(mut self, rx) -> Self`
Connects the mid-run instruction channel (fleet E1). The user can send instructions while the agent is running, and they will be processed at the turn boundary.

#### `with_cancel_token(mut self, token) -> Self`
Replaces the cancellation token. Child agents inherit `child_token()` from the parent.

#### `cancel_token(&self) -> CancellationToken`
Clones and returns the current cancellation token (for supervisors).

### 2.5. `build_tool_context(&self) -> ToolContext`

**Algorithm:**
1. Creates `ToolContext::new(self.working_dir, self.config.search)`.
2. Attaches the LLM provider via `.with_llm(self.llm.clone())`.
3. If `self.contact_db` exists — attaches via `.with_contact_db()`.
4. If `self.crm` exists — attaches via `.with_crm()`.
5. Returns the ready context.

Called once per run (not per tool invocation), so that stateful subsystems (file locks, file history, read tracking) work across calls.

### 2.6. `emit(&self, event)`

Logs an event to the `event_tx` bus. Send errors are ignored (there may be no receivers).

### 2.7. `emit_tool_hook_denied(&self, tool)`

Logs a warning via `tracing::warn!` that the tool `tool` was rejected by a PreToolUse hook.

### 2.8. `build_system_prompt(&self) -> String`

**Algorithm:**
1. Creates a `PromptBuilder::new(role, task, depth, max_depth, model)`.
2. **Context layer:** calls `builder.add_env(config, working_dir)` — adds an `<env>` block with working directory, platform, model, date, git status.
3. **Volatile layer:** extracts tool schemas from the registry, **filtering** out tools from `denied_tools` (prohibited tools are hidden from the model).
4. **Volatile layer:** gets a memory block from `memory_store.to_system_prompt_block()`. If non-empty — adds it.
5. **Volatile layer:** gets a skills block from `skill_registry.to_system_prompt_block()`. If non-empty — adds it.
6. **Stable layer:** adds behavioral instructions ("use tools, cite sources, be accurate, record findings in files, use memory").
7. Calls `builder.build()` — concatenation of layers: stable → context → volatile.

### 2.9. `recalculate_estimated_tokens(&mut self)`

Fully recalculates `estimated_tokens` from all messages in `self.messages` via `estimate_messages_tokens()`. Used after compaction (bulk rewrites).

### 2.10. `track_message_tokens(&mut self, msg)`

Incrementally adds the token estimate of a single message to `estimated_tokens`. Monotonic between compactions — a full O(n) recalculation is not needed on every addition.

### 2.11. `run_compaction(&mut self)` — async

**Algorithm:**
1. Records `tokens_before = self.estimated_tokens`.
2. Clones `self.llm` for the closure.
3. Calls `self.compaction_engine.compact(&mut self.messages, summarize_fn)`.
4. `summarize_fn` is a closure that:
   - Accepts `prompt_messages` (the summarization prompt).
   - Creates a `CompletionRequest` with temperature=0.3, max_tokens=2048, stream=false, empty tool list.
   - Calls `llm.complete(&req)`.
   - Extracts text from the `Message::Assistant` response.
5. If `compact()` returned `Ok(cr)`:
   - If `cr.tokens_after < tokens_before` — logs the reduction percentage, assigns `self.messages = cr.messages`, recalculates tokens.
   - Otherwise — logs "no effect".
6. If `compact()` returned `Err(e)` — logs the error (not fatal).

### 2.12. `AgentRuntime::run(&mut self) -> Result<AgentOutput>` — async — MAIN LOOP

**Step-by-step algorithm:**

#### Step 0: Initialization
1. Pushes `Message::system(build_system_prompt())` into `self.messages`.
2. Pushes `Message::user(&self.task)` into `self.messages`.
3. Calls `recalculate_estimated_tokens()`.
4. Saves both messages to the database via `db.add_message()`.
5. Emits event `AgentStateChanged { state: Researching }`.
6. Extracts `tool_schemas` from the registry, filtering out prohibited ones.
7. Creates `tool_ctx = build_tool_context()`.
8. Initializes: `iterations = 0`, `final_content = ""`, `doom_warning = None`.

#### Step 1: Main loop (`'main_loop: while iterations < max_iterations`)
Each iteration is one agent "turn".

##### 1a. Drain steering channel (fleet E1)
- If `steer_rx` is connected:
  - Acquires lock on `rx`.
  - Reads all accumulated instructions via `try_recv()` in a loop.
  - For each: creates `Message::user("[USER INSTRUCTION] {msg}")`, pushes to history, saves to DB, tracks tokens.

##### 1b. Inject background child results (fleet E2)
- Acquires lock on `bg_results`.
- Takes all elements via `std::mem::take`.
- For each `(label, result, tokens)`:
  - Adds `descendant_tokens += tokens`.
  - Formats text: on success — `[background agent {label} completed]\n{summary}` (truncated to 4000 chars); on error — `[background agent {label} failed: {e}]`.
  - Pushes `Message::user(text)` to history, saves to DB, tracks tokens.

##### 1c. Check cooperative cancellation
- If `self.cancel.is_cancelled()`:
  - Logs warning.
  - Emits `AgentFailed { error: "cancelled" }`.
  - `anyhow::bail!("agent cancelled")`.

##### 1d. Reset turn budget
- `self.turn_budget = TurnBudget::new(config.context.turn_budget_bytes)`.

##### 1e. Check compaction need
- `self.compaction_engine.set_estimated_tokens(self.estimated_tokens)`.
- If `should_compact()` returns `true` — calls `self.run_compaction().await`.

##### 1f. Build LLM request
- `CompletionRequest { messages: self.messages.clone(), tools: tool_schemas.clone(), temperature, max_tokens, stream: false }`.

##### 1g. Call LLM
- `self.llm.complete(&req).await`.
- On error: logs, emits `AgentFailed`, returns `Err`.

##### 1h. Track tokens
- If `response.usage` exists — `tokens_used += usage.total_tokens`.

##### 1i. Add assistant response
- Pushes `response.message` into `self.messages`.
- Saves to DB, tracks tokens.

##### 1j. Extract content and tool_calls
- If the response is `Message::Assistant { content, tool_calls }`:
  - If `content` is not empty — updates `final_content`, emits `LlmStreamChunk`.

##### 1k. Handle missing tool_calls (model wants to stop)
- If `tool_calls` is empty:
  - **Stop hooks (fleet E3):** checks `stop_continuations < MAX_STOP_CONTINUATIONS` (3).
    - If yes — calls `run_stop_hooks(&config.hooks, &summary_so_far)`.
    - If no — skips hooks, uses `StopVerdict::Stop`.
  - If `StopVerdict::Continue(reason)`:
    - `stop_continuations += 1`.
    - Logs.
    - Pushes `Message::user("[hook] Do not stop yet: {reason}")`.
    - `continue 'main_loop` — agent continues working.
  - If `StopVerdict::Stop` — `break` from the loop.

##### 1l. Process tool_calls
For each `tool_call` in order:

###### 1l-i. Doom loop detection
- Calls `self.doom_loop.record_and_check(tool_name, &tool_args)`.
- If the detector triggers (3+ identical calls):
  - Collects IDs of all remaining sibling tool_calls.
  - **First time (doom_nudged == false):**
    - Sets `doom_nudged = true`.
    - Forms a "nudge" message: "Repeated identical invocation detected. DO NOT repeat — try different arguments or finish."
    - For the first remaining call_id — pushes nudge, for the rest — `Cancelled: {nudge}`.
    - `break` from the tool_calls loop → return to LLM for strategy change.
  - **Second time (doom_nudged == true):**
    - Forms warning: "Doom loop detected: tool invoked repeatedly even after warning. Stopping agent."
    - Emits `AgentFailed`.
    - Responds to all remaining tool_calls + already collected pending_spawns with error messages.
    - If `final_content` is empty — uses warning.
    - Sets `doom_warning`.
    - `break 'main_loop` — full agent stop.

###### 1l-ii. Emit ToolCallStarted

###### 1l-iii. Cascading cancellation
- If `shell_failed` contains an error:
  - Result = `ToolOutput::err("Cancelled: sibling shell tool failed with: {err}")` with metadata `cascade_cancelled: true`.
  - Proceeds to next step (does not execute the tool).

###### 1l-iv. Role permission gate (fleet E5)
- If `denied_tools` contains `tool_name`:
  - Result = `ToolOutput::err_code("Permission denied: role ... is not allowed to use '...'", "permission_denied")`.

###### 1l-v. PreToolUse hooks (fleet E3)
- Calls `run_pre_tool_hooks(&config.hooks, tool_name, &tool_args)`.
- If `PreToolVerdict::Deny(reason)`:
  - Result = `ToolOutput::err_code("Denied by hook: {reason}", "hook_denied")`.
  - Logs via `emit_tool_hook_denied`.

###### 1l-vi. Tool execution
- If `PreToolVerdict::Allow`:
  - `self.tools.execute(tool_name, tool_args, &tool_ctx).await`.
  - On success: if `tool_name == "shell"` and `!output.success` — stores the error in `shell_failed`.
  - On execution error: `ToolOutput::err("Tool execution error: {e}")`.

###### 1l-vii. Sub-agent delegation (fleet D4)
- If `tool_name == "spawn_agent"` and metadata contains `"spawn_request": true`:
  - Saves `(call_id, metadata)` to `pending_spawns`.
  - `continue` — does not push a tool message immediately.

###### 1l-viii. PostToolUse hooks (fleet E3)
- Calls `run_post_tool_hooks(&config.hooks, tool_name, &tool_args, &result.content, success)`.
- If it returns `Some(extra)` — appends `extra` to `result.content`.

###### 1l-ix. Auto-persist contacts (fleet C1)
- If the tool is `extract_contacts` or `find_leads` and the result is successful:
  - Extracts `contact_db` from `tool_ctx`.
  - For `extract_contacts` — calls `autosave_extracted(&db, contacts_meta, &origin)`.
  - For `find_leads` — calls `autosave_leads(&db, leads_meta)`.
  - If anything was saved/merged — appends a notification to `result.content`.
  - Updates result metadata with the `auto_saved` field.

###### 1l-x. Harvest findings (fleet C4)
- Calls `self.harvest_finding(tool_name, &tool_args, &result)`.
- If it returns `Some(finding)` — pushes to `self.harvested_findings`.

###### 1l-xi. Emit ToolCallCompleted
- With result preview (first 200 chars) and `duration_ms`.

###### 1l-xii. Save full result to DB
- `db.add_tool_result(agent_id, tool_name, tool_args, result, duration)`.

###### 1l-xiii. Truncation + turn budget
- Calls `apply_turn_budget(tool_name, result, max_bytes, max_lines, &mut turn_budget, working_dir)`.
- Returns `Truncated::Unchanged` or `Truncated::Truncated { replacement }`.
- Extracts content for the tool message.

###### 1l-xiv. Add tool message
- Pushes `Message::tool(tool_call_id, content)` to history.
- Saves to DB, tracks tokens.

##### 1m. Run collected spawn requests (fleet D4)
- If `pending_spawns` is not empty — calls `self.run_spawn_batch(&mut pending_spawns).await`.

#### Step 2: Post-loop
- If `iterations >= max_iterations` and no doom_warning — logs a warning.
- If no doom_warning — emits `AgentStateChanged { state: Complete }`.
- Returns `AgentOutput { agent_id, summary, tokens_used, descendant_tokens, findings, aborted }`.

### 2.13. `prepare_child(&mut self, meta) -> Result<(AgentId, AgentRuntime)>`

**Algorithm:**
1. Computes `child_depth = self.depth + 1`.
2. Checks `child_depth > config.agent.max_depth` — if so, `bail!`.
3. Extracts `task` from `meta["task"]` — if empty, `bail!`.
4. Extracts `role` from `meta["role"]` (default `"researcher"`), maps to `AgentRole`.
5. Extracts `context: Vec<String>` from `meta["context"]`.
6. Generates a new `AgentId`.
7. Creates a DB record (`db.create_agent`).
8. Emits `AgentSpawned`.
9. Forms `full_task`: if `context` is not empty — adds bullets at the end of the task with the heading "## Context from parent agent".
10. Determines LLM: if `role_llms` contains an entry for the role — uses it, otherwise inherits the parent's.
11. Creates `AgentRuntime::new(...)` with the full set of parameters.
12. Inherits `contact_db` and `crm`.
13. Sets `child_token()` from the parent's `cancel` as the cancellation token.
14. Returns `(agent_id, child_runtime)`.

### 2.14. `run_spawn_batch(&mut self, pending) -> Result<()>` — async

**Algorithm:**

#### Preparation
1. Computes `width = max(1, config.agent.max_concurrent_children)` — parallel child limit.
2. Computes `headroom_chars` — how many characters are free in the context: `(context_window * 4) - (estimated_tokens * 4)`.
3. Determines `spill_dir = working_dir/.pr-context/spills`.
4. Clones `db`, `event_tx`.

#### Split into foreground and background
5. For each `(call_id, meta)` from `pending`:
   - Checks `meta["background"]` — if `true`, the agent is background.
   - Calls `self.prepare_child(&meta)`.
   - **Background:** creates a future via `child_wait_future`, wraps in `tokio::spawn`, writes result to `bg_results`.
   - **Foreground:** adds to `items`.
   - On preparation error — adds to `early_fails`.

#### Immediate confirmation of background launches
6. For each background launch — pushes `ToolOutput::ok("Background agent {label} launched...")` via `record_spawn_result`.

#### Parallel foreground launch
7. For each `(call_id, agent_id, child)` in `items` — creates a future via `child_wait_future`.
8. Runs all futures via `futures::stream::iter(futs).buffered(width).collect().await`.

#### Result processing
9. `early_fails` → `ToolOutput::err("Sub-agent failed: {err}")` → `record_spawn_result`.
10. Main results:
    - On success: `descendant_tokens += tokens`, `ToolOutput::ok(summary)` → `record_spawn_result`.
    - On error: `ToolOutput::err(...)` → `record_spawn_result`.

### 2.15. `record_spawn_result(&mut self, call_id, output) -> Result<()>`

**Algorithm:**
1. Saves the result to DB (`db.add_tool_result`).
2. Applies truncation + turn budget (`apply_turn_budget`).
3. Extracts content (possibly truncated).
4. Pushes `Message::tool(call_id, content)` to history.
5. Saves to DB, tracks tokens.

### 2.16. `child_wait_future(child, agent_id, db, tx, timeout_secs, headroom_chars, batch_len, spill_dir) -> Pin<Box<Future>>`

**Algorithm:**
1. Runs `child.run()` with optional `tokio::time::timeout(timeout_secs, ...)`.
2. On `Ok(output)`:
   - If `output.aborted` — updates DB status to `Failed`, emits `AgentFailed`, returns error.
   - Otherwise — updates DB status to `Completed`, emits `AgentCompleted`, creates `ResultBudget::new(headroom_chars, batch_len, spill_dir)`, limits the result via `budget.cap_result(&output.summary)`, returns `(capped_summary, total_tokens)`.
3. On `Err(e)` — updates DB status to `Failed`, emits `AgentFailed`, returns error.

### 2.17. `harvest_finding(&self, tool_name, tool_args, result) -> Option<Finding>`

**Algorithm:**
1. If `!result.success` — returns `None`.
2. Extracts `result.metadata`.
3. By `tool_name`:
   - **`extract_contacts`:**
     - Extracts `counts` from metadata (emails, phones, social_profiles, persons, companies).
     - Determines `origin` from arguments `url` (or `"inline text"`).
     - If origin starts with `http` — creates `Source { url, title }`.
     - Returns `Finding` with title "Contacts extracted from {origin}", content with counters, confidence=0.7.
   - **`find_leads`:**
     - Extracts array `leads` from metadata.
     - If empty — `None`.
     - Takes first 5 leads, formats `"- {name} @ {company}"`.
     - Returns `Finding` with title "Leads harvested: {count}", confidence=0.6.
   - **Other tools:** `None`.

### 2.18. Structure `AgentOutput`

| Field | Type | Description |
|------|------|-------------|
| `agent_id` | `AgentId` | Agent ID |
| `summary` | `String` | Final text (final answer or doom loop warning) |
| `tokens_used` | `u64` | Own tokens |
| `descendant_tokens` | `u64` | Tokens of all descendants |
| `findings` | `Vec<Finding>` | Structured findings |
| `aborted` | `bool` | `true` if stopped by doom loop detector |

---

## 3. coordinator.rs

**File:** `src/coordinator.rs` — session coordinator. Manages the lifecycle of a research session: from decomposition to synthesis.

### Architecture Overview

The `Coordinator` is the **orchestrator** for a single research session. While `AgentRuntime` handles one agent's turn loop, the `Coordinator` handles the multi-agent workflow: it decomposes the user's query into subtasks, fans out to parallel researcher agents, collects results, optionally reflects on gaps (LeadGen mode), synthesizes everything into a coherent report, and writes output files.

The coordinator is also the **entry point for graceful degradation**. It runs a background heartbeat that touches the session DB every 60 seconds (so the `SessionResumer` doesn't treat a live session as stale), and a stall monitor that tracks per-agent progress and cancels stalled agents after a configurable grace period. If the session crashes, the `SessionResumer` can recover it by reading completed agents from the DB and re-running only the pending subtasks.

### 3.1. Structure `Coordinator`

| Field | Type | Purpose |
|------|------|---------|
| `session_id` | `SessionId` | Session identifier |
| `query` | `String` | Original user query |
| `llm` | `Arc<dyn LlmProvider>` | Main LLM provider |
| `tools` | `Arc<ToolRegistry>` | Tool registry |
| `event_tx` | `broadcast::Sender<AgentEvent>` | Event bus |
| `db` | `Arc<Persistence>` | Persistence layer |
| `output_dir` | `PathBuf` | Directory for output files |
| `config` | `AppConfig` | Configuration |
| `total_tokens` | `u64` | Total session token counter |
| `total_agents` | `u32` | Total number of launched agents |
| `use_multiprocess` | `bool` | Multiprocess mode flag |
| `contact_db` | `Option<Arc<dyn ContactStore>>` | Contact database |
| `crm` | `Option<Arc<CrmSync>>` | CRM synchronization |
| `task_type` | `TaskType` | Task type: Research or LeadGen |
| `target_count` | `Option<u32>` | Target contact count (for LeadGen) |
| `started_at` | `DateTime<Utc>` | Session start time |
| `session_cancel` | `CancellationToken` | Whole-session cancellation token |
| `agent_tokens` | `Arc<Mutex<HashMap<String, CancellationToken>>>` | Cancellation tokens for live agents |
| `steer_rx` | `Option<Arc<Mutex<UnboundedReceiver<String>>>>` | Steering channel |
| `role_llms` | `HashMap<String, Arc<dyn LlmProvider>>` | LLM providers by role |

### 3.2. Enum `TaskType`

```rust
enum TaskType { Research, LeadGen }
```

### 3.3. `Coordinator::new(...)`

**Algorithm:**
1. Reads `use_multiprocess` from configuration.
2. Calls `build_role_llms(&config)` to build the role → LLM map.
3. Initializes all fields with default values: `total_tokens = 0`, `total_agents = 0`, `task_type = Research`, `target_count = None`, `started_at = Utc::now()`, new `CancellationToken`, empty collections.

### 3.4. `build_role_llms(config) -> HashMap<String, Arc<dyn LlmProvider>>`

**Algorithm:**
1. Creates an empty `HashMap`.
2. For each pair `(role, model)` in `config.agent.role_models`:
   - Clones `config.llm`, replaces `model` with the specified one.
   - Calls `pr_llm::build_provider(&llm_cfg)`.
   - On success — logs and inserts `(role_lowercase, provider)` into the map.
   - On error — logs a warning and skips.
3. Returns the map.

### 3.5. `llm_for_role(&self, role) -> Arc<dyn LlmProvider>`

Maps the role to a string key, looks it up in `role_llms`. If not found — returns `self.llm.clone()` (default).

### 3.6. `budget_exhausted(&self) -> bool`

Returns `true` if `session_token_limit > 0` and `total_tokens >= session_token_limit`.

### 3.7. Builder Methods

- `with_steer_rx(rx)` — connects the steering channel.
- `cancel()` — cancels `session_cancel`.
- `cancel_token()` — clones the token.
- `set_cancel_token(token)` — replaces the token.
- `with_contact_db(db)` — connects the contact database.
- `with_crm(crm)` — connects CRM.

### 3.8. `emit(&self, event)`

Sends an event to the bus.

### 3.9. `start_heartbeat(db, session_id) -> HeartbeatGuard`

**Algorithm:**
1. Creates a `tokio::spawn` with an infinite loop.
2. Creates a `tokio::time::interval(60s)`.
3. The first tick is skipped (it is immediate).
4. On each subsequent tick: calls `db.touch_session(&session_id)`. On error — `break`.
5. Returns `HeartbeatGuard { handle }`, which calls `handle.abort()` on `Drop`.

**Purpose:** updates `sessions.updated_at` every 60 seconds, so that `SessionResumer` does not consider a live session interrupted (threshold — 5 minutes without activity).

### 3.10. `start_stall_monitor(event_rx, tokens, warn_secs, kill_secs, session_id) -> Option<StallMonitorGuard>`

**Algorithm:**
1. If `warn_secs == 0 && kill_secs == 0` — returns `None` (monitoring disabled).
2. Launches `stall_monitor_loop(event_rx, tokens, warn_secs, kill_secs, tick=30s, session_id)` in `tokio::spawn`.
3. Returns `StallMonitorGuard` (on `Drop` — abort).

### 3.11. `stall_monitor_loop(event_rx, tokens, warn_secs, kill_secs, tick, session_id)` — async

**Algorithm:**
1. Creates `last_progress: HashMap<String, Instant>` — last activity time of each agent.
2. Creates `warned: HashSet<String>` — agents that have already received a warning.
3. Creates an `interval` with period `tick` (30 seconds).
4. Infinite loop `tokio::select!`:
   - **Branch `event_rx.recv()`:**
     - On receiving an event: extracts `agent_id` from the event, updates `last_progress` (or inserts `now()`).
     - If the event is terminal (`SessionCompleted` / `SessionFailed`) **for the current session** — `break`.
     - On `Lagged` — ignores.
     - On `Closed` — `break`.
   - **Branch `interval.tick()`:**
     - Computes `now`.
     - For each `(agent_id, last)` in `last_progress`:
       - Computes `idle_secs = now - last`.
       - Gets the token from the `tokens` map.
       - If the token is already cancelled — `continue`.
       - If `kill_secs > 0 && idle_secs >= kill_secs` — logs error, cancels the token.
       - Else if `warn_secs > 0 && idle_secs >= warn_secs && !warned` — logs warning, adds to `warned`.

### 3.12. `build_researcher(&self, agent_id, parent_id, task, depth) -> AgentRuntime`

**Algorithm:**
1. Creates `AgentRuntime::new(...)` with role `Researcher`, LLM via `llm_for_role(Researcher)`.
2. Inherits `contact_db` and `crm`.
3. Sets `role_llms` via `with_role_llms`.

### 3.13. `run_with_timeout(&self, agent) -> Result<AgentOutput>` — async

**Algorithm:**
1. If `timeout_seconds == 0` — runs `agent.run()` without timeout.
2. Otherwise — wraps in `tokio::time::timeout(Duration::from_secs(timeout_seconds), agent.run())`.
3. On `Elapsed` — returns an error.

### 3.14. `spawn_researchers(&mut self, sub_tasks) -> Vec<AgentOutput>` — async

**Algorithm:**
1. Creates `findings: Vec<AgentOutput>` and `join_set: JoinSet`.
2. For each `task_desc` in `sub_tasks`:
   - Checks `total_agents >= max_agents` — if so, logs, updates subtask status as "skipped", `break`.
   - Checks `budget_exhausted()` — if so, similarly.
   - Generates `AgentId`, increments `total_agents`.
   - Creates `AgentRecord`, saves to DB.
   - Emits `AgentSpawned`.
   - Creates an agent via `build_researcher`.
   - Connects steering (if present).
   - Creates `child_token()` from `session_cancel`, registers in `agent_tokens`.
   - Sets `cancel_token`.
   - Launches in `join_set.spawn` with a `tokio::select!` wrapper:
     - Branch `run` — agent execution with timeout.
     - Branch `token.cancelled()` — updates DB status to `Cancelled`, returns error.
3. Collects results from `join_set.join_next()`:
   - `Ok(Ok(output))`: updates `total_tokens`, updates DB status (Completed or Failed if aborted), emits event, pushes to findings (if not aborted).
   - `Ok(Err(e))`: logs error.
   - `Err(e)`: logs task panic.
4. Clears `agent_tokens`.
5. Returns `findings`.

**Architecture and concurrency model:**

`spawn_researchers` uses `tokio::task::JoinSet` to manage the concurrent agent pool. Each researcher runs inside a `tokio::select!` that races two branches: the agent's `run()` method (with optional timeout) and a `token.cancelled()` branch. This design means:

- **Graceful cancellation:** when the stall monitor or user cancels a session, it cancels `session_cancel`, which propagates to all child tokens. Each agent's `select!` notices the cancelled token and returns an error instead of blocking.
- **Bounded concurrency:** `JoinSet` naturally collects results as they complete, regardless of submission order. The `max_agents` and `budget_exhausted()` checks prevent launching beyond configured limits.
- **Timeout integration:** `run_with_timeout` wraps each agent in a `tokio::time::timeout`. If an agent exceeds its timeout, the future returns `Err(Elapsed)`, which `spawn_researchers` treats as a failure — it logs the error, updates the DB, and continues collecting other results.

The `agent_tokens` map (a `HashMap<String, CancellationToken>` behind `Arc<Mutex<...>>`) is the stall monitor's interface for killing individual agents. The stall monitor's background loop can look up any agent's token in this map and cancel it if the agent has been idle too long.

### 3.15. `Coordinator::execute(&mut self) -> Result<SessionOutput>` — async — MAIN METHOD

**Step-by-step algorithm:**

#### Step 0: Initialization
1. Starts heartbeat (`start_heartbeat`).
2. Starts stall monitor (`start_stall_monitor`).
3. Emits `SessionStarted`.

**Design rationale:** The heartbeat and stall monitor run as background tokio tasks alongside the main execution. The heartbeat keeps the DB session row alive (updated_at every 60s), while the stall monitor watches the event bus for progress. If an agent hasn't emitted any event for `warn_secs` seconds, it logs a warning; if `kill_secs` passes without progress, it cancels that agent's token. This two-level approach means a single stuck shell command won't hold up the entire session.

#### Step 1: Plan
4. Calls `self.plan().await` — gets `sub_tasks: Vec<String>`.
5. Logs the number of subtasks.
6. For each subtask — calls `db.add_subtask()` (Goal Mode light, fleet E4).

**Planner design:** The `plan()` method uses the LLM itself (with a Coordinator role prompt) to decompose the user's query into 2-5 independent subtasks. For LeadGen tasks, the planner also detects the target contact count and structures subtasks as non-overlapping collection tasks partitioned by industry, name range, or source type. The planner outputs JSON, and the parser falls back gracefully to a single monolithic task if JSON parsing fails.

#### Step 2: Fan-out
7. If `sub_tasks` is empty — runs `run_single_agent()`.
8. If `use_multiprocess` — runs `run_multiprocess_fanout(&sub_tasks)`.
9. Otherwise — runs `spawn_researchers(&sub_tasks)` (in-process).

**Fan-out strategy:** The coordinator supports three modes:
- **In-process (`spawn_researchers`):** each researcher runs as a `tokio::spawn` task within the same process. This is the default — it's simple, fast, and shares the event bus and DB connection natively.
- **Multiprocess (`run_multiprocess_fanout`):** each researcher launches as a separate OS process (`pr-agent-worker` binary) communicating over Unix Domain Sockets. This provides stronger isolation: a crash in one worker doesn't bring down the others, and OS-level parallelism is maximized.
- **Single agent:** when planning produces no subtasks (fallback), the coordinator runs a single researcher directly.

The maximum number of concurrent agents is bounded by `config.agent.max_concurrent_children` (for in-process) and `max_agents` (overall limit). The session token budget is also checked before launching each agent.

#### Step 2.5: Sync subtask statuses
10. Calls `sync_subtask_statuses()` — synchronizes subtask string statuses with agent outcomes.

#### Step 2.6: Persist structured findings (fleet C4)
11. For each output and finding — calls `db.add_finding(finding)`.

#### Step 2.7: Reflection round for LeadGen (fleet C3)
12. If `task_type == LeadGen` and `target_count` is set:
    - Calls `contacts_saved_so_far()`.
    - If `saved < target` and `total_agents < max_agents`:
      - Computes `gap = target - saved`.
      - Forms a gap-filling task.
      - Runs `spawn_researchers(&[gap_task])`.
      - Persists findings, adds to the overall list.

**Reflection round rationale:** LeadGen tasks often have a numeric target (`"find 50 contacts"`). After the initial fan-out, the coordinator checks how many contacts were actually saved. If the target wasn't reached and there's headroom in the agent budget, it launches one more researcher with a gap-filling task. This is a light form of Goal Mode: the coordinator adapts the plan based on partial results.

#### Step 3: Synthesize
13. Calls `self.synthesize(&findings).await`.

#### Step 4: Write output
14. Calls `self.write_output(&synthesis, &findings)`.

#### Step 5: Complete
15. Calls `db.complete_session(...)`.
16. Emits `SessionCompleted`.
17. Returns `SessionOutput`.

### 3.16. `execute_resume(&mut self, state) -> Result<SessionOutput>` — async

**Algorithm:**
1. Starts heartbeat and stall monitor.
2. Emits `SessionStarted` (with `[resume]` prefix).
3. Restores `findings` from `state.completed_agents`.
4. For each restored finding — adds tokens to `total_tokens` and increments `total_agents`.
5. If there are `pending_tasks` — runs `spawn_researchers(&state.pending_tasks)`.
6. Calls `synthesize`, `write_output`, `complete_session`, `emit(SessionCompleted)`.
7. Returns `SessionOutput`.

### 3.17. `detect_task_type(query) -> TaskType`

**Algorithm:**
1. Converts the query to lowercase.
2. Checks for any of the markers: `"email"`, `"phone"`, `"контакт"`, `"лид"`, `"lead"`, `"ceo"`, `"cto"`, `"linkedin"` and others.
3. If at least one marker is found — `LeadGen`, otherwise `Research`.

### 3.18. `detect_target_count(query) -> Option<u32>`

**Algorithm:**
1. Converts the query to lowercase.
2. Defines an array of markers: `"email"`, `"контакт"`, `"contact"`, `"лид"`, `"lead"`, `"телефон"`, `"phone"`.
3. Scans the string byte-by-byte:
   - When encountering a digit — collects the full number.
   - Checks a 16-character window after the number for the presence of markers.
   - If a marker is found — parses the number. If `0 < n <= 10_000` — returns `Some(n)`.
4. If not found — `None`.

### 3.19. `plan(&mut self) -> Result<Vec<String>>` — async

**Algorithm:**
1. Determines `task_type` via `detect_task_type(&self.query)`.
2. Determines `target_count` via `detect_target_count(&self.query)`.
3. Forms the prompt:
   - For **LeadGen**: decomposition instruction into 2-5 non-overlapping contact collection tasks split by industry, name range, source type. Each task should contain goal description, tools, quota.
   - For **Research**: decomposition instruction into 2-5 independent research subtasks.
4. Creates a `CompletionRequest`:
   - System: `"You are a research planner.\n\n{role_prompt_for(Coordinator)}\n\nOutput only valid JSON."`
   - User: decomposition prompt.
   - temperature=0.3, max_tokens=2048, stream=false, tools=empty.
5. Calls `llm.complete(&req)`.
6. Response parsing:
   - Tries `serde_json::from_str::<Vec<String>>(text)`.
   - If unsuccessful — looks for `[` and `]` in the text, tries to parse the content between them.
7. Fallback: returns `vec![self.query.clone()]` (single task).

### 3.20. `run_single_agent(&mut self) -> Result<AgentOutput>` — async

**Algorithm:**
1. Creates `AgentId`, increments `total_agents`.
2. Creates `AgentRecord` (role=Researcher, depth=0), saves to DB.
3. Creates an agent via `build_researcher`, connects steering and cancel token.
4. Runs via `run_with_timeout`.
5. Wrapped in `tokio::select!` with a `token.cancelled()` branch:
   - On cancellation — updates DB status, emits `AgentFailed`, bail.
6. Updates `total_tokens`.
7. Updates DB status (Completed or Failed if aborted), emits event.
8. Returns `output`.

### 3.21. `run_multiprocess_fanout(&mut self, sub_tasks) -> Result<Vec<AgentOutput>>` — async

**Algorithm:**

#### Step 1: Spawn workers
1. Creates `ProcessManager::new(socket_dir)`, where `socket_dir = output_dir/.sockets`.
2. For each `task_desc`:
   - Checks `max_agents` and `budget_exhausted()`.
   - Creates `AgentId`, `AgentRecord`, saves to DB, emits `AgentSpawned`.
   - Calls `pm.spawn_worker(agent_id, session_id, task, role)`.
   - On success — saves `agent_id` to `worker_ids`.
   - On error — updates DB status, emits `AgentFailed`.

#### Step 2: Wait for completion
3. For each `agent_id` from `worker_ids`:
   - Calls `pm.wait_for_completion_with_events(&agent_id, Some(&event_tx))`.
   - Processes the result:
     - `Completed`: updates `total_tokens`, updates DB status, emits, pushes to findings.
     - `Failed`: logs, updates status, emits.
     - `Disconnected`: same as Failed.
     - `Err(e)`: same.

#### Step 3: Cleanup
4. Calls `pm.shutdown_all().await`.
5. Returns `findings`.

**Architecture and process isolation:**

Multiprocess fan-out is the alternative to in-process `spawn_researchers`. Its core value is **fault isolation**: each researcher runs in a completely separate OS process, so a crash, SIGKILL, or unbounded memory use in one researcher cannot corrupt the coordinator or its siblings. This is particularly valuable for shell-heavy research tasks or for running untrusted tool code.

The protocol is a **request/reply + event stream hybrid over a Unix Domain Socket**:
- The coordinator binds a socket at `socket_dir/{agent_id}.sock`, spawns the `pr-agent-worker` binary with the socket path, and waits up to 30 seconds for the worker to connect.
- Once connected, the worker sends `Ready` and then streams `Event` messages back to the coordinator (mirrored onto the coordinator's event bus via `to_agent_event`) and finally a `Result` message.
- The coordinator waits for the result with `wait_for_completion_with_events`, which runs until either the worker sends a `Result` or the child process exits unexpectedly (in which case it produces an `aborted: true` output, equivalent to a `Disconnected` status).
- `shutdown_all()` performs a graceful teardown: send `Cancel` to every worker, wait up to 5 seconds, then SIGKILL stragglers. This ensures no orphaned worker processes remain.

Event forwarding in multiprocess mode happens through `IpcMessage::Event` — see [section 8](#8-ipcrs) for the mapping and [section 9](#9-process_managerrs) for the process management details.

### 3.22. `synthesize(&self, findings) -> Result<String>` — async

**Algorithm:**
1. If `findings` is empty — returns "No findings were collected.".
2. Computes `headroom = context_window * 2`.
3. Creates `ResultBudget::new(headroom, findings.len(), spill_dir)`.
4. For each finding — limits via `budget.cap_result(&summary)`, forms `"### Finding {i}\n{capped}"`.
5. Forms the synthesis prompt:
   - System: `"You are a research synthesizer.\n\n{role_prompt_for(Writer)}"`
   - User: report writing instructions with 5 requirements (answer the query, integrate findings, note contradictions, list sources, indicate gaps).
6. `CompletionRequest` with temperature=0.5, max_tokens=from config.
7. Calls `llm.complete(&req)`.
8. Returns the response text.

### 3.23. `sync_subtask_statuses(&self)`

**Algorithm:**
1. Gets the list of session agents from DB.
2. For each agent:
   - Maps `status` to `"completed"` / `"failed"` / `"running"`.
   - Calls `db.update_subtask_status(session_id, task, status, summary)`.

### 3.24. `contacts_saved_so_far(&self) -> Option<u32>` — async

**Algorithm:**
1. If `contact_db` is not set — `None`.
2. Calls `store.list_all(i64::MAX, 0)`.
3. Filters contacts where `created_at >= self.started_at`.
4. Returns the count.

### 3.25. `write_output(&self, synthesis, findings) -> Result<()>`

**Algorithm:**
1. `create_dir_all(&output_dir)`.
2. **summary.md** — writes `synthesis`.
3. **index.md** — forms Markdown with metadata (query, date, agents, tokens) and links to finding files.
4. **findings/finding-{i}.md** — for each finding writes `summary`.
5. **sources.md** — collects unique sources from all `finding.sources`, forms a list `"- [title](url)"`. If no sources — `_No structured sources were recorded._`.

---

## 4. compaction.rs

**File:** `src/compaction.rs` — context compaction engine.

### Architecture Overview

The compaction engine is the system's primary defense against context window overflow. It operates in **two phases**: first a cheap, no-LLM micro-compaction that deduplicates and prunes, then (if needed) an LLM-driven summarization that condenses the middle of the conversation.

The key design insight is that the `compact()` method is **idempotent and non-fatal**. If the LLM summarization fails, the engine simply returns the original messages unchanged — the agent loop continues, and the next turn will try compaction again. This makes the compaction system resilient to transient LLM failures.

The engine also tracks **effectiveness**: if two consecutive compaction passes reduce the context by less than 5%, it enters a 300-second cooldown (`COOLDOWN_DURATION`). This prevents wasting tokens on fruitless compaction attempts (e.g., when the conversation is already as dense as possible).

### 4.1. Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `SUMMARIZATION_OVERHEAD_TOKENS` | 4000 | Token reserve for LLM summarization call |
| `MICRO_COMPACT_THRESHOLD_TOKENS` | 40000 | Token threshold for micro-compaction (without LLM) |
| `MAX_INEFFECTIVE_PASSES` | 2 | Maximum ineffective passes before cooldown |
| `COOLDOWN_DURATION` | 300 sec | Cooldown duration |

### 4.2. Structure `CompactionResult`

| Field | Type | Description |
|------|------|-------------|
| `messages` | `Vec<Message>` | Compacted messages |
| `tokens_before` | `u32` | Tokens before compaction |
| `tokens_after` | `u32` | Tokens after compaction |
| `cooldown_triggered` | `bool` | Whether cooldown was triggered |
| `micro_pruned` | `u32` | Number of pruned tool messages |
| `used_llm` | `bool` | Whether LLM summarization was used |

### 4.3. Structure `CompactionEngine`

| Field | Type | Description |
|------|------|-------------|
| `ineffective_passes` | `u32` | Counter of consecutive ineffective passes |
| `cooldown_until` | `Option<Instant>` | Cooldown end time |
| `estimated_tokens` | `u32` | Current token estimate |
| `config` | `ContextConfig` | Context configuration |

### 4.4. `CompactionEngine::new(config)`

Initializes all fields with zeros/`None`.

### 4.5. `set_estimated_tokens(&mut self, tokens)` / `estimated_tokens(&self)`

Setter/getter for `estimated_tokens`.

### 4.6. `should_compact(&self) -> bool`

**Algorithm:**
1. Computes `threshold = context_window * compact_threshold`.
2. Returns `estimated_tokens >= threshold`.

### 4.7. `is_in_cooldown(&self) -> bool`

**Algorithm:**
1. If `cooldown_until` is Some(until):
   - If `Instant::now() < until` — `true` (in cooldown).
   - Otherwise — `false` (cooldown expired).
2. If `None` — `false`.

### 4.8. `micro_compact(&self, messages) -> u32` — WITHOUT LLM

**Algorithm:**
1. `pruned = 0`, `running_tokens = 0`, `seen_content_hashes: HashSet<u64>`.
2. Iterates `messages.iter_mut()`:
   - For each `Message::Tool { content, .. }`:
     - Computes `tokens = estimate_tokens(content)`.
     - `running_tokens += tokens`.
     - **Deduplication:** computes `hash = content_hash(content)`.
       - If hash is already in `seen_content_hashes`:
         - Replaces `content` with `"[Duplicate tool result — {bytes} bytes, {tokens} tokens]"`.
         - `pruned += 1`. `continue`.
       - Otherwise — inserts hash into the set.
     - **Prune old outputs:** if `running_tokens > MICRO_COMPACT_THRESHOLD_TOKENS && tokens > 100`:
       - Replaces `content` with `"[Tool output pruned — {bytes} bytes, {tokens} tokens. Original output was from an earlier conversation turn.]"`.
       - `pruned += 1`.
3. Returns `pruned`.

### 4.9. `compact(&mut self, messages, summarize_fn) -> Result<CompactionResult>` — async

**Algorithm:**

#### Phase 0: Preparation
1. `tokens_before = estimate_messages_tokens(messages)`.
2. Checks `is_in_cooldown()` — if so, returns `CompactionResult` with `cooldown_triggered: true` and `tokens_after = tokens_before`.

#### Phase 1: Micro-compaction
3. `micro_pruned = self.micro_compact(messages)`.
4. `after_micro = estimate_messages_tokens(messages)`.
5. If `after_micro < threshold` — calls `update_effectiveness`, returns result (LLM not used).

#### Phase 2: Split head/middle/tail
6. Calls `split_head_middle_tail(messages)`.

#### Phase 3: LLM summarization of middle
7. If `middle` is not empty:
   - Converts middle to text via `messages_to_text(&middle)`.
   - Forms prompt: system = `SUMMARIZE_SYSTEM_PROMPT`, user = `"Summarize the following conversation section concisely...\n\n{middle_text}"`.
   - Calls `summarize_fn(prompt)`.
   - On error — `"[Compaction summarization failed: {e}. Middle section removed.]"`.
8. If middle is empty — `summary_text = ""`.

#### Phase 4: Recombination
9. Creates `compacted = Vec::with_capacity(head.len() + 2 + tail.len())`.
10. Adds `head`.
11. If `summary_text` is not empty — pushes `Message::system("[Context compaction — previous conversation summarized]\n\n{summary_text}")`.
12. Adds `tail`.
13. Computes `tokens_after`.
14. Calls `update_effectiveness(tokens_before, tokens_after)`.
15. Returns `CompactionResult`.

### 4.10. `update_effectiveness(&mut self, before, after)`

**Algorithm:**
1. `reduction = before.saturating_sub(after)`.
2. `threshold_5pct = before / 20`.
3. If `reduction < threshold_5pct` (less than 5% reduction):
   - `ineffective_passes += 1`.
   - If `ineffective_passes >= MAX_INEFFECTIVE_PASSES` (2):
     - Sets `cooldown_until = Some(Instant::now() + COOLDOWN_DURATION)`.
     - Resets `ineffective_passes = 0`.
4. Otherwise (effective pass):
   - `ineffective_passes = 0`.
   - `cooldown_until = None`.

### 4.11. `reset_cooldown(&mut self)`

Resets `ineffective_passes = 0` and `cooldown_until = None`.

### 4.12. `split_head_middle_tail(messages) -> (head, middle, tail)`

**Algorithm:**
1. If `messages.len() <= 7` — returns `(messages, [], [])` (too short to split).
2. `head_end = min(4, messages.len())` — first 4 messages (system + first 3).
3. `tail_start = messages.len().saturating_sub(4)` — last 4 messages.
4. **Tool-group safety:**
   - While `head_end < messages.len()` and `messages[head_end]` is a tool message: `head_end += 1` (pulls tool results into head).
   - While `tail_start > head_end` and `messages[tail_start]` is a tool message: `tail_start -= 1` (pushes tail start away from orphaned tool results).
5. If `tail_start <= head_end` — degenerate case: returns `(messages[..head_end], [], messages[head_end..])`.
6. Returns `(messages[..head_end], messages[head_end..tail_start], messages[tail_start..])`.

### 4.13. `messages_to_text(messages) -> String`

**Algorithm:**
For each message forms a string:
- `System` → `"[system]: {content}\n\n"`
- `User` → `"[user]: {content}\n\n"`
- `Assistant` → `"[assistant]: {content}\n\n"` + for each tool_call: `"[assistant tool_call]: {name}({arguments})\n\n"`
- `Tool` → `"[tool result (id={id})]: {content}\n\n"` (content truncated to 2000 chars at UTF-8 boundary).

### 4.14. `content_hash(s) -> u64`

Creates a `DefaultHasher`, hashes the string via the `Hash` trait, returns `hasher.finish()`.

### 4.15. Summarization System Prompt (`SUMMARIZE_SYSTEM_PROMPT`)

Structured by sections:
- **Goal** — what the agent was trying to do.
- **Done** — key findings and completed actions.
- **Blocked** — errors, blockers, unresolved issues.
- **Next** — planned next steps.

Instruction: be concise, preserve facts, URLs, numbers, remove "fluff".

---

## 5. prompt.rs

**File:** `src/prompt.rs` — system prompt builder with a three-tier architecture (stable/context/volatile).

### 5.1. Prompt Constants

- `DEFAULT_PROMPT_BASE` — loaded from `prompts/default.txt` via `include_str!`.
- `DEEPSEEK_PROMPT_BASE` — loaded from `prompts/deepseek.txt`.
- Role blocks: `ROLE_COORDINATOR`, `ROLE_RESEARCHER`, `ROLE_ANALYST`, `ROLE_VERIFIER`, `ROLE_WRITER` — inline strings.

### 5.2. `build_env_block(config, working_dir) -> String`

**Algorithm:**
1. Determines `platform` via `cfg!(target_os = ...)` → `"darwin"` / `"linux"` / `"windows"` / `"unknown"`.
2. Determines `is_git` — checks if `working_dir.join(".git")` exists.
3. Gets `today = Utc::now().format("%Y-%m-%d")`.
4. Forms the block:
   ```
   <env>
   Working directory: {path}
   Is git repo: {yes/no}
   Platform: {platform}
   Model: {model}
   Date: {date}
   </env>
   ```

### 5.3. Structure `PromptBuilder`

Three-tier builder:
- `stable: Vec<String>` — stable layer (cached between sessions).
- `context: Vec<String>` — context layer (stable within a session).
- `volatile: Vec<String>` — volatile layer (changes every turn).

### 5.4. `PromptBuilder::new(role, task, depth, max_depth, model)`

**Algorithm:**
1. Selects the base prompt via `select_model_base(model)`.
2. Gets the role prompt via `role_prompt_for(role)`.
3. Forms the stable layer: `[base, "## Your Role\n{role}\n\n## Current Task\n{task}\n\nDepth: {depth}/{max_depth}"]`.

### 5.5. `add_env(&mut self, config, working_dir)`

Calls `build_env_block()`, pushes the result into `context`.

### 5.6. `add_tools(&mut self, tools)`

**Algorithm:**
1. If `tools` is empty — does nothing.
2. Forms the section `"## Available Tools\n\n"`.
3. For each tool: `"### {name}\n{description}\n\n"`.
4. Pushes into `volatile`.

### 5.7. `add_stable_instruction(&mut self, instruction)` / `add_volatile_block(&mut self, block)`

Simply pushes the string into the corresponding layer.

### 5.8. `build(&self) -> String`

Concatenates all layers in order: stable → context → volatile, separated by `"\n\n"`.

### 5.9. `tier_counts(&self) -> (usize, usize, usize)`

Returns the number of sections in each layer.

### 5.10. `select_model_base(model) -> &'static str`

**Algorithm:**
1. Converts `model` to lowercase.
2. If it contains `"deepseek"` — returns `DEEPSEEK_PROMPT_BASE`.
3. Otherwise — `DEFAULT_PROMPT_BASE`.

### 5.11. `role_prompt_for(role) -> &'static str`

Returns a static string for each role:
- **Coordinator** — instructions on decomposition, delegation, result collection, synthesis.
- **Researcher** — search workflow (search → fetch → extract → record), strategy, source hierarchy, OSINT/lead-gen workflow, safety rules.
- **Analyst** — analysis framework (cross-reference, patterns, contradictions, reliability assessment).
- **Verifier** — verification approach (independent sources, primary sources, consensus, statuses VERIFIED/LIKELY/UNVERIFIED/CONTRADICTED).
- **Writer** — report writing guide (structure, formatting, quality standards).

---

## 6. prompts/

### 6.1. `prompts/default.txt`

Base prompt for most models (GPT-4, Claude, etc.). Contains:
- Identification: "You are an autonomous research agent working within the Fathom system."
- General instructions: use tools, don't fabricate, be accurate, record issues.
- Behavioral guidelines: think step by step, break down tasks, try alternatives, use markdown.

### 6.2. `prompts/deepseek.txt`

Abbreviated version for DeepSeek — more concise, with a focus on tools. Same core principles, but without verbose explanations.

---

## 7. hooks.rs

**File:** `src/hooks.rs` — lifecycle hooks (fleet E3, ZCode pattern). Subprocesses invoked at specific stages of the agent cycle.

### Architecture Overview

The hook system implements a **subprocess-based extension mechanism** inspired by the ZCode pattern. At three points in the agent lifecycle — before a tool runs, after a tool runs, and when the LLM wants to stop — the runtime invokes a user-configured subprocess command, passes a JSON payload on stdin, and parses the JSON response from stdout.

This design is deliberately **process-boundary**: hooks run as separate OS processes, not as in-process plugins. This means:
- **No language lock-in.** Hooks can be written in any language (bash, Python, Go, Node.js) — the protocol is JSON over stdin/stdout.
- **No runtime crashes.** A buggy hook that segfaults or panics won't take down the agent.
- **30-second timeout.** Every hook call is wrapped in a `tokio::time::timeout` with `kill_on_drop`. If a hook hangs, it's killed and treated as "no response" (the agent continues).
- **Wildcard matching.** A hook with `tool: ""` (empty string) matches any tool, while a hook with a specific tool name only matches that tool. This allows broad safety policies (e.g., "deny all shell commands") alongside tool-specific hooks.

The three hook points serve different purposes:

| Hook Point | When | Use Case |
|------------|------|----------|
| `pre_tool_use` | Before tool execution | Security gate: deny dangerous commands, enforce rate limits, validate arguments |
| `post_tool_use` | After tool execution | Audit log, save output snapshots, notify external systems |
| `stop` | When LLM wants to stop | Quality gate: force the agent to continue if its output is incomplete |

### 7.1. Constants

- `MAX_STOP_CONTINUATIONS = 3` — maximum number of forced continuations by Stop hooks per run.

### 7.2. Enums

#### `PreToolVerdict`
```rust
enum PreToolVerdict { Allow, Deny(String) }
```

#### `StopVerdict`
```rust
enum StopVerdict { Stop, Continue(String) }
```

### 7.3. `hooks_for(hooks, event, tool) -> Vec<&HookConfig>`

**Algorithm:**
1. Filters hooks where `h.event` matches `event` (case-insensitive).
2. Further filters: `h.tool` is empty (wildcard — matches any tool) **OR** `h.tool` matches `tool` (case-insensitive).
3. Returns the filtered vector of references.

**Usage:** used to filter PreToolUse, PostToolUse, and Stop hooks by event and tool name.

### 7.4. `run_hook(cmd, input_json) -> Option<String>` — async

**Algorithm:**
1. Gets the shell path: `SHELL` from environment (fallback `/bin/bash`).
2. Creates `Command::new(&shell_path)` with arguments `["-c", cmd]`.
3. Sets `stdin` to `piped`.
4. Captures `stdout` and `stderr` as `piped`.
5. Tries `cmd.spawn()`:
   - On error — logs warning, returns `None`.
6. Via `child.stdin.take()` writes `input_json` + newline, then closes stdin.
7. Creates a future `child.wait_with_output()`.
8. Wraps in `tokio::time::timeout(Duration::from_secs(30), output_future)`.
9. On `Elapsed` — calls `child.kill().await` (kill_on_drop), logs warning, returns `None`.
10. On timeout error — logs warning, returns `None`.
11. On `Ok(Ok(output))`:
    - If `!output.status.success()` — logs stderr, returns `None`.
    - Concatenates stdout + stderr → `all`.
    - Removes trailing newline (if present).
    - If `all.is_empty()` — returns `None`.
    - Otherwise — returns `Some(all)`.

### 7.5. `run_pre_tool_hooks(hooks, tool_name, tool_args_json) -> PreToolVerdict` — async

**Algorithm:**
1. Filters hooks: `hooks_for(hooks, "pre_tool_use", tool_name)`.
2. If no matching hooks — returns `PreToolVerdict::Allow`.
3. Forms input JSON: `{ "event": "pre_tool_use", "tool": tool_name, "args": {tool_args_json}, "timestamp": iso8601 }`.
4. For each hook from the filtered list:
   - Calls `run_hook(&h.cmd, &input_json).await`.
   - If `run_hook` returned `None` — `continue` (hook does not block).
   - Parses stdout as JSON.
   - Extracts `verdict` (string).
   - If `verdict == "deny"`:
     - Extracts `reason` (string, default: `"blocked by hook"`).
     - Returns `PreToolVerdict::Deny(reason)`.
5. If no hook returned deny — `PreToolVerdict::Allow`.

**Input JSON format:**
```json
{
  "event": "pre_tool_use",
  "tool": "shell",
  "args": {"command": "ls -la"},
  "timestamp": "2024-01-15T10:30:00Z"
}
```

**Response JSON format:**
```json
{
  "verdict": "deny",
  "reason": "shell commands are not allowed"
}
```

### 7.6. `run_post_tool_hooks(hooks, tool_name, tool_args_json, tool_output, success) -> Option<String>` — async

**Algorithm:**
1. Filters hooks: `hooks_for(hooks, "post_tool_use", tool_name)`.
2. If no matching hooks — returns `None`.
3. Forms input JSON: `{ "event": "post_tool_use", "tool": tool_name, "args": {tool_args_json}, "output": tool_output, "success": bool, "timestamp": iso8601 }`.
4. For each hook:
   - Calls `run_hook(&h.cmd, &input_json).await`.
   - If `Some(extra_info)` — appends to `accumulated_output` with `"\n\n[Hook {i+1}]: {extra_info}"`.
5. If `accumulated_output` is not empty — returns `Some(accumulated_output)`.
6. Otherwise — `None`.

**Input JSON format:**
```json
{
  "event": "post_tool_use",
  "tool": "fetch_url",
  "args": {"url": "https://example.com"},
  "output": "<html>...</html>",
  "success": true,
  "timestamp": "2024-01-15T10:30:05Z"
}
```

**Response JSON format:**
```json
{
  "verdict": "ok",
  "extra_info": "Saved snapshot to /tmp/snapshots/example.html"
}
```

### 7.7. `run_stop_hooks(hooks, summary_so_far) -> StopVerdict` — async

**Algorithm:**
1. Filters hooks: `hooks_for(hooks, "stop", "")`.
2. If no matching hooks — returns `StopVerdict::Stop`.
3. Computes `char_count = summary_so_far.chars().count()`.
4. Forms input JSON: `{ "event": "stop", "tool": null, "output_preview": {preview up to 4000 chars}, "output_char_count": char_count, "timestamp": iso8601 }`.
5. For each hook:
   - Calls `run_hook(&h.cmd, &input_json).await`.
   - If `None` — `continue`.
   - Parses stdout as JSON.
   - Extracts `verdict`.
   - If `verdict == "continue"`:
     - Extracts `reason` (default: `"hook requested continuation"`).
     - Returns `StopVerdict::Continue(reason)`.
6. If no hook returned continue — `StopVerdict::Stop`.

**Input JSON format:**
```json
{
  "event": "stop",
  "tool": null,
  "output_preview": "The analysis shows that...",
  "output_char_count": 15000,
  "timestamp": "2024-01-15T10:35:00Z"
}
```

**Response JSON format:**
```json
{
  "verdict": "continue",
  "reason": "Need to verify claims with at least one more source"
}
```

### 7.8. Structure `HookConfig`

| Field | Type | Description |
|------|------|-------------|
| `cmd` | `String` | Shell command to execute |
| `event` | `String` | Event type: `"pre_tool_use"`, `"post_tool_use"`, `"stop"` |
| `tool` | `String` | Tool name (empty string = wildcard) |

---

## 8. ipc.rs

**File:** `src/ipc.rs` — inter-process communication protocol between coordinator and worker processes via Unix Domain Socket.

### Architecture Overview

The IPC protocol is the backbone of the multiprocess fan-out mode. It uses a **simple JSON-line protocol over a Unix Domain Socket** (UDS): each message is a single line of JSON terminated by `\n`. This is intentionally simpler than gRPC or a full message bus — it requires no external dependencies, no schema registry, and the protocol is trivially debuggable (`echo '{"Ready":null}' | nc -U socket.sock`).

The protocol has **6 message types**, 3 from coordinator → worker and 3 from worker → coordinator:

| Direction | Message | Purpose |
|-----------|---------|---------|
| Coordinator → Worker | `Task` | Assigns the research task to the worker |
| Coordinator → Worker | `Cancel` | Requests early termination |
| Coordinator → Worker | `Ack` | Acknowledges a received result |
| Worker → Coordinator | `Ready` | Signals that the worker has initialized |
| Worker → Coordinator | `Result` | Returns the final output (status, summary, tokens) |
| Worker → Coordinator | `Event` | Streams a live `AgentEvent` for real-time monitoring |

The `Event` forwarding mechanism is what makes the multiprocess mode transparent to the TUI and stall monitor: the worker serializes each `AgentEvent` as an `IpcMessage::Event`, the coordinator deserializes it and re-broadcasts it on its own event bus. This is handled by `to_agent_event()` in [section 8.3](#83-to_agent_eventipc_msg-session_id---option).

### 8.1. Enum `IpcMessage`

All variants are serialized/deserialized via `serde`:

#### `IpcMessage::Task { agent_id, session_id, task, role }`
Purpose: coordinator → worker. Sends the task to the worker.

**JSON format:**
```json
{
  "Task": {
    "agent_id": "550e8400-e29b-41d4-a716-446655440000",
    "session_id": "660e8400-e29b-41d4-a716-446655440001",
    "task": "Research quantum computing advances in 2024",
    "role": "Researcher"
  }
}
```

#### `IpcMessage::Result { agent_id, status, summary, tokens_used, error }`
Purpose: worker → coordinator. Returns the execution result.

**JSON format:**
```json
{
  "Result": {
    "agent_id": "550e8400-e29b-41d4-a716-446655440000",
    "status": "Completed",
    "summary": "Found 5 key advances in quantum computing...",
    "tokens_used": 15000,
    "error": null
  }
}
```

#### `IpcMessage::Cancel { agent_id }`
Purpose: coordinator → worker. Cancellation request.

**JSON format:**
```json
{
  "Cancel": {
    "agent_id": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

#### `IpcMessage::Ready`
Purpose: worker → coordinator. Readiness signal after startup.

**JSON format:**
```json
"Ready"
```

#### `IpcMessage::Ack { agent_id, message }`
Purpose: coordinator → worker. Acknowledgment with an arbitrary message.

**JSON format:**
```json
{
  "Ack": {
    "agent_id": "550e8400-e29b-41d4-a716-446655440000",
    "message": "Task received"
  }
}
```

#### `IpcMessage::Event { event }`
Purpose: worker → coordinator. Forwards an `AgentEvent` to the coordinator's event bus.

**JSON format:**
```json
{
  "Event": {
    "event": { /* AgentEvent serialized */ }
  }
}
```

### 8.2. `write_msg(stream, msg) -> Result<()>` — async

**Algorithm:**
1. Serializes `msg` to JSON via `serde_json::to_string(msg)?`.
2. Appends a newline (`\n`) at the end.
3. Calls `stream.write_all(json.as_bytes()).await?`.

### 8.3. `to_agent_event(ipc_msg, session_id) -> Option<AgentEvent>`

**Algorithm:**
Matches by `IpcMessage` variant:

| IpcMessage | AgentEvent |
|------------|------------|
| `SessionStarted { session_id }` | `SessionStarted { session_id }` |
| `SessionCompleted { session_id, total_tokens }` | `SessionCompleted { session_id, total_tokens, total_agents: 0 }` |
| `SessionFailed { session_id, error }` | `SessionFailed { session_id, error }` |
| `AgentSpawned { agent_id, task }` | `AgentSpawned { agent_id, parent_id: None, task, depth: 0, role: Researcher }` |
| `AgentCompleted { agent_id, tokens_used }` | `AgentCompleted { agent_id, summary: "".into(), tokens_used, findings: vec![], aborted: false }` |
| `AgentFailed { agent_id, error }` | `AgentFailed { agent_id, error }` |
| `LlmStreamChunk { agent_id, content }` | `LlmStreamChunk { agent_id, content }` |
| `ToolCallStarted { agent_id, tool_name, tool_args }` | `ToolCallStarted { agent_id, tool_name, tool_args, call_id: "".into() }` |
| `ToolCallCompleted { agent_id, tool_name, duration_ms, preview }` | `ToolCallCompleted { agent_id, tool_name, duration_ms, preview }` |
| `AgentStateChanged { agent_id, state }` | `AgentStateChanged { agent_id, state }` |

**Note:** several fields are filled with default values since IPC messages contain a simplified set of fields.

---

## 9. process_manager.rs

**File:** `src/process_manager.rs` — worker process management for multiprocess fan-out.

### Architecture Overview

`ProcessManager` is the process lifecycle owner in multiprocess mode. It is responsible for the complete worker lifecycle: **spawn → handshake → stream events → await result → graceful shutdown**. It tracks two maps — `children` (the OS `Child` handles) and `streams` (the write halves of the UDS connections) — keyed by `agent_id`.

The design ensures that no worker is ever left orphaned:
- During spawn, a 30-second connection timeout guards the UDS handshake.
- During shutdown, the sequence is **Cancel → 5s grace → SIGKILL → reap**. `children` is drained so terminated processes are properly reaped (avoiding zombie processes).
- Event reading saturates at 16MB per IPC line (`read_line_capped`), protecting against OOM from a malicious or malfunctioning worker.

### 9.1. Structure `ProcessManager`

| Field | Type | Purpose |
|------|------|---------|
| `socket_dir` | `PathBuf` | Directory for Unix Domain Socket files |
| `children` | `HashMap<AgentId, Child>` | Live child processes |
| `streams` | `HashMap<AgentId, OwnedWriteHalf>` | Active write streams to workers |

### 9.2. `ProcessManager::new(socket_dir)`

**Algorithm:**
1. `create_dir_all(&socket_dir)`.
2. Initializes empty `children` and `streams`.

### 9.3. `spawn_worker(&mut self, agent_id, session_id, task, role) -> Result<()>` — async

**Algorithm:**
1. Forms `socket_path = socket_dir/{agent_id}.sock`.
2. Deletes the old socket file if it exists (`fs::remove_file`).
3. Creates `UnixListener::bind(&socket_path)?`.
4. Determines the path to the `pr-agent-worker` binary (looks next to the current binary or in `$CARGO_MANIFEST_DIR/../../target/release|debug/`).
5. If the binary is not found — `bail!`.
6. Creates `Command::new(&worker_binary)`:
   - Arguments: `["--socket", &socket_path, "--agent-id", &agent_id, "--session-id", &session_id, "--task", &task, "--role", &role]`.
   - `stdin(Stdio::null())`, `stdout(Stdio::piped())`, `stderr(Stdio::piped())`.
7. Launches via `.spawn()?`.
8. Saves `child` to `children`.
9. Runs `tokio::time::timeout(30s, listener.accept())` — waits for the worker to connect.
10. On `Elapsed` — `bail!("Worker {agent_id} did not connect within 30s")`.
11. On successful `accept` — gets `(stream, _addr)`.
12. Creates `tokio::io::split(stream)` → `(read_half, write_half)`.
13. Saves `write_half` to `streams`.
14. Launches `tokio::spawn(Self::stream_events(read_half, event_tx))` to read IPC messages from the worker stream.
15. Logs successful launch.

### 9.4. `send_to(&mut self, agent_id, msg) -> Result<()>` — async

**Algorithm:**
1. Looks up `stream` in `streams` by `agent_id`.
2. If not found — `bail!("No stream for agent {agent_id}")`.
3. Calls `write_msg(stream, msg).await`.

### 9.5. `shutdown_all(&mut self) -> Result<()>` — async

**Algorithm:**
1. For each `(agent_id, stream)` in `streams`:
   - Tries to send `IpcMessage::Cancel { agent_id }`.
   - Errors are ignored (the worker may have already terminated).
2. Clears `streams`.
3. For each `(agent_id, mut child)` in `children`:
   - Runs `tokio::time::timeout(5s, child.wait())`.
   - On `Elapsed` — logs warning, calls `child.kill().await`.
4. Clears `children`.

**Sequence: Cancel → grace 5s → kill → reap.**

### 9.6. `wait_for_completion_with_events(&mut self, agent_id, event_tx) -> Result<AgentOutput>` — async

**Algorithm:**
1. Extracts `write_half` from `streams` (or bail).
2. Sends `IpcMessage::Ack` with message `"waiting for completion"`.
3. Creates `BufReader::new(read_half)`.
4. Infinite loop `tokio::select!`:
   - **Branch `read_line_capped(&mut reader, 16MB)`:**
     - Deserializes the line as `IpcMessage`.
     - **`Ready`** — logs.
     - **`Event { event }`** — if `event_tx` is set, forwards via `to_agent_event()` and `event_tx.send()`.
     - **`Result { status, summary, tokens_used, error }`** — forms `AgentOutput` based on status (Completed / Failed / Disconnected), returns.
     - **Read error** — logs, returns `AgentOutput` with `aborted: true`.
   - **Branch `child.wait()` (via mutable access):**
     - If the process terminated before receiving Result — logs, returns `AgentOutput` with `aborted: true`.

### 9.7. `stream_events(read_half, event_tx)`

**Algorithm:**
1. Creates `BufReader::new(read_half)`.
2. Loop `read_line_capped(&mut reader, 16MB)`:
   - If `Ok(line)` — deserializes, forwards via `to_agent_event()`, sends to `event_tx`.
   - If `Err(e)` — logs, break.
3. Logs stream termination.

### 9.8. `read_line_capped(reader, max_bytes) -> Result<String>`

**Algorithm:**
1. Creates buffer `String::new()`.
2. Loop `reader.read_line(&mut line)`:
   - If `Ok(0)` — EOF, `bail!("unexpected EOF")`.
   - If `Ok(_)` — checks `line.len() > max_bytes`:
     - If yes — `bail!("IPC line too long: {len} bytes (max {max_bytes})")`.
   - Checks `line.ends_with('\n')`:
     - If yes — removes trailing newline (and `\r` if present), returns the string.
3. On read error — `bail!`.

**Purpose:** protection against OOM when reading from the IPC stream (workers should not send messages > 16MB).

---

## 10. background.rs

**File:** `src/background.rs` — background task management for the agent (fleet E2).

### Architecture Overview

The background task system lets an agent **fire-and-forget non-blocking work**. When the `spawn_agent` tool is invoked with `"background": true`, the runtime wraps the child in a background task instead of awaiting it. The results are collected asynchronously and injected into the parent's conversation at the next turn boundary (via the `bg_results` buffer that `AgentRuntime::run` drains in step 1b).

The concurrency model is a simple **job vector + token handle** design:

- The `BackgroundManager` owns a list of `BackgroundJob`s, each wrapping a `JoinHandle<()>`. The child's `run()` future is spawned detached (`tokio::spawn`); the spawned task computes the result and pushes it to the shared `bg_results: Arc<Mutex<Vec<...>>>`.
- `poll_completed()` is invoked by the parent (typically between turns or during `agent.run()`) to check `JoinHandle::is_finished()` and harvest results with their elapsed duration.
- `cancel_all()` aborts every living handle — used during shutdown so background work doesn't outlive the session.

The key coordination point is the `bg_results` channel. Because it is an `Arc<Mutex<Vec<_>>>`, it is shareable between the spawned background task (which appends results) and the agent runtime (which drains them at the next turn boundary via `std::mem::take`). Drain happens at the start of each turn, so results appear in the conversation exactly when the model next composes a response.

### 10.1. Structure `BackgroundJob`

| Field | Type | Description |
|------|------|-------------|
| `id` | `AgentId` | Background agent identifier |
| `label` | `String` | Human-readable label (task, truncated to 30 chars) |
| `handle` | `JoinHandle<()>` | Async task handle |
| `started_at` | `Instant` | Start time |

### 10.2. Structure `BackgroundManager`

| Field | Type | Description |
|------|------|-------------|
| `jobs` | `Vec<BackgroundJob>` | List of active background tasks |

### 10.3. `BackgroundManager::new()`

Creates an empty vector `jobs`.

### 10.4. `spawn(&mut self, label, agent_id, child, bg_results)` — async

**Algorithm:**
1. Clones `bg_results` (Arc).
2. Launches `tokio::spawn`:
   - Calls `child.run().await`.
   - On success `output`:
     - Computes `total = output.tokens_used + output.descendant_tokens`.
     - Forms `capped_summary` — truncates `output.summary` to 4000 chars at UTF-8 boundary.
     - Acquires lock on `bg_results`, pushes `(label, Ok(capped_summary), total)`.
   - On error `e`:
     - Acquires lock on `bg_results`, pushes `(label, Err(e.to_string()), 0)`.
3. Saves `BackgroundJob { id: agent_id, label: label.to_string(), handle, started_at: Instant::now() }` to `jobs`.

### 10.5. `poll_completed(&mut self) -> Vec<(AgentId, String, Duration)>`

**Algorithm:**
1. Creates `completed: Vec`.
2. Iterates `jobs` by index:
   - For each `job` checks `job.handle.is_finished()`.
   - If finished:
     - Computes `elapsed = job.started_at.elapsed()`.
     - Pushes `(job.id, job.label, elapsed)`.
3. Removes completed jobs from `jobs` (iteration in reverse order for correct index removal).
4. Returns `completed`.

### 10.6. `cancel_all(&self)`

**Algorithm:**
For each `job` in `jobs` — calls `job.handle.abort()`.

### 10.7. `active_count(&self) -> usize`

Returns `jobs.len()`.

### 10.8. `is_empty(&self) -> bool`

Returns `jobs.is_empty()`.

---

## 11. budget.rs

**File:** `src/budget.rs` — tool result budget. Limits output volume per turn and per run.

### Architecture Overview

The budget system is a **two-layer output limiter** that prevents any single tool result or turn from overflowing the context window.

1. **`TurnBudget`** — per-turn byte budget. Reset at the start of every turn (step 1d of the main loop). Each tool result's content is charged against this budget via `record()`. If the budget is exhausted, subsequent tool results are dropped entirely with a `"[Tool output dropped: turn output budget exhausted]"` message.
2. **`ResultBudget`** — per-result character budget, used during spawn batch result collection and synthesis. It divides the available headroom evenly among the batch and spills oversized results to disk.

When a result exceeds the byte or line limit, `apply_turn_budget` performs a **three-step truncation**:
1. **Line truncation** — if lines exceed the limit, the middle is removed and replaced with a note.
2. **Byte truncation** — if bytes exceed the limit, the content is truncated at a UTF-8-safe boundary.
3. **Spill to disk** — the full original content is saved to `{working_dir}/.pr-context/spills/{agent_id}/{tool_name}_{timestamp}_{hash}.txt`. The truncated message includes a spill path and first 500 characters, so the agent can re-read the full output with `read_file` if needed.

The spill directory is capped at `MAX_SPILL_FILES_PER_AGENT` (20) to prevent unbounded disk usage.

### 11.1. Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `DEFAULT_MAX_BYTES` | 250,000 | Maximum bytes per result |
| `DEFAULT_MAX_LINES` | 5,000 | Maximum lines per result |
| `DEFAULT_SHELL_MAX_BYTES` | 200,000 | Maximum for shell output |
| `DEFAULT_SHELL_MAX_LINES` | 4,000 | Maximum lines for shell |
| `SPILL_DIR_NAME` | `.pr-context` | Directory name for spill files |
| `MAX_SPILL_FILES_PER_AGENT` | 20 | Maximum spill files |

### 11.2. Structure `Truncated`

```rust
enum Truncated {
    Unchanged,
    Truncated { replacement: String },
}
```

### 11.3. `Truncated::content(&self, original) -> &str`

Returns `replacement` if truncated, otherwise `original`.

### 11.4. `apply_turn_budget(tool_name, result, max_bytes, max_lines, turn_budget, working_dir) -> Truncated`

**Algorithm:**
1. Copies `result.content` to `content`.
2. Determines `bytes_budget` and `lines_budget`:
   - If `tool_name == "shell"` — uses `min(max_bytes, DEFAULT_SHELL_MAX_BYTES)` and `min(max_lines, DEFAULT_SHELL_MAX_LINES)`.
   - Otherwise — uses `min(max_bytes, DEFAULT_MAX_BYTES)` and `min(max_lines, DEFAULT_MAX_LINES)`.
3. **Budget check:** if `turn_budget.remaining() < bytes_budget` — uses `min(turn_budget.remaining(), bytes_budget)` as the new `bytes_budget`.
4. If `bytes_budget == 0` — returns `Truncated::Truncated` with message `"[Tool output dropped: turn output budget exhausted]"`.
5. **Truncate by lines:**
   - Splits `content` by newline.
   - If `lines.len() > lines_budget` — truncates to `lines_budget`, appends `"\n\n[Truncated — {N} lines omitted. Re-run with the 'offset' argument to continue.]"`.
   - Updates `content`.
6. **Truncate by bytes:**
   - If `content.len() > bytes_budget`:
     - Truncates at UTF-8 boundary.
     - Records `full_len = content.len()`.
     - Records hash of original content: `{tool_name}:{content_hash:x}:{full_len}`.
     - **Spill to disk:**
       - Creates directory `{working_dir}/{SPILL_DIR_NAME}/{agent_id}`.
       - Counts existing spill files (glob `*.txt`).
       - If < `MAX_SPILL_FILES_PER_AGENT`:
         - Forms name `{tool_name}_{timestamp}_{hash}.txt`.
         - Writes full content to file.
         - Logs.
       - If >= limit — logs warning (does not save).
     - Replaces `content` with:
       ```
       [Tool output truncated — {full_len} bytes exceeded budget ({bytes_budget} bytes).

       The full output was saved to disk at:
         {spill_path}

       To access it, use the `read_file` tool with that path.

       First 500 characters:]
       {first_500_chars}
       ```
     - Updates metadata: `spill_path`, `content_hash`, `truncated`, `spilled`, `original_bytes`.
7. **Budget accounting:** `turn_budget.record(content.len() as u64)`.
8. Returns `Truncated::Truncated { replacement: content }` or `Truncated::Unchanged`.

### 11.5. Structure `TurnBudget`

| Field | Type | Description |
|------|------|-------------|
| `limit_bytes` | `u64` | Total per-turn limit |
| `used_bytes` | `u64` | Bytes consumed |

### 11.6. `TurnBudget::new(limit)` / `record(bytes)` / `remaining()`

- `new(limit)` — initializes with `used_bytes = 0`.
- `record(bytes)` — increments `used_bytes`.
- `remaining()` — returns `limit_bytes.saturating_sub(used_bytes)`.

### 11.7. Structure `ResultBudget`

| Field | Type | Description |
|------|------|-------------|
| `headroom_chars` | `usize` | Free context space in characters |
| `per_result_chars` | `usize` | Limit per result |
| `spill_dir` | `PathBuf` | Directory for spill files |
| `agent_dir` | `PathBuf` | Spill file directory for a specific agent |

### 11.8. `ResultBudget::new(headroom, batch_len, spill_dir)`

**Algorithm:**
1. `per_result_chars = headroom / max(batch_len, 1)`. Minimum 4000.
2. `agent_dir = spill_dir/agent`.
3. `create_dir_all(&agent_dir)`.
4. Counts existing spill files: `existing = agent_dir.read_dir().count()`.
5. If `existing >= 20` — logs warning (spill disabled).

### 11.9. `cap_result(&self, text) -> String`

**Algorithm:**
1. If `text.len() <= per_result_chars` — returns `text.to_string()`.
2. Computes boundary at `per_result_chars` (at UTF-8 boundary).
3. Creates spill file `{agent_dir}/finding_{timestamp}_{short_hash}.txt`.
4. Writes full text to file.
5. Logs.
6. Returns truncated text + suffix:
   ```
   ... [truncated — {total} chars; full text saved to: {spill_path}].
   ```

---

## 12. doom_loop.rs

**File:** `src/doom_loop.rs` — agent loop detection.

### Architecture Overview

The doom loop detector is a **sliding-window pattern matcher** that identifies when an agent is repeatedly calling the same tool with identical arguments. This is a common failure mode for LLMs: they get stuck in a loop where they call `read_file` on the same path, or `search_web` with the same query, over and over.

The detector works by maintaining a sliding window of the last 6 invocations. It normalizes argument JSON using `BTreeMap` (so `{"a":1,"b":2}` and `{"b":2,"a":1}` produce the same hash), then checks for 3+ consecutive identical calls. On detection, it returns a warning string that the main loop uses to issue a "nudge" — a message injected into the conversation telling the model to try a different approach.

The system has two escalation levels:
1. **Nudge** (first detection): the agent is warned but allowed to continue. The `doom_nudged` flag is set.
2. **Stop** (second detection after nudge): the agent is aborted with a doom loop warning. The `MAX_NUDGE_CONTINUATIONS` (3) limit in `has_exceeded_nudge_limit` provides additional headroom for the `record_nudge` path.

The detector resets cleanly (`reset()`) when a new session or agent starts, so prior loops don't carry over.

### 12.1. Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_HISTORY` | 6 | Sliding window size |
| `THRESHOLD` | 3 | Trigger threshold (3+ identical) |
| `MAX_NUDGE_CONTINUATIONS` | 3 | Nudge continuation limit |

### 12.2. Structure `ToolInvocation`

| Field | Type | Description |
|------|------|-------------|
| `tool_name` | `String` | Tool name |
| `args_hash` | `u64` | Arguments hash |

### 12.3. Structure `DoomLoopDetector`

| Field | Type | Description |
|------|------|-------------|
| `history` | `VecDeque<ToolInvocation>` | Sliding window of the last 6 invocations |
| `consecutive_same` | `u32` | Counter of consecutive identical invocations |
| `nudge_count` | `u32` | Nudge continuation counter |

### 12.4. `DoomLoopDetector::new()`

Creates an empty `VecDeque` with capacity `MAX_HISTORY`, zeroes counters.

### 12.5. `args_hash(args) -> u64`

**Algorithm:**
1. Tries to deserialize `args` as `serde_json::Value`.
2. If `Value::Object(map)`:
   - Converts to `BTreeMap<String, Value>` (key sorting).
   - Serializes back to string.
   - Hashes via `DefaultHasher`.
   - Returns `hasher.finish()`.
3. Otherwise (not an object) — hashes the raw string.
4. On deserialization error — hashes the raw string.

**Purpose:** guarantees that `{"a":1,"b":2}` and `{"b":2,"a":1}` produce the same hash.

### 12.6. `record_and_check(&mut self, tool_name, args_json) -> Option<String>`

**Algorithm:**
1. Computes `hash = args_hash(args_json)`.
2. Creates `invocation = ToolInvocation { tool_name, args_hash: hash }`.
3. **Repeat check:**
   - Gets the last element from `history` via `back()`.
   - If `last.tool_name == tool_name && last.args_hash == hash`:
     - `consecutive_same += 1`.
   - Otherwise — `consecutive_same = 1`.
4. Pushes `invocation` to `history`.
5. If `history.len() > MAX_HISTORY` — pops from the front (removes the oldest).
6. **Threshold check:** if `consecutive_same >= THRESHOLD`:
   - Returns `Some(warning_message)`.
7. Otherwise — `None`.

**Warning format:**
```
⚠️ Doom loop detected: tool '{tool_name}' has been invoked {consecutive_same} times in a row with identical arguments.
You MUST stop calling this tool. Try different arguments or a different tool.
If you cannot make progress, summarize what you have and stop.
```

### 12.7. `record_nudge(&mut self)`

Increments `nudge_count`.

### 12.8. `has_exceeded_nudge_limit(&self) -> bool`

Returns `nudge_count >= MAX_NUDGE_CONTINUATIONS`.

### 12.9. `reset(&mut self)`

Resets `history`, `consecutive_same`, and `nudge_count` to their initial state.

---

## 13. recovery.rs

**File:** `src/recovery.rs` — session recovery after crashes (fleet H1, H3).

### Architecture Overview

The recovery system provides **crash resilience through persistence**. Because all messages, tool results, and agent states are written to the SQLite DB as they happen, a crashed session can be reconstructed: completed agents are re-read from the DB, their final summaries regenerated from the persisted message history, and only the pending (never-completed) subtasks are re-executed.

The key concept is the **staleness threshold** (`stale_threshold`, default 5 minutes). A live session keeps touching its `updated_at` via the heartbeat (see [section 3.9](#39-start_heartbeatdb-session_id---heartbeatguard)); if a session has been `Running` but hasn't been updated for 5 minutes, it's presumed dead — the previous heartbeat thread died with the process, or the whole host went down. Only such stale sessions are offered for resumption.

The recovery flow integrates with the CLI (`resume.rs`) to offer the user a choice at startup: resume a `Running` stale session, or start fresh. After resumption, the `Coordinator::execute_resume` path reuses the existing `session_id`, restores accumulated tokens and agent counts, re-runs pending tasks, then re-synthesizes and rewrites the output files, so the final report is complete and idempotent.

### 13.1. Structure `RecoveredSession`

| Field | Type | Description |
|------|------|-------------|
| `session_id` | `SessionId` | Recovered session ID |
| `query` | `String` | Original query |
| `sub_tasks` | `Vec<String>` | List of subtasks |
| `completed_agents` | `Vec<AgentOutput>` | Completed agent results |
| `pending_tasks` | `Vec<String>` | Subtasks without completed agents |
| `created_at` | `DateTime<Utc>` | Session creation time |

### 13.2. Structure `SessionResumer`

| Field | Type | Description |
|------|------|-------------|
| `db` | `Arc<Persistence>` | Persistence layer |
| `config` | `AppConfig` | Configuration |
| `event_tx` | `Option<broadcast::Sender<AgentEvent>>` | Event bus |
| `stale_threshold` | `Duration` | Session staleness threshold (default 5 minutes) |

### 13.3. `SessionResumer::new(db, config, event_tx)`

Initializes with `stale_threshold = Duration::from_secs(300)`.

### 13.4. `find_resumable(&self) -> Vec<SessionSummary>`

**Algorithm:**
1. Calls `db.active_sessions()?` — gets a list of sessions with status `Running`.
2. Filters: keeps only sessions where `updated_at` is older than `stale_threshold` (5 minutes).
3. For each filtered session:
   - Gets session agents via `db.agents_for_session()`.
   - Counts agents by status: `completed`, `failed`, `running`.
   - Gets `sub_tasks` via `db.get_subtasks()`.
   - Forms `SessionSummary { session_id, query, created_at, completed_agents, failed_agents, running_agents, total_tokens, sub_tasks }`.
4. Sorts by `created_at` (newest first).
5. Returns the vector.

### 13.5. `resume(&self, session_id) -> Result<RecoveredSession>` — async

**Algorithm:**
1. Calls `db.get_session(&session_id)?`.
2. Checks `session.status != Running` — if not Running, bail.
3. Gets agents via `db.agents_for_session()`.
4. Gets subtasks via `db.get_subtasks()`.
5. **For each completed agent:**
   - Gets messages via `db.messages_for_agent(&agent_id)`.
   - Generates a summary via `summarize_agent_messages(&messages)`.
   - Gets findings via `db.findings_for_agent(&agent_id)`.
   - Pushes `AgentOutput { agent_id, summary, tokens_used, descendant_tokens: 0, findings, aborted: false }`.
6. **Determines pending_tasks:**
   - Collects the set of tasks from completed agents.
   - From subtasks, selects those not in the completed set.
7. Emits `SessionResumed { session_id, completed, pending }`.
8. Returns `RecoveredSession`.

### 13.6. `mark_stale_sessions_running(&self) -> Result<()>`

**Algorithm:**
1. Calls `db.active_sessions()`.
2. For each session:
   - If `updated_at` is older than 5 minutes:
     - Calls `db.mark_session_status(&session_id, "running")`.
     - Logs.
3. Errors for individual sessions are logged, do not interrupt processing.

### 13.7. `summarize_agent_messages(messages) -> String`

**Algorithm:**
1. Takes the **last** `Message::Assistant` from the list.
2. If found — returns its `content`.
3. If not found — looks for the last `Message::Tool`.
4. If neither found — `"(no summary available)"`.

---

## 14. resume.rs

**File:** `src/resume.rs` — CLI utility for selecting and launching session resumption.

### Architecture Overview

The resume module is the **user-facing CLI gateway** to the recovery system. When the application starts, it calls `handle_resume_interactive()`, which:

1. Scans the DB for stale `Running` sessions via `SessionResumer::find_resumable()`.
2. If found, presents an interactive menu showing each session's query, age, progress (% done), agent counts, and token usage.
3. The user chooses: resume a session, start fresh, or quit.

The user experience is designed to be **zero-surprise**: each session is displayed with its completion percentage and human-readable age, so the user can recognize which sessions matter. The `format_summary` function renders this in a compact, scannable format.

The `interactive_select` function reads from stdin and handles empty input, invalid numbers, and explicit quit commands. After selection, the coordinator is initialized with the recovered session's `session_id` and `query`, so the output files overwrite (not duplicate) the previous ones.

### 14.1. Structure `SessionSummary`

| Field | Type | Description |
|------|------|-------------|
| `session_id` | `SessionId` | Session ID |
| `query` | `String` | Query |
| `created_at` | `DateTime<Utc>` | Creation date |
| `completed_agents` | `u32` | Completed agents |
| `failed_agents` | `u32` | Failed agents |
| `running_agents` | `u32` | Running agents |
| `total_tokens` | `u64` | Total tokens |
| `sub_tasks` | `Vec<SubtaskRecord>` | Subtasks |

### 14.2. Structure `ResumeOption`

```rust
enum ResumeOption {
    Resume(RecoveredSession),
    Fresh,
    Exit,
}
```

### 14.3. `format_summary(summary, index) -> String`

**Algorithm:**
1. Computes `age` — the difference between `Utc::now()` and `created_at`.
2. Formats age human-readably: `< 1 hour`, `N hours`, `N days`.
3. Computes `done_pct = completed_agents * 100 / total_agents`.
4. Forms the string:
   ```
   [{index}] {query (first 60 chars)}... ({age} ago)
       Agents: {completed} done / {failed} failed / {running} running ({done_pct}%)
       Tokens: {total_tokens}
   ```

### 14.4. `interactive_select(summaries) -> ResumeOption`

**Algorithm:**
1. If `summaries` is empty — logs "No resumable sessions found", returns `Fresh`.
2. Prints header: `"Found {n} resumable session(s):"`.
3. For each summary with index — prints `format_summary(summary, i+1)`.
4. Prints options: `"[N] Resume session N`, `[F] Start fresh`, `[Q] Quit"`.
5. Reads stdin:
   - Empty input — `Exit`.
   - `"f"` or `"F"` — `Fresh`.
   - `"q"` or `"Q"` — `Exit`.
   - Number — parses, checks `1 <= n <= summaries.len()`, returns `Resume(summaries[n-1])`.
   - Invalid input — `Exit`.

### 14.5. `handle_resume_interactive(db, config, event_tx) -> Option<Coordinator>` — async

**Algorithm:**
1. Creates `SessionResumer::new(db, config, event_tx)`.
2. Calls `resumer.find_resumable()`.
3. If `summaries` is empty — returns `None`.
4. Calls `interactive_select(&summaries)`.
5. Matches result:
   - **`Fresh`** — returns `None`.
   - **`Exit`** — `process::exit(0)`.
   - **`Resume(selected)`:**
     - Calls `resumer.resume(&selected.session_id).await`.
     - Creates `Coordinator::new(...)` with the same `session_id` and `query` from the recovered session.
     - Connects `contact_db`, `crm`, `steer_rx`.
     - Returns `Some(coordinator)`.

---

## 15. tool_executor.rs

**File:** `src/tool_executor.rs` — parallel tool executor. Classifies tool_calls into concurrent and sequential, launches parallel ones simultaneously with path overlap detection.

### Architecture Overview

The parallel tool executor addresses a core performance concern: **modern LLMs emit batched tool calls**, and executing them one at a time is slow. Instead, the executor classifies each tool call into a category, groups safe-to-parallelize calls, and launches them with `join_all` (or `tokio::join!` when both parallel and sequential batches exist).

The safety rules are:

| Category | Tools | Execution |
|----------|-------|-----------|
| `ReadOnly` | `search_code`, `fetch_url`, `read_file`, `get_contacts` | Always concurrent — reads have no side effects |
| `Write` | `write_file`, `append_file`, `update_record` | Concurrent, but only if paths don't overlap |
| `Shell` | `shell` | Always sequential — ordering and side effects matter |
| `Unknown` | anything else | Always sequential — the executor is conservative |

**Path overlap detection** prevents data races: two write calls targeting the same file (or the same directory tree) are placed into the sequential batch. For example, `write_file` on `/tmp/data` and `write_file` on `/tmp/data/file.txt` conflict (one is a prefix of the other), so they execute sequentially. Path normalization (trailing-slash removal, `./` prefix stripping) makes the comparison robust.

The `execute_batch` entry point runs the parallel and sequential batches concurrently with `tokio::join!`, then re-sorts the combined results by their original index, preserving the order expected by the agent loop (which pairs results to call IDs in invocation order).

### 15.1. Structure `ToolCategory`

```rust
enum ToolCategory {
    ReadOnly,   // search_code, fetch_url, read_file, get_contacts
    Write,      // write_file, append_file, update_record
    Shell,      // shell
    Unknown,    // everything else
}
```

### 15.2. `classify_tool(name) -> ToolCategory`

**Algorithm:**
1. Converts `name` to lowercase.
2. Matches:
   - `"search_code"` | `"fetch_url"` | `"read_file"` | `"get_contacts"` → `ReadOnly`
   - `"write_file"` | `"append_file"` | `"update_record"` → `Write`
   - `"shell"` → `Shell`
   - `_` → `Unknown`

### 15.3. `extract_paths(tool_name, args) -> Vec<String>`

**Algorithm:**
1. Deserializes `args` as `serde_json::Value`.
2. If not an object — returns an empty vector.
3. Creates `HashSet<String>` for unique paths.
4. For each key in `{"path", "file", "file_path", "source", "destination"}`:
   - If the field exists and is a string — inserts into the set.
5. For keys `"paths"` and `"files"`:
   - If the field is a string array — inserts all.
6. Collects into `Vec`, normalizes the path:
   - Removes trailing `/`.
   - Removes `./` prefix.
7. Returns the vector.

### 15.4. `paths_overlap(a, b) -> bool`

**Algorithm:**
1. For each path from `a` and each path from `b`:
   - If `pa == pb` — `return true`.
   - If `pa.starts_with(pb)` **OR** `pb.starts_with(pa)` — `return true`.
2. If no comparison matched — `return false`.

**Example:** `["/tmp/foo.txt"]` and `["/tmp/foo.txt"]` → true. `["/tmp/data"]` and `["/tmp/data/file.txt"]` → true.

### 15.5. Structure `PartitionedBatch`

| Field | Type | Description |
|------|------|-------------|
| `sequential` | `Vec<(usize, String, String)>` | `(index, name, args)` — must be executed sequentially |
| `concurrent` | `Vec<(usize, String, String)>` | `(index, name, args)` — can be executed in parallel |
| `conflict_groups` | `Vec<Vec<usize>>` | Groups of indices that conflict by paths |

### 15.6. `partition_batch(tool_calls) -> PartitionedBatch`

**Algorithm:**
1. Classifies each `tool_call` via `classify_tool`.
2. **Shell and Unknown** → always in `sequential`.
3. **Write** → checks for conflicts:
   - If `sequential` already contains a Write with overlapping paths — adds to `sequential`.
   - Otherwise — adds to `concurrent`.
4. **ReadOnly** → adds to `concurrent`.
5. Forms `conflict_groups` from `sequential` indices.
6. Returns `PartitionedBatch`.

### 15.7. `execute_parallel(tools, concurrent, tool_ctx) -> Vec<(usize, ToolOutput)>` — async

**Algorithm:**
1. If `concurrent` is empty — returns an empty vector.
2. For each `(idx, name, args)` in `concurrent`:
   - Creates a future `tools.execute(&name, &args, tool_ctx)`.
3. Runs all futures via `futures::future::join_all(futs).await`.
4. Collects results: `(idx, output)` for each.
5. Returns the vector of pairs.

### 15.8. `execute_sequential(tools, sequential, tool_ctx) -> Vec<(usize, ToolOutput)>` — async

**Algorithm:**
1. Creates `results: Vec`.
2. For each `(idx, name, args)` in `sequential`:
   - Calls `tools.execute(&name, &args, tool_ctx).await`.
   - Pushes `(idx, output)`.
3. Returns `results`.

### 15.9. `execute_batch(tools, tool_calls, tool_ctx) -> Vec<(usize, ToolOutput)>` — async

**Algorithm:**
1. Calls `partition_batch(tool_calls)`.
2. Runs `execute_parallel(tools, concurrent, tool_ctx)` and `execute_sequential(tools, sequential, tool_ctx)` in parallel via `tokio::join!`.
3. Creates `all_results: Vec`.
4. Concatenates results: first `seq_results`, then `par_results`.
5. Sorts `all_results` by `idx` (restores original order).
6. Returns `all_results`.

---

## Overall Architectural Picture

### Session Lifecycle

```
CLI → Coordinator::execute()
  → plan()                    # LLM decomposition into subtasks
  → fan-out:
    - spawn_researchers()     # in-process (tokio tasks)
    - run_multiprocess_fanout() # out-of-process (Unix sockets)
  → synthesize()              # LLM report assembly
  → write_output()            # Files: summary.md, index.md, sources.md, findings/
```

### Agent Lifecycle (runtime loop)

```
init: [system_prompt, user_task]
loop while iterations < max:
  drain steer_rx
  inject bg_results
  check cancellation
  reset turn_budget
  check compaction threshold
  LLM call
  if no tool_calls:
    run Stop hooks → break or continue
  for each tool_call:
    doom_loop check
    cascade cancel check
    role gate
    PreToolUse hooks
    execute tool
    sub-agent delegation check
    PostToolUse hooks
    auto-persist contacts
    harvest findings
    truncation + turn budget
  run spawn_agent batch
```

### Context Compaction System

```
should_compact? (tokens >= threshold)
  → is_in_cooldown? → skip
  → micro_compact (without LLM):
    - deduplication by content_hash
    - prune old tool output > 40000 tokens
  → if still above threshold:
    - split head(4) / middle / tail(4) with tool-group safety
    - LLM summarize middle → system message
    - reassemble: head + summary + tail
  → update_effectiveness:
    - < 5% reduction → ineffective_passes++
    - 2 ineffective → cooldown 300 sec
```

### Hook System

```
PreToolUse hook: { event, tool, args, timestamp }
  → hook subprocess (stdin JSON, stdout JSON)
  → verdict: "allow" | "deny" + reason
  → 30s timeout + kill_on_drop

PostToolUse hook: { event, tool, args, output, success, timestamp }
  → extra_info appended to tool output

Stop hook: { event, output_preview, output_char_count, timestamp }
  → verdict: "stop" | "continue" + reason
  → max 3 continuations
```

### Doom Loop Detection System

```
Sliding window (last 6 invocations)
  → args_hash via serde_json BTreeMap normalization
  → 3+ consecutive identical → DETECTED
  → First time: NUDGE (warn, continue)
  → Second time: STOP (abort agent)
```