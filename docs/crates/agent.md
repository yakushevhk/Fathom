# Full architectural documentation for `pr-agent`

> Every function is described step by step, down to individual lines of code.

---

## Table of Contents

1. [lib.rs — Module entry point](#1-librs)
2. [runtime.rs — Agent runtime core](#2-runtimers)
3. [coordinator.rs — Session coordinator](#3-coordinatorrs)
4. [compaction.rs — Context compaction](#4-compactionrs)
5. [prompt.rs — Building system prompts](#5-promptrs)
6. [prompts/ — Prompt text templates](#6-prompts)
7. [hooks.rs — Lifecycle hooks](#7-hooksrs)
8. [ipc.rs — Inter-process protocol](#8-ipcrs)
9. [process_manager.rs — Worker process management](#9-process_managerrs)
10. [budget.rs — Output budget](#10-budgetrs)
11. [doom_loop.rs — Loop detector](#11-doom_looprs)
12. [resume.rs — Session resumption](#12-resumers)
13. [tool_executor.rs — Parallel tool executor](#13-tool_executorrs)

---

## 1. lib.rs

**File:** `src/lib.rs` — entry point of the `pr-agent` module.

### Purpose

Declares all public submodules of the crate and re-exports their contents via `pub use ...::*` so that external consumers can write `use pr_agent::AgentRuntime` instead of `use pr_agent::runtime::AgentRuntime`.

### Declared modules (16 in total)

```
lifecycle, runtime, coordinator, compaction, ipc, process_manager, prompt,
tool_executor, budget, resume, doom_loop, hooks, control,
task_tree, improvement, reflection
```

Additional agent modules:

| Module | Key types and purpose |
|--------|----------------------|
| `lifecycle` | `AgentLifecycleManager` — park, revive, and release agents |
| `task_tree` | Task-tree structure and parent/child task coordination |
| `improvement` | Agent improvement tracking and optimization |
| `reflection` | Agent reflection and self-review |

### Re-exports (11 in total)

All modules except `ipc` and `process_manager` re-export their public API at the crate root level. The `ipc` and `process_manager` modules remain internal — they are used only inside `coordinator.rs`.

---

## 2. runtime.rs

**File:** `src/runtime.rs` — the core of the agent runtime. Contains the agent's main loop.

### 2.1. The `AgentRuntime` struct

The main working unit of the system. Each instance is a single LLM agent operating in its own "think → call tool → get result" loop.

**Fields:**

| Field | Type | Purpose |
|------|-----|------------|
| `id` | `AgentId` | Unique agent identifier (UUID) |
| `session_id` | `SessionId` | Identifier of the session the agent belongs to |
| `parent_id` | `Option<AgentId>` | ID of the parent agent (for nested spawns) |
| `role` | `AgentRole` | Role: Coordinator, Researcher, Analyst, Verifier, Writer |
| `task` | `String` | Text description of the agent's task |
| `depth` | `u32` | Current nesting depth in the agent tree |
| `llm` | `Arc<dyn LlmProvider>` | LLM provider (GPT-4, Claude, DeepSeek, etc.) |
| `tools` | `Arc<ToolRegistry>` | Registry of available tools |
| `event_tx` | `broadcast::Sender<AgentEvent>` | Event bus for notifications (TUI, logging) |
| `db` | `Arc<Persistence>` | Persistence layer (SQLite) |
| `working_dir` | `PathBuf` | Working directory (for file operations) |
| `max_iterations` | `u32` | Maximum number of iterations of the main loop |
| `config` | `AppConfig` | Full application configuration |
| `messages` | `Vec<Message>` | Message history (system, user, assistant, tool) |
| `tokens_used` | `u64` | Counter of tokens spent (by this agent) |
| `descendant_tokens` | `u64` | Tokens spent by child agents |
| `estimated_tokens` | `u32` | Estimate of the current context size in tokens |
| `contact_db` | `Option<Arc<dyn ContactStore>>` | Contact database (inherited by child agents) |
| `crm` | `Option<Arc<CrmSync>>` | CRM synchronization |
| `compaction_engine` | `CompactionEngine` | Context compaction engine |
| `turn_budget` | `TurnBudget` | Tool output budget per turn |
| `memory_store` | `MemoryStore` | Agent's persistent memory (cross-session facts) |
| `skill_registry` | `SkillRegistry` | Registry of discovered skills |
| `doom_loop` | `DoomLoopDetector` | Loop detector |
| `doom_nudged` | `bool` | Whether a "nudge" (first warning) has already been sent |
| `harvested_findings` | `Vec<Finding>` | Structured findings extracted from tool metadata |
| `cancel` | `CancellationToken` | Cooperative cancellation token |
| `denied_tools` | `HashSet<String>` | Tools forbidden for this role |
| `steer_rx` | `Option<Arc<Mutex<UnboundedReceiver<String>>>>` | Channel for mid-run instructions from the user |
| `steer_tx` | `Option<UnboundedSender<String>>` | Sender half of the steering channel, forwarded to child agents |
| `peer_steer_rx` | `Option<Arc<Mutex<UnboundedReceiver<String>>>>` | Channel for mid-run instructions from peer agents |
| `irc_rx` | `Option<Arc<Mutex<UnboundedReceiver<IrcMessage>>>>` | Incoming messages from the process-global IrcBus |
| `job_results` | `Arc<Mutex<Vec<AsyncJobResult>>>` | Results of in-process async background jobs |
| `bg_results` | `Arc<Mutex<Vec<(String, Result<String, String>, u64)>>>` | Results of background child agents |
| `truncation_retries` | `u32` | Counter of retries after response truncation |
| `stop_continuations` | `u32` | Counter of forced continuations by Stop hooks |
| `role_llms` | `HashMap<String, Arc<dyn LlmProvider>>` | LLM providers per role (fleet E8) |

### 2.2. `denied_tools_for_role(config, role) -> HashSet<String>`

**Algorithm:**
1. Maps the `AgentRole` value to a string key: `"coordinator"`, `"researcher"`, `"analyst"`, `"verifier"`, `"writer"`.
2. Looks up an entry with that key in `config.agent.deny_tools`.
3. If the entry is found — collects the tools into a `HashSet`, converting each name to lowercase.
4. If not found — returns an empty `HashSet` (`.unwrap_or_default()`).

### 2.3. `AgentRuntime::new(...)`

**Algorithm:**
1. Extracts `max_iterations` from the configuration.
2. Calls `denied_tools_for_role()` to build the set of forbidden tools.
3. Creates a `TurnBudget` with the limit `config.context.turn_budget_bytes`.
4. Creates a `CompactionEngine` from the context configuration.
5. Determines the home directory (`dirs::home_dir()`, fallback — `/tmp`).
6. Creates a `MemoryStore` from the home directory.
7. Creates a `SkillRegistry` from the home directory.
8. Calls `skill_registry.discover()` in best-effort mode (errors are logged, not fatal).
9. Initializes all struct fields with default values: empty vectors, zeros, `None`, empty `HashSet`, a new `CancellationToken`.

### 2.4. Builder methods

#### `with_role_llms(mut self, map) -> Self`
Replaces `self.role_llms` with the given map. Allows the coordinator to assign different LLMs to different roles (fleet E8).

#### `with_steer_rx(mut self, rx) -> Self`
Connects the mid-run instruction channel (fleet E1). The user can send instructions while the agent is working; they are processed at the turn boundary.

#### `with_cancel_token(mut self, token) -> Self`
Replaces the cancellation token. Child agents inherit `child_token()` from the parent.

#### `cancel_token(&self) -> CancellationToken`
Clones and returns the current cancellation token (for supervisors).

### 2.5. `build_tool_context(&self) -> ToolContext`

**Algorithm:**
1. Creates `ToolContext::new(self.working_dir, self.config.search)`.
2. Attaches the LLM provider via `.with_llm(self.llm.clone())`.
3. If `self.contact_db` exists — attaches it via `.with_contact_db()`.
4. If `self.crm` exists — attaches it via `.with_crm()`.
5. Returns the ready context.

Called once per run (not on every tool invocation) so that stateful subsystems (file locks, file history, read tracking) work across invocations.

### 2.6. `emit(&self, event)`

Logs the event to the `event_tx` bus. Send errors are ignored (there may be no receivers).

### 2.7. `emit_tool_hook_denied(&self, tool)`

Logs a warning via `tracing::warn!` that the tool `tool` was denied by a PreToolUse hook.

### 2.8. `build_system_prompt(&self) -> String`

**Algorithm:**
1. Creates `PromptBuilder::new(role, task, depth, max_depth, model)`.
2. **Context layer:** calls `builder.add_env(config, working_dir)` — adds an `<env>` block with the working directory, platform, model, date, git status.
3. **Volatile layer:** extracts tool schemas from the registry, **filtering out** the tools in `denied_tools` (forbidden tools are hidden from the model).
4. **Volatile layer:** obtains the memory block from `memory_store.to_system_prompt_block()`. Adds it if not empty.
5. **Volatile layer:** obtains the skills block from `skill_registry.to_system_prompt_block()`. Adds it if not empty.
6. **Stable layer:** adds behavioral instructions ("use tools, cite sources, be precise, write findings to files, use memory").
7. Calls `builder.build()` — concatenation of layers: stable → context → volatile.

### 2.9. `recalculate_estimated_tokens(&mut self)`

Fully recomputes `estimated_tokens` from all messages in `self.messages` via `estimate_messages_tokens()`. Used after compaction (bulk rewrites).

### 2.10. `track_message_tokens(&mut self, msg)`

Incrementally adds the token estimate of a single message to `estimated_tokens`. Monotonic between compactions — a full O(n) recomputation is not needed on every addition.

### 2.11. `run_compaction(&mut self)` — async

**Algorithm:**
1. Records `tokens_before = self.estimated_tokens`.
2. Clones `self.llm` for the closure.
3. Calls `self.compaction_engine.compact(&mut self.messages, summarize_fn)`.
4. `summarize_fn` is a closure that:
   - Accepts `prompt_messages` (the summarization prompt).
   - Creates a `CompletionRequest` with temperature=0.3, max_tokens=2048, stream=false, and an empty tools list.
   - Calls `llm.complete(&req)`.
   - Extracts the text from the `Message::Assistant` response.
5. If `compact()` returned `Ok(cr)`:
   - If `cr.tokens_after < tokens_before` — logs the reduction percentage, assigns `self.messages = cr.messages`, recomputes tokens.
   - Otherwise — logs "no effect".
6. If `compact()` returned `Err(e)` — logs the error (not fatal).

### 2.12. `AgentRuntime::run(&mut self) -> Result<AgentOutput>` — async — MAIN LOOP

**Step-by-step algorithm:**

#### Step 0: Initialization
1. Pushes `Message::system(build_system_prompt())` into `self.messages`.
2. Pushes `Message::user(&self.task)` into `self.messages`.
3. Calls `recalculate_estimated_tokens()`.
4. Saves both messages to the DB via `db.add_message()`.
5. Emits the `AgentStateChanged { state: Researching }` event.
6. Extracts `tool_schemas` from the registry, filtering out forbidden ones.
7. Creates `tool_ctx = build_tool_context()`.
8. Initializes: `iterations = 0`, `final_content = ""`, `doom_warning = None`.

#### Step 1: Main loop (`'main_loop: while iterations < max_iterations`)
Each iteration is one agent "turn".

##### 1a. Drain the steering channel (fleet E1)
- If `steer_rx` is connected:
  - Acquires the lock on `rx`.
  - Reads all accumulated instructions in a `try_recv()` loop.
  - For each: creates `Message::user("[USER INSTRUCTION] {msg}")`, pushes it to history, saves it to the DB, tracks tokens.

##### 1b. Drain the peer steering channel
- If `peer_steer_rx` is connected:
  - Acquires the lock on `rx`.
  - Reads all accumulated instructions in a `try_recv()` loop.
  - For each: creates `Message::user("[PEER INSTRUCTION] {msg}")`, pushes it to history, saves it to the DB, tracks tokens.

##### 1c. Drain the IRC bus
- If `irc_rx` is connected:
  - Acquires the lock on `rx`.
  - Reads all accumulated messages in a `try_recv()` loop.
  - For each: creates `Message::user("[IRC] {sender}: {payload}")`, pushes it to history, saves it to the DB, tracks tokens.

##### 1d. Drain async job results
- Acquires the lock on `job_results`.
- Takes all items via `std::mem::take`.
- For each `AsyncJobResult`:
  - Adds `descendant_tokens += result.tokens`.
  - Builds the text: on success — `[async job {label} completed]\n{summary}`; on error — `[async job {label} failed: {e}]`.
  - Pushes `Message::user(text)` to history, saves to DB, tracks tokens.

##### 1e. Inject background child results (fleet E2)
- Acquires the lock on `bg_results`.
- Takes all items via `std::mem::take`.
- For each `(label, result, tokens)`:
  - Adds `descendant_tokens += tokens`.
  - Builds the text: on success — `[background agent {label} completed]\n{summary}` (truncated to 4000 characters); on error — `[background agent {label} failed: {e}]`.
  - Pushes `Message::user(text)` to history, saves to DB, tracks tokens.

##### 1f. Cooperative cancellation check
- If `self.cancel.is_cancelled()`:
  - Logs a warning.
  - Emits `AgentFailed { error: "cancelled" }`.
  - `anyhow::bail!("agent cancelled")`.

##### 1g. Reset the turn budget
- `self.turn_budget = TurnBudget::new(config.context.turn_budget_bytes)`.

##### 1h. Check whether compaction is needed
- `self.compaction_engine.set_estimated_tokens(self.estimated_tokens)`.
- If `should_compact()` returns `true` — calls `self.run_compaction().await`.

##### 1i. Build the LLM request
- `CompletionRequest { messages: self.messages.clone(), tools: tool_schemas.clone(), temperature, max_tokens, stream: false }`.

##### 1j. Call the LLM
- `self.llm.complete(&req).await`.
- On error: logs, emits `AgentFailed`, returns `Err`.

##### 1k. Token tracking
- If `response.usage` exists — `tokens_used += usage.total_tokens`.

##### 1l. Add the assistant response
- Pushes `response.message` into `self.messages`.
- Saves to DB, tracks tokens.

##### 1m. Extract content and tool_calls
- If the response is `Message::Assistant { content, tool_calls }`:
  - If `content` is not empty — updates `final_content`, emits `LlmStreamChunk`.

##### 1n. Handle the absence of tool_calls (the model wants to stop)
- If `tool_calls` is empty:
  - **Stop hooks (fleet E3):** checks `stop_continuations < MAX_STOP_CONTINUATIONS` (3).
    - If so — calls `run_stop_hooks(&config.hooks, &summary_so_far)`.
    - If not — skips the hooks and uses `StopVerdict::Stop`.
  - If `StopVerdict::Continue(reason)`:
    - `stop_continuations += 1`.
    - Logs.
    - Pushes `Message::user("[hook] Do not stop yet: {reason}")`.
    - `continue 'main_loop` — the agent keeps working.
  - If `StopVerdict::Stop` — `break` out of the loop.

##### 1o. Handle tool_calls
For each `tool_call` in order:

###### 1o-i. Doom loop detection
- Calls `self.doom_loop.record_and_check(tool_name, &tool_args)`.
- If the detector triggers (3+ identical calls):
  - Collects the IDs of all remaining sibling tool_calls.
  - **First time (doom_nudged == false):**
    - Sets `doom_nudged = true`.
    - Builds the "nudge" message: "Repeated identical call detected. Do NOT repeat — try other arguments or finish."
    - For the first remaining call_id — pushes the nudge; for the rest — `Cancelled: {nudge}`.
    - `break` out of the tool_calls loop → return to the LLM to change strategy.
  - **Second time (doom_nudged == true):**
    - Builds the warning: "Doom loop detected: tool invoked repeatedly even after warning. Stopping agent."
    - Emits `AgentFailed`.
    - Answers all remaining tool_calls plus the already collected pending_spawns with error messages.
    - If `final_content` is empty — uses the warning.
    - Sets `doom_warning`.
    - `break 'main_loop` — full agent stop.

###### 1o-ii. Emit ToolCallStarted

###### 1o-iii. Cascading cancellation
- If `shell_failed` contains an error:
  - Result = `ToolOutput::err("Cancelled: sibling shell tool failed with: {err}")` with metadata `cascade_cancelled: true`.
  - Moves to the next step (does not execute the tool).

###### 1o-iv. Role permission gate (fleet E5)
- If `denied_tools` contains `tool_name`:
  - Result = `ToolOutput::err_code("Permission denied: role ... is not allowed to use '...'", "permission_denied")`.

###### 1o-v. PreToolUse hooks (fleet E3)
- Calls `run_pre_tool_hooks(&config.hooks, tool_name, &tool_args)`.
- If `PreToolVerdict::Deny(reason)`:
  - Result = `ToolOutput::err_code("Denied by hook: {reason}", "hook_denied")`.
  - Logs via `emit_tool_hook_denied`.

###### 1o-vi. Execute the tool
- If `PreToolVerdict::Allow`:
  - `self.tools.execute(tool_name, tool_args, &tool_ctx).await`.
  - On success: if `tool_name == "shell"` and `!output.success` — records the error in `shell_failed`.
  - On execution error: `ToolOutput::err("Tool execution error: {e}")`.

###### 1o-vii. Sub-agent delegation (fleet D4)
- If `tool_name == "spawn_agent"` and the metadata contains `"spawn_request": true`:
  - Saves `(call_id, metadata)` into `pending_spawns`.
  - `continue` — does not push a tool message immediately.

###### 1l-viii. PostToolUse hooks (fleet E3)
- Calls `run_post_tool_hooks(&config.hooks, tool_name, &tool_args, &result.content, success)`.
- If it returned `Some(extra)` — appends `extra` to `result.content`.

###### 1l-ix. Auto-persist contacts (fleet C1)
- If the tool is `extract_contacts` or `find_leads` and the result is successful:
  - Extracts `contact_db` from `tool_ctx`.
  - For `extract_contacts` — calls `autosave_extracted(&db, contacts_meta, &origin)`.
  - For `find_leads` — calls `autosave_leads(&db, leads_meta)`.
  - If anything was saved/merged — appends a notification to `result.content`.
  - Updates the result metadata with the `auto_saved` field.

###### 1l-x. Harvest findings (fleet C4)
- Calls `self.harvest_finding(tool_name, &tool_args, &result)`.
- If it returned `Some(finding)` — pushes it into `self.harvested_findings`.

###### 1l-xi. Emit ToolCallCompleted
- With a result preview (first 200 characters) and `duration_ms`.

###### 1l-xii. Save the full result to the DB
- `db.add_tool_result(agent_id, tool_name, tool_args, result, duration)`.

###### 1l-xiii. Truncation + turn budget
- Calls `apply_turn_budget(tool_name, result, max_bytes, max_lines, &mut turn_budget, working_dir)`.
- Returns `Truncated::Unchanged` or `Truncated::Truncated { replacement }`.
- Extracts the content for the tool message.

###### 1l-xiv. Add the tool message
- Pushes `Message::tool(tool_call_id, content)` to history.
- Saves to DB, tracks tokens.

##### 1p. Run the collected spawn requests (fleet D4)
- If `pending_spawns` is not empty — calls `self.run_spawn_batch(&mut pending_spawns).await`.

#### Step 2: Post-loop
- If `iterations >= max_iterations` and there is no doom_warning — logs a warning.
- If there is no doom_warning — emits `AgentStateChanged { state: Complete }`.
- Returns `AgentOutput { agent_id, summary, tokens_used, descendant_tokens, findings, aborted }`.

### 2.13. `prepare_child(&mut self, meta) -> Result<(AgentId, AgentRuntime)>`

**Algorithm:**
1. Computes `child_depth = self.depth + 1`.
2. Checks `child_depth > config.agent.max_depth` — if so, `bail!`.
3. Extracts `task` from `meta["task"]` — if empty, `bail!`.
4. Extracts `role` from `meta["role"]` (default `"researcher"`), maps it to `AgentRole`.
5. Extracts `context: Vec<String>` from `meta["context"]`.
6. Generates a new `AgentId`.
7. Creates a record in the DB (`db.create_agent`).
8. Emits `AgentSpawned`.
9. Builds `full_task`: if `context` is not empty — appends bullets to the end of the task under the heading "## Context from parent agent".
10. Determines the LLM: if `role_llms` contains an entry for the role — uses it, otherwise inherits the parent's.
11. Creates `AgentRuntime::new(...)` with the full set of parameters.
12. Inherits `contact_db` and `crm`.
13. Sets `child_token()` from the parent's `cancel` as the cancellation token.
14. Returns `(agent_id, child_runtime)`.

### 2.14. `run_spawn_batch(&mut self, pending) -> Result<()>` — async

#### `spawn_agent` tool parameters

The `spawn_agent` tool produces metadata that the runtime processes in the main loop (step 1o). Key parameters:

| Parameter | Type | Description |
|-----------|------|-------------|
| `task` | `String` | Task description for a single sub-agent |
| `role` | `String` | Agent role: researcher, analyst, verifier, writer |
| `context` | `Vec<String>` | Facts and constraints inherited by the child |
| `tasks` | `Vec<BatchTask>` | Batch of sub-tasks for parallel fan-out (mutually exclusive with `task`) |
| `output_schema` | `Option<Value>` | Expected JSON schema for structured output |
| `handoff_to` | `Option<String>` | Agent ID to hand off the full conversation context to |
| `isolated` | `bool` | If true, the child runs in isolated mode |

**Batch spawn flow:** When `tasks` is non-empty, the tool emits a `spawn_batch` marker. The runtime expands it into individual pending spawns (one per task), each inheriting the parent's `role` and `context`. The expanded items are then processed by `run_spawn_batch` below.

**Handoff flow:** When `handoff_to` is set, the tool emits a metadata flag `handoff: true`. At the turn boundary the runtime serializes the parent's full state (messages, findings, tokens) and sends it via the global `IrcBus` to the target agent. The parent then breaks out of its main loop. Handoff is mutually exclusive with `task` and `tasks`.

**Algorithm:**

#### Preparation
1. Computes `width = max(1, config.agent.max_concurrent_children)` — the limit of parallel children.
2. Computes `headroom_chars` — how many characters are free in the context: `(context_window * 4) - (estimated_tokens * 4)`.
3. Determines `spill_dir = working_dir/.pr-context/spills`.
4. Clones `db`, `event_tx`.

#### Split into foreground and background
5. For each `(call_id, meta)` in `pending`:
   - Checks `meta["background"]` — if `true`, the agent is a background one.
   - Calls `self.prepare_child(&meta)`.
   - **Background:** creates a future via `child_wait_future`, wraps it in `tokio::spawn`, writes the result to `bg_results`.
   - **Foreground:** adds to `items`.
   - On preparation error — adds to `early_fails`.

#### Immediate confirmation of background launches
6. For each background launch — pushes `ToolOutput::ok("Background agent {label} launched...")` via `record_spawn_result`.

#### Parallel launch of foreground
7. For each `(call_id, agent_id, child)` in `items` — creates a future via `child_wait_future`.
8. Runs all futures via `futures::stream::iter(futs).buffered(width).collect().await`.

#### Result handling
9. `early_fails` → `ToolOutput::err("Sub-agent failed: {err}")` → `record_spawn_result`.
10. Main results:
    - On success: `descendant_tokens += tokens`, `ToolOutput::ok(summary)` → `record_spawn_result`.
    - On error: `ToolOutput::err(...)` → `record_spawn_result`.

### 2.15. `record_spawn_result(&mut self, call_id, output) -> Result<()>`

**Algorithm:**
1. Saves the result to the DB (`db.add_tool_result`).
2. Applies truncation + turn budget (`apply_turn_budget`).
3. Extracts the content (possibly truncated).
4. Pushes `Message::tool(call_id, content)` to history.
5. Saves to DB, tracks tokens.

### 2.16. `child_wait_future(child, agent_id, db, tx, timeout_secs, headroom_chars, batch_len, spill_dir) -> Pin<Box<Future>>`

**Algorithm:**
1. Runs `child.run()` with an optional `tokio::time::timeout(timeout_secs, ...)`.
2. On `Ok(output)`:
   - If `output.aborted` — updates the DB status to `Failed`, emits `AgentFailed`, returns an error.
   - Otherwise — updates the status to `Completed`, emits `AgentCompleted`, creates `ResultBudget::new(headroom_chars, batch_len, spill_dir)`, caps the result via `budget.cap_result(&output.summary)`, returns `(capped_summary, total_tokens)`.
3. On `Err(e)` — updates the status to `Failed`, emits `AgentFailed`, returns an error.

### 2.17. `harvest_finding(&self, tool_name, tool_args, result) -> Option<Finding>`

**Algorithm:**
1. If `!result.success` — returns `None`.
2. Extracts `result.metadata`.
3. Based on `tool_name`:
   - **`extract_contacts`:**
     - Extracts `counts` from metadata (emails, phones, social_profiles, persons, companies).
     - Determines `origin` from the `url` argument (or `"inline text"`).
     - If origin starts with `http` — creates `Source { url, title }`.
     - Returns a `Finding` titled "Contacts extracted from {origin}" with count content and confidence=0.7.
   - **`find_leads`:**
     - Extracts the `leads` array from metadata.
     - If empty — `None`.
     - Takes the first 5 leads, formats `"- {name} @ {company}"`.
     - Returns a `Finding` titled "Leads harvested: {count}" with confidence=0.6.
   - **Other tools:** `None`.

### 2.18. The `AgentOutput` struct

| Field | Type | Description |
|------|-----|----------|
| `agent_id` | `AgentId` | Agent ID |
| `summary` | `String` | Final text (final answer or doom loop warning) |
| `tokens_used` | `u64` | Own tokens |
| `descendant_tokens` | `u64` | Tokens of all descendants |
| `findings` | `Vec<Finding>` | Structured findings |
| `aborted` | `bool` | `true` if stopped by the doom loop detector |

---

## 3. coordinator.rs

**File:** `src/coordinator.rs` — the session coordinator. Manages the lifecycle of a research session: from decomposition to synthesis.

### 3.1. The `Coordinator` struct

| Field | Type | Purpose |
|------|-----|------------|
| `session_id` | `SessionId` | Session identifier |
| `query` | `String` | Original user query |
| `llm` | `Arc<dyn LlmProvider>` | Main LLM provider |
| `tools` | `Arc<ToolRegistry>` | Tool registry |
| `event_tx` | `broadcast::Sender<AgentEvent>` | Event bus |
| `db` | `Arc<Persistence>` | Persistence |
| `output_dir` | `PathBuf` | Directory for output files |
| `config` | `AppConfig` | Configuration |
| `total_tokens` | `u64` | Total token counter for the session |
| `total_agents` | `u32` | Total number of launched agents |
| `use_multiprocess` | `bool` | Multiprocess mode flag |
| `contact_db` | `Option<Arc<dyn ContactStore>>` | Contact database |
| `crm` | `Option<Arc<CrmSync>>` | CRM synchronization |
| `task_type` | `TaskType` | Task type: Research or LeadGen |
| `target_count` | `Option<u32>` | Target number of contacts (for LeadGen) |
| `started_at` | `DateTime<Utc>` | Session start time |
| `session_cancel` | `CancellationToken` | Cancellation token for the whole session |
| `agent_tokens` | `Arc<Mutex<HashMap<String, CancellationToken>>>` | Cancellation tokens of live agents |
| `steer_rx` | `Option<Arc<Mutex<UnboundedReceiver<String>>>>` | Steering channel |
| `role_llms` | `HashMap<String, Arc<dyn LlmProvider>>` | LLM providers per role |

### 3.2. The `TaskType` enum

```rust
enum TaskType { Research, LeadGen }
```

### 3.3. `Coordinator::new(...)`

**Algorithm:**
1. Reads `use_multiprocess` from the configuration.
2. Calls `build_role_llms(&config)` to build the role → LLM map.
3. Initializes all fields with default values: `total_tokens = 0`, `total_agents = 0`, `task_type = Research`, `target_count = None`, `started_at = Utc::now()`, a new `CancellationToken`, empty collections.

### 3.4. `build_role_llms(config) -> HashMap<String, Arc<dyn LlmProvider>>`

**Algorithm:**
1. Creates an empty `HashMap`.
2. For each `(role, model)` pair in `config.agent.role_models`:
   - Clones `config.llm`, replaces `model` with the specified one.
   - Calls `pr_llm::build_provider(&llm_cfg)`.
   - On success — logs and inserts `(role_lowercase, provider)` into the map.
   - On error — logs a warning and skips.
3. Returns the map.

### 3.5. `llm_for_role(&self, role) -> Arc<dyn LlmProvider>`

Maps the role to a string key and looks it up in `role_llms`. If not found — returns `self.llm.clone()` (default).

### 3.6. `budget_exhausted(&self) -> bool`

Returns `true` if `session_token_limit > 0` and `total_tokens >= session_token_limit`.

### 3.7. Builder methods

- `with_steer_rx(rx)` — connects the steering channel.
- `cancel()` — cancels `session_cancel`.
- `cancel_token()` — clones the token.
- `set_cancel_token(token)` — replaces the token.
- `with_contact_db(db)` — connects the contact database.
- `with_crm(crm)` — connects the CRM.

### 3.8. `emit(&self, event)`

Sends the event to the bus.

### 3.9. `start_heartbeat(db, session_id) -> HeartbeatGuard`

**Algorithm:**
1. Creates a `tokio::spawn` with an infinite loop.
2. Creates `tokio::time::interval(60s)`.
3. The first tick is skipped (it is immediate).
4. On each subsequent tick: calls `db.touch_session(&session_id)`. On error — `break`.
5. Returns `HeartbeatGuard { handle }`, which calls `handle.abort()` on `Drop`.

**Purpose:** updates `sessions.updated_at` every 60 seconds so that `SessionResumer` does not consider a live session interrupted (the threshold is 5 minutes of inactivity).

### 3.10. `start_stall_monitor(event_rx, tokens, warn_secs, kill_secs, session_id) -> Option<StallMonitorGuard>`

**Algorithm:**
1. If `warn_secs == 0 && kill_secs == 0` — returns `None` (monitoring disabled).
2. Launches `stall_monitor_loop(event_rx, tokens, warn_secs, kill_secs, tick=30s, session_id)` in `tokio::spawn`.
3. Returns `StallMonitorGuard` (aborts on `Drop`).

### 3.11. `stall_monitor_loop(event_rx, tokens, warn_secs, kill_secs, tick, session_id)` — async

**Algorithm:**
1. Creates `last_progress: HashMap<String, Instant>` — the last activity time of each agent.
2. Creates `warned: HashSet<String>` — agents that have already received a warning.
3. Creates an `interval` with period `tick` (30 seconds).
4. Infinite `tokio::select!` loop:
   - **`event_rx.recv()` branch:**
     - On receiving an event: extracts `agent_id` from the event, updates `last_progress` (or inserts `now()`).
     - If the event is terminal (`SessionCompleted` / `SessionFailed`) **for the current session** — `break`.
     - On `Lagged` — ignores.
     - On `Closed` — `break`.
   - **`interval.tick()` branch:**
     - Computes `now`.
     - For each `(agent_id, last)` in `last_progress`:
       - Computes `idle_secs = now - last`.
       - Gets the token from the `tokens` map.
       - If the token is already cancelled — `continue`.
       - If `kill_secs > 0 && idle_secs >= kill_secs` — logs an error, cancels the token.
       - Otherwise if `warn_secs > 0 && idle_secs >= warn_secs && !warned` — logs a warning, adds to `warned`.

### 3.12. `build_researcher(&self, agent_id, parent_id, task, depth) -> AgentRuntime`

**Algorithm:**
1. Creates `AgentRuntime::new(...)` with role `Researcher` and the LLM via `llm_for_role(Researcher)`.
2. Inherits `contact_db` and `crm`.
3. Sets `role_llms` via `with_role_llms`.

### 3.13. `run_with_timeout(&self, agent) -> Result<AgentOutput>` — async

**Algorithm:**
1. If `timeout_seconds == 0` — runs `agent.run()` without a timeout.
2. Otherwise — wraps it in `tokio::time::timeout(Duration::from_secs(timeout_seconds), agent.run())`.
3. On `Elapsed` — returns an error.

### 3.14. `spawn_researchers(&mut self, sub_tasks) -> Vec<AgentOutput>` — async

**Algorithm:**
1. Creates `findings: Vec<AgentOutput>` and `join_set: JoinSet`.
2. For each `task_desc` in `sub_tasks`:
   - Checks `total_agents >= max_agents` — if so, logs, updates the subtask status to "skipped", `break`.
   - Checks `budget_exhausted()` — if so, similarly.
   - Generates an `AgentId`, increments `total_agents`.
   - Creates an `AgentRecord`, saves it to the DB.
   - Emits `AgentSpawned`.
   - Creates the agent via `build_researcher`.
   - Connects steering (if present).
   - Creates `child_token()` from `session_cancel`, registers it in `agent_tokens`.
   - Sets the `cancel_token`.
   - Launches in `join_set.spawn` wrapped in `tokio::select!`:
     - `run` branch — the agent runs with a timeout.
     - `token.cancelled()` branch — updates the DB status to `Cancelled`, returns an error.
3. Collects results from `join_set.join_next()`:
   - `Ok(Ok(output))`: updates `total_tokens`, updates the DB status (Completed or Failed if aborted), emits an event, pushes into findings (if not aborted).
   - `Ok(Err(e))`: logs the error.
   - `Err(e)`: logs the task panic.
4. Clears `agent_tokens`.
5. Returns `findings`.

### 3.15. `Coordinator::execute(&mut self) -> Result<SessionOutput>` — async — MAIN METHOD

**Step-by-step algorithm:**

#### Step 0: Initialization
1. Starts the heartbeat (`start_heartbeat`).
2. Starts the stall monitor (`start_stall_monitor`).
3. Emits `SessionStarted`.

#### Step 1: Plan
4. Calls `self.plan().await` — gets `sub_tasks: Vec<String>`.
5. Logs the number of subtasks.
6. For each subtask — calls `db.add_subtask()` (Goal Mode light, fleet E4).

#### Step 2: Fan-out
7. If `sub_tasks` is empty — runs `run_single_agent()`.
8. If `use_multiprocess` — runs `run_multiprocess_fanout(&sub_tasks)`.
9. Otherwise — runs `spawn_researchers(&sub_tasks)` (in-process).

#### Step 2.5: Sync subtask statuses
10. Calls `sync_subtask_statuses()` — synchronizes subtask row statuses with agent outcomes.

#### Step 2.6: Persist structured findings (fleet C4)
11. For each output and finding — calls `db.add_finding(finding)`.

#### Step 2.7: Reflection round for LeadGen (fleet C3)
12. If `task_type == LeadGen` and `target_count` is set:
    - Calls `contacts_saved_so_far()`.
    - If `saved < target` and `total_agents < max_agents`:
      - Computes `gap = target - saved`.
      - Builds a gap-filling task.
      - Runs `spawn_researchers(&[gap_task])`.
      - Persists findings, adds them to the overall list.

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
1. Starts the heartbeat and stall monitor.
2. Emits `SessionStarted` (with the `[resume]` prefix).
3. Restores `findings` from `state.completed_agents`.
4. For each restored finding — adds tokens to `total_tokens` and increments `total_agents`.
5. If there are `pending_tasks` — runs `spawn_researchers(&state.pending_tasks)`.
6. Calls `synthesize`, `write_output`, `complete_session`, `emit(SessionCompleted)`.
7. Returns `SessionOutput`.

### 3.17. `detect_task_type(query) -> TaskType`

**Algorithm:**
1. Converts the query to lowercase.
2. Checks for the presence of any of the markers: `"email"`, `"phone"`, `"контакт"`, `"лид"`, `"lead"`, `"ceo"`, `"cto"`, `"linkedin"` and others.
3. If at least one marker is found — `LeadGen`, otherwise `Research`.

### 3.18. `detect_target_count(query) -> Option<u32>`

**Algorithm:**
1. Converts the query to lowercase.
2. Determines the marker array: `"email"`, `"контакт"`, `"contact"`, `"лид"`, `"lead"`, `"телефон"`, `"phone"`.
3. Scans the string byte by byte:
   - On encountering a digit — collects the full number.
   - Checks the 16-character window after the number for the presence of markers.
   - If a marker is found — parses the number. If `0 < n <= 10_000` — returns `Some(n)`.
4. If nothing is found — `None`.

### 3.19. `plan(&mut self) -> Result<Vec<String>>` — async

**Algorithm:**
1. Determines `task_type` via `detect_task_type(&self.query)`.
2. Determines `target_count` via `detect_target_count(&self.query)`.
3. Builds the prompt:
   - For **LeadGen**: instructions to decompose into 2-5 non-overlapping contact collection tasks split by industry, name range, and source type. Each task must include a goal description, tools, and a quota.
   - For **Research**: instructions to decompose into 2-5 independent research subtasks.
4. Creates a `CompletionRequest`:
   - System: `"You are a research planner.\n\n{role_prompt_for(Coordinator)}\n\nOutput only valid JSON."`
   - User: the decomposition prompt.
   - temperature=0.3, max_tokens=2048, stream=false, tools=empty.
5. Calls `llm.complete(&req)`.
6. Parses the response:
   - Tries `serde_json::from_str::<Vec<String>>(text)`.
   - If that fails — looks for `[` and `]` in the text, tries to parse the content between them.
7. Fallback: returns `vec![self.query.clone()]` (a single task).

### 3.20. `run_single_agent(&mut self) -> Result<AgentOutput>` — async

**Algorithm:**
1. Creates an `AgentId`, increments `total_agents`.
2. Creates an `AgentRecord` (role=Researcher, depth=0), saves it to the DB.
3. Creates the agent via `build_researcher`, connects steering and the cancel token.
4. Runs it via `run_with_timeout`.
5. Wrapped in `tokio::select!` with a `token.cancelled()` branch:
   - On cancellation — updates the DB status, emits `AgentFailed`, bail.
6. Updates `total_tokens`.
7. Updates the DB status (Completed or Failed if aborted), emits an event.
8. Returns `output`.

### 3.21. `run_multiprocess_fanout(&mut self, sub_tasks) -> Result<Vec<AgentOutput>>` — async

**Algorithm:**

#### Step 1: Spawn workers
1. Creates `ProcessManager::new(socket_dir)`, where `socket_dir = output_dir/.sockets`.
2. For each `task_desc`:
   - Checks `max_agents` and `budget_exhausted()`.
   - Creates an `AgentId`, `AgentRecord`, saves to DB, emits `AgentSpawned`.
   - Calls `pm.spawn_worker(agent_id, session_id, task, role)`.
   - On success — saves `agent_id` into `worker_ids`.
   - On error — updates the DB status, emits `AgentFailed`.

#### Step 2: Wait for completion
3. For each `agent_id` in `worker_ids`:
   - Calls `pm.wait_for_completion_with_events(&agent_id, Some(&event_tx))`.
   - Handles the result:
     - `Completed`: updates `total_tokens`, updates the DB status, emits, pushes into findings.
     - `Failed`: logs, updates the status, emits.
     - `Disconnected`: same as Failed.
     - `Err(e)`: same.

#### Step 3: Cleanup
4. Calls `pm.shutdown_all().await`.
5. Returns `findings`.

### 3.22. `synthesize(&self, findings) -> Result<String>` — async

**Algorithm:**
1. If `findings` is empty — returns "No findings were collected.".
2. Computes `headroom = context_window * 2`.
3. Creates `ResultBudget::new(headroom, findings.len(), spill_dir)`.
4. For each finding — caps it via `budget.cap_result(&summary)`, builds `"### Finding {i}\n{capped}"`.
5. Builds the synthesis prompt:
   - System: `"You are a research synthesizer.\n\n{role_prompt_for(Writer)}"`
   - User: instructions for writing the report with 5 requirements (answer the query, integrate findings, note contradictions, list sources, indicate gaps).
6. `CompletionRequest` with temperature=0.5, max_tokens=from the configuration.
7. Calls `llm.complete(&req)`.
8. Returns the response text.

### 3.23. `sync_subtask_statuses(&self)`

**Algorithm:**
1. Gets the session's agent list from the DB.
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
3. **index.md** — builds Markdown with metadata (query, date, agents, tokens) and links to finding files.
4. **findings/finding-{i}.md** — writes `summary` for each finding.
5. **sources.md** — collects unique sources from all `finding.sources`, builds the list `"- [title](url)"`. If there are no sources — `_No structured sources were recorded._`.

---

## 4. compaction.rs

**File:** `src/compaction.rs` — the context compaction engine.

### 4.1. Constants

| Constant | Value | Purpose |
|-----------|----------|------------|
| `SUMMARIZATION_OVERHEAD_TOKENS` | 4000 | Token reserve for the LLM summarization call |
| `MICRO_COMPACT_THRESHOLD_TOKENS` | 40000 | Token threshold for micro-compaction (no LLM) |
| `MAX_INEFFECTIVE_PASSES` | 2 | Maximum ineffective passes before cooldown |
| `COOLDOWN_DURATION` | 300 sec | Cooldown duration |

### 4.2. The `CompactionResult` struct

| Field | Type | Description |
|------|-----|----------|
| `messages` | `Vec<Message>` | Compacted messages |
| `tokens_before` | `u32` | Tokens before compaction |
| `tokens_after` | `u32` | Tokens after compaction |
| `cooldown_triggered` | `bool` | Whether the cooldown was triggered |
| `micro_pruned` | `u32` | Number of pruned tool messages |
| `used_llm` | `bool` | Whether LLM summarization was used |

### 4.3. The `CompactionEngine` struct

| Field | Type | Description |
|------|-----|----------|
| `ineffective_passes` | `u32` | Counter of consecutive ineffective passes |
| `cooldown_until` | `Option<Instant>` | Time when the cooldown ends |
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
1. If `cooldown_until` is `Some(until)`:
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
       - If the hash is already in `seen_content_hashes`:
         - Replaces `content` with `"[Duplicate tool result — {bytes} bytes, {tokens} tokens]"`.
         - `pruned += 1`. `continue`.
       - Otherwise — inserts the hash into the set.
     - **Prune old outputs:** if `running_tokens > MICRO_COMPACT_THRESHOLD_TOKENS && tokens > 100`:
       - Replaces `content` with `"[Tool output pruned — {bytes} bytes, {tokens} tokens. Original output was from an earlier conversation turn.]"`.
       - `pruned += 1`.
3. Returns `pruned`.

### 4.9. `compact(&mut self, messages, summarize_fn) -> Result<CompactionResult>` — async

**Algorithm:**

#### Phase 0: Preparation
1. `tokens_before = estimate_messages_tokens(messages)`.
2. Checks `is_in_cooldown()` — if so, returns a `CompactionResult` with `cooldown_triggered: true` and `tokens_after = tokens_before`.

#### Phase 1: Micro-compaction
3. `micro_pruned = self.micro_compact(messages)`.
4. `after_micro = estimate_messages_tokens(messages)`.
5. If `after_micro < threshold` — calls `update_effectiveness`, returns the result (the LLM was not used).

#### Phase 2: Split into head/middle/tail
6. Calls `split_head_middle_tail(messages)`.

#### Phase 3: LLM summarization of the middle
7. If `middle` is not empty:
   - Converts the middle to text via `messages_to_text(&middle)`.
   - Builds the prompt: system = `SUMMARIZE_SYSTEM_PROMPT`, user = `"Summarize the following conversation section concisely...\n\n{middle_text}"`.
   - Calls `summarize_fn(prompt)`.
   - On error — `"[Compaction summarization failed: {e}. Middle section removed.]"`.
8. If `middle` is empty — `summary_text = ""`.

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
2. `head_end = min(4, messages.len())` — the first 4 messages (system + the first 3).
3. `tail_start = messages.len().saturating_sub(4)` — the last 4 messages.
4. **Tool-group safety:**
   - While `head_end < messages.len()` and `messages[head_end]` is a tool message: `head_end += 1` (pulls tool results into the head).
   - While `tail_start > head_end` and `messages[tail_start]` is a tool message: `tail_start -= 1` (moves the tail start away from orphaned tool results).
5. If `tail_start <= head_end` — degenerate case: returns `(messages[..head_end], [], messages[head_end..])`.
6. Returns `(messages[..head_end], messages[head_end..tail_start], messages[tail_start..])`.

### 4.13. `messages_to_text(messages) -> String`

**Algorithm:**
For each message, builds a string:
- `System` → `"[system]: {content}\n\n"`
- `User` → `"[user]: {content}\n\n"`
- `Assistant` → `"[assistant]: {content}\n\n"` + for each tool_call: `"[assistant tool_call]: {name}({arguments})\n\n"`
- `Tool` → `"[tool result (id={id})]: {content}\n\n"` (content truncated to 2000 characters at a UTF-8 boundary).

### 4.14. `content_hash(s) -> u64`

Creates a `DefaultHasher`, hashes the string via the `Hash` trait, returns `hasher.finish()`.

### 4.15. Summarization system prompt (`SUMMARIZE_SYSTEM_PROMPT`)

Structured by sections:
- **Goal** — what the agent was trying to do.
- **Done** — key findings and completed actions.
- **Blocked** — errors, blockers, unresolved issues.
- **Next** — planned next steps.

Instruction: be concise, preserve facts, URLs, numbers, remove "fluff".

---

## 5. prompt.rs

**File:** `src/prompt.rs` — a system prompt builder with a three-layer architecture (stable/context/volatile).

### 5.1. Prompt constants

- `DEFAULT_PROMPT_BASE` — loaded from `prompts/default.txt` via `include_str!`.
- `DEEPSEEK_PROMPT_BASE` — loaded from `prompts/deepseek.txt`.
- Role blocks: `ROLE_COORDINATOR`, `ROLE_RESEARCHER`, `ROLE_ANALYST`, `ROLE_VERIFIER`, `ROLE_WRITER` — inline strings.

### 5.2. `build_env_block(config, working_dir) -> String`

**Algorithm:**
1. Determines `platform` via `cfg!(target_os = ...)` → `"darwin"` / `"linux"` / `"windows"` / `"unknown"`.
2. Determines `is_git` — checks whether `working_dir.join(".git")` exists.
3. Gets `today = Utc::now().format("%Y-%m-%d")`.
4. Builds the block:
   ```
   <env>
   Working directory: {path}
   Is git repo: {yes/no}
   Platform: {platform}
   Model: {model}
   Date: {date}
   </env>
   ```

### 5.3. The `PromptBuilder` struct

A three-layer builder:
- `stable: Vec<String>` — the stable layer (cached between sessions).
- `context: Vec<String>` — the context layer (stable within a session).
- `volatile: Vec<String>` — the volatile layer (changes every turn).

### 5.4. `PromptBuilder::new(role, task, depth, max_depth, model)`

**Algorithm:**
1. Selects the base prompt via `select_model_base(model)`.
2. Gets the role prompt via `role_prompt_for(role)`.
3. Builds the stable layer: `[base, "## Your Role\n{role}\n\n## Current Task\n{task}\n\nDepth: {depth}/{max_depth}"]`.

### 5.5. `add_env(&mut self, config, working_dir)`

Calls `build_env_block()`, pushes the result into `context`.

### 5.6. `add_tools(&mut self, tools)`

**Algorithm:**
1. If `tools` is empty — does nothing.
2. Builds the section `"## Available Tools\n\n"`.
3. For each tool: `"### {name}\n{description}\n\n"`.
4. Pushes into `volatile`.

### 5.7. `add_stable_instruction(&mut self, instruction)` / `add_volatile_block(&mut self, block)`

Simply push the string into the corresponding layer.

### 5.8. `build(&self) -> String`

Concatenates all layers in the order: stable → context → volatile, separated by `"\n\n"`.

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
- **Analyst** — analysis framework (cross-referencing, patterns, contradictions, reliability assessment).
- **Verifier** — verification approach (independent sources, primary sources, consensus, VERIFIED/LIKELY/UNVERIFIED/CONTRADICTED statuses).
- **Writer** — report writing guide (structure, formatting, quality standards).

---

## 6. prompts/

### 6.1. `prompts/default.txt`

The base prompt for most models (GPT-4, Claude, etc.). Contains:
- Identification: "You are an autonomous worker working within the Fathom system."
- General instructions: use tools, do not fabricate, be precise, record problems.
- Behavioral guidelines: think step by step, break tasks down, try alternatives, use markdown.

### 6.2. `prompts/deepseek.txt`

An abridged version for DeepSeek — more laconic, with a focus on tools. The same core principles, but without verbose explanations.

---

## 7. hooks.rs

**File:** `src/hooks.rs` — lifecycle hooks (fleet E3, ZCode pattern). Subprocesses invoked at certain stages of the agent loop.

### 7.1. Constants

- `MAX_STOP_CONTINUATIONS = 3` — the maximum number of forced continuations by Stop hooks per run.

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
2. Additionally filters: `h.tool` is empty (wildcard — matches any tool) **OR** `h.tool` matches `tool` (case-insensitive).
3. Returns the filtered vector of references.

**Usage:** used to filter PreToolUse, PostToolUse, and Stop hooks by event and tool name.

### 7.4. `run_hook(cmd, input_json) -> Option<String>` — async

**Algorithm:**
1. Gets the shell path: `SHELL` from the environment (fallback `/bin/bash`).
2. Creates `Command::new(&shell_path)` with arguments `["-c", cmd]`.
3. Sets `stdin` to `piped`.
4. Captures `stdout` and `stderr` as `piped`.
5. Tries `cmd.spawn()`:
   - On error — logs a warning, returns `None`.
6. Via `child.stdin.take()` writes `input_json` + newline, then closes stdin.
7. Creates the future `child.wait_with_output()`.
8. Wraps it in `tokio::time::timeout(Duration::from_secs(30), output_future)`.
9. On `Elapsed` — calls `child.kill().await` (kill_on_drop), logs a warning, returns `None`.
10. On timeout error — logs a warning, returns `None`.
11. On `Ok(Ok(output))`:
    - If `!output.status.success()` — logs stderr, returns `None`.
    - Concatenates stdout + stderr → `all`.
    - Strips the trailing newline (if any).
    - If `all.is_empty()` — returns `None`.
    - Otherwise — returns `Some(all)`.

### 7.5. `run_pre_tool_hooks(hooks, tool_name, tool_args_json) -> PreToolVerdict` — async

**Algorithm:**
1. Filters hooks: `hooks_for(hooks, "pre_tool_use", tool_name)`.
2. If there are no matching hooks — returns `PreToolVerdict::Allow`.
3. Builds the input JSON: `{ "event": "pre_tool_use", "tool": tool_name, "args": {tool_args_json}, "timestamp": iso8601 }`.
4. For each hook in the filtered list:
   - Calls `run_hook(&h.cmd, &input_json).await`.
   - If `run_hook` returned `None` — `continue` (the hook does not block).
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
2. If there are no matches — returns `None`.
3. Builds the input JSON: `{ "event": "post_tool_use", "tool": tool_name, "args": {tool_args_json}, "output": tool_output, "success": bool, "timestamp": iso8601 }`.
4. For each hook:
   - Calls `run_hook(&h.cmd, &input_json).await`.
   - If `Some(extra_info)` — appends the string `"\n\n[Hook {i+1}]: {extra_info}"` to `accumulated_output`.
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
2. If there are no matches — returns `StopVerdict::Stop`.
3. Computes `char_count = summary_so_far.chars().count()`.
4. Builds the input JSON: `{ "event": "stop", "tool": null, "output_preview": {preview up to 4000 characters}, "output_char_count": char_count, "timestamp": iso8601 }`.
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

### 7.8. The `HookConfig` struct

| Field | Type | Description |
|------|-----|----------|
| `cmd` | `String` | Shell command to execute |
| `event` | `String` | Event type: `"pre_tool_use"`, `"post_tool_use"`, `"stop"` |
| `tool` | `String` | Tool name (empty string = wildcard) |

---

## 8. ipc.rs

**File:** `src/ipc.rs` — the inter-process message protocol between the coordinator and worker processes over Unix Domain Sockets.

### 8.1. The `IpcMessage` enum

All variants are serialized/deserialized via `serde`:

#### `IpcMessage::Task { agent_id, session_id, task, role }`
Purpose: coordinator → worker. Passes the assignment to the worker.

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
Purpose: worker → coordinator. Forwards an `AgentEvent` to the coordinator's bus.

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
2. Appends a newline (`\n`).
3. Calls `stream.write_all(json.as_bytes()).await?`.

### 8.3. `to_agent_event(ipc_msg, session_id) -> Option<AgentEvent>`

**Algorithm:**
Matches on the `IpcMessage` variant:

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

**Note:** several fields are filled with default values, since IPC messages contain a simplified set of fields.

---

## 9. process_manager.rs

**File:** `src/process_manager.rs` — worker process management for multiprocess fan-out.

### 9.1. The `ProcessManager` struct

| Field | Type | Purpose |
|------|-----|------------|
| `socket_dir` | `PathBuf` | Directory for Unix Domain Socket files |
| `children` | `HashMap<AgentId, Child>` | Live child processes |
| `streams` | `HashMap<AgentId, OwnedWriteHalf>` | Active write streams to workers |

### 9.2. `ProcessManager::new(socket_dir)`

**Algorithm:**
1. `create_dir_all(&socket_dir)`.
2. Initializes empty `children` and `streams`.

### 9.3. `spawn_worker(&mut self, agent_id, session_id, task, role) -> Result<()>` — async

**Algorithm:**
1. Builds `socket_path = socket_dir/{agent_id}.sock`.
2. Removes the old socket file if it exists (`fs::remove_file`).
3. Creates `UnixListener::bind(&socket_path)?`.
4. Determines the path to the `pr-agent-worker` binary (searches next to the current binary or in `$CARGO_MANIFEST_DIR/../../target/release|debug/`).
5. If the binary is not found — `bail!`.
6. Creates `Command::new(&worker_binary)`:
   - Arguments: `["--socket", &socket_path, "--agent-id", &agent_id, "--session-id", &session_id, "--task", &task, "--role", &role]`.
   - `stdin(Stdio::null())`, `stdout(Stdio::piped())`, `stderr(Stdio::piped())`.
7. Launches via `.spawn()?`.
8. Saves `child` into `children`.
9. Runs `tokio::time::timeout(30s, listener.accept())` — waits for the worker to connect.
10. On `Elapsed` — `bail!("Worker {agent_id} did not connect within 30s")`.
11. On successful `accept` — gets `(stream, _addr)`.
12. Creates `tokio::io::split(stream)` → `(read_half, write_half)`.
13. Saves `write_half` into `streams`.
14. Runs `tokio::spawn(Self::stream_events(read_half, event_tx))` to read IPC messages from the worker's stream.
15. Logs the successful launch.

### 9.4. `send_to(&mut self, agent_id, msg) -> Result<()>` — async

**Algorithm:**
1. Looks up `stream` in `streams` by `agent_id`.
2. If not found — `bail!("No stream for agent {agent_id}")`.
3. Calls `write_msg(stream, msg).await`.

### 9.5. `shutdown_all(&mut self) -> Result<()>` — async

**Algorithm:**
1. For each `(agent_id, stream)` in `streams`:
   - Tries to send `IpcMessage::Cancel { agent_id }`.
   - Errors are ignored (the worker may have already exited).
2. Clears `streams`.
3. For each `(agent_id, mut child)` in `children`:
   - Runs `tokio::time::timeout(5s, child.wait())`.
   - On `Elapsed` — logs a warning, calls `child.kill().await`.
4. Clears `children`.

**Sequence: Cancel → 5s grace → kill → reap.**

### 9.6. `wait_for_completion_with_events(&mut self, agent_id, event_tx) -> Result<AgentOutput>` — async

**Algorithm:**
1. Extracts `write_half` from `streams` (or bail).
2. Sends `IpcMessage::Ack` with the message `"waiting for completion"`.
3. Creates `BufReader::new(read_half)`.
4. Infinite `tokio::select!` loop:
   - **`read_line_capped(&mut reader, 16MB)` branch:**
     - Deserializes the line as `IpcMessage`.
     - **`Ready`** — logs.
     - **`Event { event }`** — if `event_tx` is set, relays it via `to_agent_event()` and `event_tx.send()`.
     - **`Result { status, summary, tokens_used, error }`** — builds an `AgentOutput` based on the status (Completed / Failed / Disconnected), returns it.
     - **Read error** — logs, returns an `AgentOutput` with `aborted: true`.
   - **`child.wait()` branch (via mutable access):**
     - If the process exited before the Result arrived — logs, returns an `AgentOutput` with `aborted: true`.

### 9.7. `stream_events(read_half, event_tx)`

**Algorithm:**
1. Creates `BufReader::new(read_half)`.
2. `read_line_capped(&mut reader, 16MB)` loop:
   - If `Ok(line)` — deserializes, relays via `to_agent_event()`, sends to `event_tx`.
   - If `Err(e)` — logs, break.
3. Logs the end of the stream.

### 9.8. `read_line_capped(reader, max_bytes) -> Result<String>`

**Algorithm:**
1. Creates an empty `String` buffer.
2. `reader.read_line(&mut line)` loop:
   - If `Ok(0)` — EOF, `bail!("unexpected EOF")`.
   - If `Ok(_)` — checks `line.len() > max_bytes`:
     - If so — `bail!("IPC line too long: {len} bytes (max {max_bytes})")`.
   - Checks `line.ends_with('\n')`:
     - If so — strips the trailing newline (and `\r` if present), returns the string.
3. On read error — `bail!`.

**Purpose:** protection against OOM when reading from the IPC stream (a worker must not send messages > 16MB).

---

## 10. budget.rs

**File:** `src/budget.rs` — the tool output budget. Limits output volume per turn and per run.

### 10.1. Constants

| Constant | Value | Purpose |
|-----------|----------|------------|
| `DEFAULT_MAX_BYTES` | 250,000 | Maximum bytes per single result |
| `DEFAULT_MAX_LINES` | 5,000 | Maximum lines per single result |
| `DEFAULT_SHELL_MAX_BYTES` | 200,000 | Maximum for shell output |
| `DEFAULT_SHELL_MAX_LINES` | 4,000 | Maximum lines for shell |
| `SPILL_DIR_NAME` | `.pr-context` | Directory name for spill files |
| `MAX_SPILL_FILES_PER_AGENT` | 20 | Maximum number of spill files |

### 10.2. The `Truncated` enum

```rust
enum Truncated {
    Unchanged,
    Truncated { replacement: String },
}
```

### 10.3. `Truncated::content(&self, original) -> &str`

Returns `replacement` if truncated, otherwise `original`.

### 10.4. `apply_turn_budget(tool_name, result, max_bytes, max_lines, turn_budget, working_dir) -> Truncated`

**Algorithm:**
1. Copies `result.content` into `content`.
2. Determines `bytes_budget` and `lines_budget`:
   - If `tool_name == "shell"` — uses `min(max_bytes, DEFAULT_SHELL_MAX_BYTES)` and `min(max_lines, DEFAULT_SHELL_MAX_LINES)`.
   - Otherwise — uses `min(max_bytes, DEFAULT_MAX_BYTES)` and `min(max_lines, DEFAULT_MAX_LINES)`.
3. **Budget check:** if `turn_budget.remaining() < bytes_budget` — uses `min(turn_budget.remaining(), bytes_budget)` as the new `bytes_budget`.
4. If `bytes_budget == 0` — returns `Truncated::Truncated` with the message `"[Tool output dropped: turn output budget exhausted]"`.
5. **Truncate by lines:**
   - Splits `content` by newlines.
   - If `lines.len() > lines_budget` — truncates to `lines_budget`, appends `"\n\n[Truncated — {N} lines omitted. Re-run with the 'offset' argument to continue.]"`.
   - Updates `content`.
6. **Truncate by bytes:**
   - If `content.len() > bytes_budget`:
     - Truncates at a UTF-8 boundary.
     - Saves `full_len = content.len()`.
     - Saves the hash of the original content: `{tool_name}:{content_hash:x}:{full_len}`.
     - **Spill to disk:**
       - Creates the directory `{working_dir}/{SPILL_DIR_NAME}/{agent_id}`.
       - Counts existing spill files (glob `*.txt`).
       - If < `MAX_SPILL_FILES_PER_AGENT`:
         - Builds the name `{tool_name}_{timestamp}_{hash}.txt`.
         - Writes the full content to the file.
         - Logs.
       - If >= the limit — logs a warning (does not save).
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
7. **Charge to the budget:** `turn_budget.record(content.len() as u64)`.
8. Returns `Truncated::Truncated { replacement: content }` or `Truncated::Unchanged`.

### 10.5. The `TurnBudget` struct

| Field | Type | Description |
|------|-----|----------|
| `limit_bytes` | `u64` | Total limit per turn |
| `used_bytes` | `u64` | Bytes spent |

### 10.6. `TurnBudget::new(limit)` / `record(bytes)` / `remaining()`

- `new(limit)` — initializes with `used_bytes = 0`.
- `record(bytes)` — increments `used_bytes`.
- `remaining()` — returns `limit_bytes.saturating_sub(used_bytes)`.

### 10.7. The `ResultBudget` struct

| Field | Type | Description |
|------|-----|----------|
| `headroom_chars` | `usize` | Free context space in characters |
| `per_result_chars` | `usize` | Limit per single result |
| `spill_dir` | `PathBuf` | Directory for spill files |
| `agent_dir` | `PathBuf` | Spill file directory for a specific agent |

### 10.8. `ResultBudget::new(headroom, batch_len, spill_dir)`

**Algorithm:**
1. `per_result_chars = headroom / max(batch_len, 1)`. Minimum 4000.
2. `agent_dir = spill_dir/agent`.
3. `create_dir_all(&agent_dir)`.
4. Counts existing spill files: `existing = agent_dir.read_dir().count()`.
5. If `existing >= 20` — logs a warning (spill disabled).

### 10.9. `cap_result(&self, text) -> String`

**Algorithm:**
1. If `text.len() <= per_result_chars` — returns `text.to_string()`.
2. Computes the boundary at `per_result_chars` (at a UTF-8 boundary).
3. Creates the spill file `{agent_dir}/finding_{timestamp}_{short_hash}.txt`.
4. Writes the full text to the file.
5. Logs.
6. Returns the truncated text plus a suffix:
   ```
   ... [truncated — {total} chars; full text saved to: {spill_path}].
   ```

---

## 11. doom_loop.rs

**File:** `src/doom_loop.rs` — the agent loop detector.

### 11.1. Constants

| Constant | Value | Purpose |
|-----------|----------|------------|
| `MAX_HISTORY` | 6 | Size of the sliding window |
| `THRESHOLD` | 3 | Trigger threshold (3+ identical) |
| `MAX_NUDGE_CONTINUATIONS` | 3 | Nudge continuation limit |

### 11.2. The `ToolInvocation` struct

| Field | Type | Description |
|------|-----|----------|
| `tool_name` | `String` | Tool name |
| `args_hash` | `u64` | Hash of the arguments |

### 11.3. The `DoomLoopDetector` struct

| Field | Type | Description |
|------|-----|----------|
| `history` | `VecDeque<ToolInvocation>` | Sliding window of the last 6 invocations |
| `consecutive_same` | `u32` | Counter of consecutive identical invocations |
| `nudge_count` | `u32` | Counter of nudge continuations |

### 11.4. `DoomLoopDetector::new()`

Creates an empty `VecDeque` with capacity `MAX_HISTORY`, zeroes the counters.

### 11.5. `args_hash(args) -> u64`

**Algorithm:**
1. Tries to deserialize `args` into `serde_json::Value`.
2. If `Value::Object(map)`:
   - Converts to `BTreeMap<String, Value>` (key sorting).
   - Serializes back to a string.
   - Hashes via `DefaultHasher`.
   - Returns `hasher.finish()`.
3. In other cases (non-object) — hashes the raw string.
4. On deserialization error — hashes the raw string.

**Purpose:** guarantees that `{"a":1,"b":2}` and `{"b":2,"a":1}` produce the same hash.

### 11.6. `record_and_check(&mut self, tool_name, args_json) -> Option<String>`

**Algorithm:**
1. Computes `hash = args_hash(args_json)`.
2. Creates `invocation = ToolInvocation { tool_name, args_hash: hash }`.
3. **Duplication check:**
   - Gets the last element from `history` via `back()`.
   - If `last.tool_name == tool_name && last.args_hash == hash`:
     - `consecutive_same += 1`.
   - Otherwise — `consecutive_same = 1`.
4. Pushes `invocation` into `history`.
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

### 11.7. `record_nudge(&mut self)`

Increments `nudge_count`.

### 11.8. `has_exceeded_nudge_limit(&self) -> bool`

Returns `nudge_count >= MAX_NUDGE_CONTINUATIONS`.

### 11.9. `reset(&mut self)`

Resets `history`, `consecutive_same`, and `nudge_count` to their initial state.

---

## 12. resume.rs

**File:** `src/resume.rs` — a CLI utility for selecting and launching session resumption.

### 12.1. The `SessionSummary` struct

| Field | Type | Description |
|------|-----|----------|
| `session_id` | `SessionId` | Session ID |
| `query` | `String` | Query |
| `created_at` | `DateTime<Utc>` | Creation date |
| `completed_agents` | `u32` | Completed agents |
| `failed_agents` | `u32` | Failed agents |
| `running_agents` | `u32` | Running agents |
| `total_tokens` | `u64` | Total tokens |
| `sub_tasks` | `Vec<SubtaskRecord>` | Subtasks |

### 12.2. The `ResumeOption` enum

```rust
enum ResumeOption {
    Resume(RecoveredSession),
    Fresh,
    Exit,
}
```

### 12.3. `format_summary(summary, index) -> String`

**Algorithm:**
1. Computes `age` — the difference between `Utc::now()` and `created_at`.
2. Formats the age in a human-readable way: `< 1 hour`, `N hours`, `N days`.
3. Computes `done_pct = completed_agents * 100 / total_agents`.
4. Builds the string:
   ```
   [{index}] {query (first 60 chars)}... ({age} ago)
       Agents: {completed} done / {failed} failed / {running} running ({done_pct}%)
       Tokens: {total_tokens}
   ```

### 12.4. `interactive_select(summaries) -> ResumeOption`

**Algorithm:**
1. If `summaries` is empty — logs "No resumable sessions found", returns `Fresh`.
2. Prints the header: `"Found {n} resumable session(s):"`.
3. For each summary with an index — prints `format_summary(summary, i+1)`.
4. Prints the options: `"[N] Resume session N`, `[F] Start fresh`, `[Q] Quit"`.
5. Reads stdin:
   - Empty input — `Exit`.
   - `"f"` or `"F"` — `Fresh`.
   - `"q"` or `"Q"` — `Exit`.
   - A number — parses it, checks `1 <= n <= summaries.len()`, returns `Resume(summaries[n-1])`.
   - Invalid input — `Exit`.

### 12.5. `handle_resume_interactive(db, config, event_tx) -> Option<Coordinator>` — async

**Algorithm:**
1. Creates `SessionResumer::new(db, config, event_tx)`.
2. Calls `resumer.find_resumable()`.
3. If `summaries` is empty — returns `None`.
4. Calls `interactive_select(&summaries)`.
5. Matches the result:
   - **`Fresh`** — returns `None`.
   - **`Exit`** — `process::exit(0)`.
   - **`Resume(selected)`:**
     - Calls `resumer.resume(&selected.session_id).await`.
     - Creates `Coordinator::new(...)` with the same `session_id` and the `query` from the recovered session.
     - Connects `contact_db`, `crm`, `steer_rx`.
     - Returns `Some(coordinator)`.

---

## 13. tool_executor.rs

**File:** `src/tool_executor.rs` — the parallel tool executor. Classifies tool_calls into concurrent and sequential, runs parallel ones simultaneously with path-overlap detection.

### 13.1. The `ToolCategory` enum

```rust
enum ToolCategory {
    ReadOnly,   // search_code, fetch_url, read_file, get_contacts
    Write,      // write_file, append_file, update_record
    Shell,      // shell
    Unknown,    // everything else
}
```

### 13.2. `classify_tool(name) -> ToolCategory`

**Algorithm:**
1. Converts `name` to lowercase.
2. Matches:
   - `"search_code"` | `"fetch_url"` | `"read_file"` | `"get_contacts"` → `ReadOnly`
   - `"write_file"` | `"append_file"` | `"update_record"` → `Write`
   - `"shell"` → `Shell`
   - `_` → `Unknown`

### 13.3. `extract_paths(tool_name, args) -> Vec<String>`

**Algorithm:**
1. Deserializes `args` into `serde_json::Value`.
2. If not an object — returns an empty vector.
3. Creates a `HashSet<String>` for unique paths.
4. For each key in `{"path", "file", "file_path", "source", "destination"}`:
   - If the field exists and is a string — inserts it into the set.
5. For the keys `"paths"` and `"files"`:
   - If the field is an array of strings — inserts all of them.
6. Collects into a `Vec`, normalizes the path:
   - Strips the trailing `/`.
   - Strips the `./` prefix.
7. Returns the vector.

### 13.4. `paths_overlap(a, b) -> bool`

**Algorithm:**
1. For each path from `a` and each path from `b`:
   - If `pa == pb` — `return true`.
   - If `pa.starts_with(pb)` **OR** `pb.starts_with(pa)` — `return true`.
2. If no comparison matched — `return false`.

**Example:** `["/tmp/foo.txt"]` and `["/tmp/foo.txt"]` → true. `["/tmp/data"]` and `["/tmp/data/file.txt"]` → true.

### 13.5. The `PartitionedBatch` struct

| Field | Type | Description |
|------|-----|----------|
| `sequential` | `Vec<(usize, String, String)>` | `(index, name, args)` — must run sequentially |
| `concurrent` | `Vec<(usize, String, String)>` | `(index, name, args)` — may run in parallel |
| `conflict_groups` | `Vec<Vec<usize>>` | Groups of indices conflicting over paths |

### 13.6. `partition_batch(tool_calls) -> PartitionedBatch`

**Algorithm:**
1. Classifies each `tool_call` via `classify_tool`.
2. **Shell and Unknown** → always in `sequential`.
3. **Write** — checks for conflicts:
   - If `sequential` already contains a Write with overlapping paths — adds to `sequential`.
   - Otherwise — adds to `concurrent`.
4. **ReadOnly** → adds to `concurrent`.
5. Builds `conflict_groups` from the `sequential` indices.
6. Returns `PartitionedBatch`.

### 13.7. `execute_parallel(tools, concurrent, tool_ctx) -> Vec<(usize, ToolOutput)>` — async

**Algorithm:**
1. If `concurrent` is empty — returns an empty vector.
2. For each `(idx, name, args)` in `concurrent`:
   - Creates the future `tools.execute(&name, &args, tool_ctx)`.
3. Runs all futures via `futures::future::join_all(futs).await`.
4. Collects the results: `(idx, output)` for each.
5. Returns the vector of pairs.

### 13.8. `execute_sequential(tools, sequential, tool_ctx) -> Vec<(usize, ToolOutput)>` — async

**Algorithm:**
1. Creates `results: Vec`.
2. For each `(idx, name, args)` in `sequential`:
   - Calls `tools.execute(&name, &args, tool_ctx).await`.
   - Pushes `(idx, output)`.
3. Returns `results`.

### 13.9. `execute_batch(tools, tool_calls, tool_ctx) -> Vec<(usize, ToolOutput)>` — async

**Algorithm:**
1. Calls `partition_batch(tool_calls)`.
2. Runs `execute_parallel(tools, concurrent, tool_ctx)` and `execute_sequential(tools, sequential, tool_ctx)` in parallel via `tokio::join!`.
3. Creates `all_results: Vec`.
4. Concatenates the results: first `seq_results`, then `par_results`.
5. Sorts `all_results` by `idx` (restores the original order).
6. Returns `all_results`.

---

## Overall architectural picture

### Session lifecycle

```
CLI → Coordinator::execute()
  → plan()                    # LLM decomposition into subtasks
  → fan-out:
    - spawn_researchers()     # in-process (tokio tasks)
    - run_multiprocess_fanout() # out-of-process (Unix sockets)
  → synthesize()              # LLM report assembly
  → write_output()            # Files: summary.md, index.md, sources.md, findings/
```

### Agent lifecycle (runtime loop)

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

### Context compaction system

```
should_compact? (tokens >= threshold)
  → is_in_cooldown? → skip
  → micro_compact (no LLM):
    - deduplication by content_hash
    - prune old tool output > 40000 tokens
  → if still above the threshold:
    - split head(4) / middle / tail(4) with tool-group safety
    - LLM summarize middle → system message
    - reassemble: head + summary + tail
  → update_effectiveness:
    - < 5% reduction → ineffective_passes++
    - 2 ineffective passes → cooldown 300 sec
```

### Hooks system

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

### Doom loop detection system

```
Sliding window (last 6 invocations)
  → args_hash via serde_json BTreeMap normalization
  → 3+ consecutive identical → DETECTED
  → First time: NUDGE (warn, continue)
  → Second time: STOP (abort agent)
```