# Architecture

Fathom is a modular system of **12 crates** in a Cargo workspace. The core is built around an async agent loop on `tokio` with a broadcast-channel event bus, a pluggable tool execution pipeline, and a coordinator that decomposes work across sub-agents. Every component is designed for observability, crash recovery, and graceful degradation. On top of research and outreach, Fathom is a universal autonomous AI worker: it can operate a real browser (computer use), enforce governed policy with an audit trail, and run as persistent scheduled coworkers.

---

## Crate Overview

```
fathom/
├── crates/
│   ├── core/          # Fundamental types and domain logic
│   ├── llm/           # LLM provider abstraction
│   ├── agent/         # Agent runtime, coordination, control plane
│   ├── tools/         # 63 tools (57 built-in + 6 computer)
│   ├── memory/        # Long-term semantic memory + entity graph
│   ├── mcp/           # Model Context Protocol (client and server)
│   ├── persistence/   # Data storage (SQLite, connection pool, jobs)
│   ├── server/        # HTTP API
│   ├── tui/           # Terminal interface
│   ├── lsp/           # Language server protocol (editor integration)
│   ├── governance/    # Policy engine — allow/deny rules, audit decision records
│   └── supervisor/    # Docker per-agent computer provisioning
└── src/main.rs        # CLI entry point
```

### Crate Dependencies

```
core  ←──  llm  ←──  agent  ←──  server
  ↑          ↑          ↑           ↑
  └── tools ─┴── mcp ───┴── tui ────┘
       ↑          └── persistence ──┘
       └── memory ──┘ (depends on core + llm)

governance ──── agent (hooks into tool execution)
supervisor ──── server (provisions Docker containers for computer use)
```

`core` depends on nothing (foundation). `agent` combines `llm`, `tools`, `memory`, `persistence`, and hooks into `governance` for policy enforcement. `supervisor` provides Docker-based computer provisioning to `server`. The dependency graph is a strict DAG — there are no circular dependencies, which keeps compilation fast and makes each crate independently testable. The `lsp` crate provides editor integration via the Language Server Protocol, allowing IDEs to inspect sessions, memory, and submit queries from within the editor.

---

## crates/core

Fundamental types and domain logic. **Does not depend on other crates.** Every other crate imports from `core` for its shared types, configuration structures, and error types.

| Module | Purpose |
|--------|---------|
| `ids` | `SessionId`, `AgentId`, `FindingId` (UUID v7, time-ordered for efficient B-tree indexing) |
| `message` | OpenAI-compatible `Message`, `ToolCall` — the canonical message format used throughout the system |
| `agent` | `AgentRole`, `AgentState`, `AgentStatus`, `AgentRecord` — role definitions (researcher, analyst, verifier, writer, coordinator) and state machine |
| `irc` | `IrcBus` — peer-to-peer message bus between agents; `AgentRegistry`, `SteerRegistry`, `AsyncJobManager`, `DaemonRegistry` — runtime registries |
| `steer` | Steer instructions — operator/parent mid-run directives delivered at the next turn boundary |
| `async_job` | Durable async job coordination (`AsyncJobManager`) — submit, monitor, and collect results of background agents |
| `daemon` | Long-running daemon agents (`DaemonRegistry`) — persistent background tasks with restart semantics |
| `protected` | Protected tool/interaction mechanisms |
| `profile` | Agent profile definitions |
| `capability` | Capability registry — declared tool/interaction capabilities of an agent |
| `event` | `AgentEvent` — all events for the broadcast bus (session lifecycle, agent lifecycle, tool calls, LLM streaming, control-plane requests) |
| `finding` | `Finding`, `Source` — structured research results with source attribution, confidence levels, and category tags |
| `config` | `AppConfig` and all 10 config sections (agent, llm, tools, memory, persistence, server, tui, export, notify, crm) |
| `memory` | `MemoryStore` (MEMORY.md/USER.md), typed memories with scopes |
| `skill` | `Skill`, `SkillRegistry` (SKILL.md) — loadable skill definitions from the filesystem |
| `export` | `Exporter` — PDF/HTML/JSON/DOCX export with templating |
| `notify` | `Notifier` — webhook/email/Telegram notifications on session completion |
| `crm` | `CrmSync` — amoCRM/Bitrix24/HubSpot CRM integration |
| `session` | `SessionOutput` — session result structure consumed by export, notify, and CRM subsystems |
| `token` | Accurate token counting (tiktoken cl100k_base) + CJK-aware heuristic fallback for models without BPE data |
| `error` | `PrError`, `PrResult` — unified error type with context (source location, chain of causes) |

The `config` module loads from `~/.fathom/config.toml` (or a custom path via `--config`) and merges environment variable overrides for every section. Each config section maps to a subsystem and can be reloaded at runtime (the `server` and `tui` hosts watch for SIGHUP or config file changes).

---

## crates/llm

Abstraction of LLM providers. The crate defines a single `LlmProvider` trait that all providers implement, making the agent runtime provider-agnostic.

- **`LlmProvider` trait** — `complete()` and `stream()` methods
  - `complete()`: sends a full message list, returns the model's response (text + optional tool calls). Used for the ordinary turn.
  - `stream()`: sends a full message list, returns a `Stream<Item = LlmDelta>` where each delta is either a text chunk, a partial tool-call fragment, or a finish signal. The runtime assembles streaming deltas into complete tool calls and text blocks.
- **`DeepSeekProvider`** — OpenAI-compatible API
  - Retry with exponential backoff (3 attempts, base delay 1s, jitter)
  - Streaming fallback for large responses: if the initial stream fails, the runtime falls back to `complete()`
  - Response size limits (50MB) — protects against runaway model output
  - HTTP timeout (5 min) — configurable per provider
- **`retry`** — generic `with_retry()` helper that retries on transient errors (network errors, 5xx, rate limits) with exponential backoff and jitter, while propagating permanent errors (4xx, auth failures) immediately

The trait is designed so that adding a new provider (e.g., Anthropic, Google Gemini) requires implementing only two async methods. The `select_model_base()` function in `agent::prompt` chooses the appropriate system prompt template per model family.

---

## crates/agent

The heart of the system — the agent runtime. This crate orchestrates the LLM call loop, tool execution, sub-agent coordination, context management, and the control plane for human-in-the-loop interaction.

| Module | Purpose |
|--------|---------|
| `runtime` | `AgentRuntime` — the main loop: system prompt → LLM → tool calls → execute → repeat |
| `lifecycle` | `AgentLifecycleManager` — park/revive agent lifecycle (suspend agent to disk and restore) |
| `coordinator` | `Coordinator` — planning, fan-out, Goal Mode, synthesis, session lifecycle |
| `compaction` | Hermes-style context compression (micro-compaction without LLM, full compaction with LLM summarization) |
| `prompt` | `PromptBuilder` — 3 cache tiers (stable/context/volatile), role-specific prompts, model base selection |
| `tool_executor` | Smart parallelism engine (read-only tools run concurrently, write tools serialize, path-overlap detection) |
| `budget` | Result budget capping for sub-agent outputs (fair-share truncation with spill-to-disk) |
| `control` | Control plane: question/approval request types, oneshot reply channels, timeout handling |
| `ipc` | IPC protocol for multi-process mode (Unix domain socket, JSON-line messages) |
| `process_manager` | `ProcessManager` — spawn/monitor worker processes, socket lifecycle, kill-on-drop guarantees |
| `doom_loop` | `DoomLoopDetector` — protection against infinite looping (3 consecutive identical tool calls) |
| `resume` | `SessionResumer` — session resumption after crash (reconstructs state from the database) |
| `hooks` | PreToolUse/PostToolUse/Stop subprocess hooks (ZCode pattern, best-effort external policy enforcement) |

### Agent Loop

```
┌─────────────────────────────────────────────────┐
│ 1. Build system prompt (3 cache tiers +         │
│    memory digest for depth-0)                   │
│ 2. Check doom loop                              │
│ 3. Estimate tokens (tiktoken BPE) → compact     │
│ 4. Call LLM (stream → deltas into events;       │
│    tool calls assembled from fragments;          │
│    fallback to complete on stream failure)       │
│ 5. Gates: role deny → approval (approval_tools) │
│    → PreToolUse hooks                           │
│ 6. If tool_calls → execute (parallel)           │
│    ├─ question → operator response              │
│    ├─ spawn_agent → child agents                │
│    ├─ autosave contacts + absorb into memory    │
│    └─ truncate results, append to messages      │
│ 7. Loop until no tool_calls or max_iter         │
│ 8. Return final answer                          │
└─────────────────────────────────────────────────┘
```

**Step-by-step detail:**

1. **Prompt construction** — `PromptBuilder` assembles three tiers: *stable* (identity, role, general instructions — persists across sessions for cache hits), *context* (cwd, platform, model, date — per session), and *volatile* (tools list, skills, memory digest — changes every turn). For depth-0 agents, a memory digest is injected from the long-term memory store.

2. **Doom loop check** — `DoomLoopDetector` compares the incoming tool call signature (name + hash of args) against the last N calls. Three identical calls in a row triggers a stop, preventing the model from burning tokens on a failing operation.

3. **Token estimation** — Uses `tiktoken` cl100k_base BPE encoding for accurate counting. If the estimated tokens exceed `context_window * compact_threshold` (default 50%), the compaction engine kicks in.

4. **LLM call** — The runtime calls `stream()` on the active provider. Every text delta is forwarded to the event bus as `LlmStreamChunk` events (consumed by the TUI and HTTP SSE endpoints). Tool-call fragments are assembled incrementally — if the model emits a partial tool call (e.g., function name first, then arguments), the runtime collects fragments until the call is complete. If streaming fails mid-response, the runtime falls back to `complete()`.

5. **Gates** — Before any tool call is executed, it passes through four gates in order:
   - **Governance policy** — the policy engine evaluates `<tool, target>` against allow/deny rules; a `deny` verdict or an unmatched (fail-closed) pair blocks the call and is written to the audit trail
   - **Role deny** — per-role deny lists from `[agent.role_deny]` config (e.g., a `researcher` may not run `shell`)
   - **Approval** — tools listed in `[agent] approval_tools` require operator approval before execution
   - **PreToolUse hooks** — subprocess hooks that can deny the call with a reason

6. **Tool execution** — `ToolExecutor` partitions calls into parallel-safe and sequential groups. Read-only tools (web_search, web_fetch, file_read, glob, grep, OSINT lookups) run concurrently via `futures::future::join_all`. Write tools (file_write, file_edit, shell, spawn_agent) take exclusive access. Path-overlap detection serializes file tools operating on the same path. The `question` tool is special: it blocks the agent loop, emits a `QuestionAsked` event, and waits for the operator to respond via the control plane (HTTP endpoint or TUI input).

    On turn boundaries, the runtime drains:
    - **Steering** — operator/parent steer directives from `SteerRegistry`
    - **Peer steering** — peer-to-peer steer directives from `IrcBus`
    - **IrcBus inbox** — peer messages delivered via the inter-agent message bus
    - **AsyncJobManager results** — completed background async-job outputs

    Before the first turn, `register_with_bus()` advertises the agent on the IrcBus; after the final turn, `unregister_from_bus()` removes it.

7. **Loop** — The runtime repeats until the model produces no tool calls, or `max_iterations` (configurable, default 30) is reached. Each iteration appends the tool results to the message list and rebuilds the volatile prompt tier.

8. **Return** — The final text response is returned as the agent's output, along with total token usage, findings, and any collected contacts.

### Coordinator Flow (Full Goal Mode)

```
Query
  ↓
Plan (LLM decomposes into sub-tasks; subtasks in DB)
  ↓
Fan-out (spawn sub-agents: JoinSet or ProcessManager)
  ↓
Collect results (budget-capped summaries)
  ↓
Reflection (lead-gen: gap-round when contacts fall short of target)
  ↓
Goal Mode (up to replan_rounds rounds):
  LLM judge compares result against goal →
  new gap-filling sub-tasks for specific gaps
  ↓
Synthesize (LLM merges findings)
  ↓
Write output (index.md, summary.md, findings/) + absorb results into memory
  ↓
Export + Notify
```

**Coordinator lifecycle in detail:**

- **Planning** — The coordinator agent (special role `AgentRole::Coordinator`) receives the user query and decomposes it into 2–5 self-contained sub-tasks. Each sub-task is persisted to the database with metadata (role, parent, status, depth). The coordinator decides the role for each sub-agent: `researcher`, `analyst`, `verifier`, or `writer`.

- **Fan-out** — Sub-agents are spawned either as in-process tokio tasks (default) or as separate OS processes (when `use_multiprocess = true`). Each sub-agent gets its own `AgentRuntime` with a fresh prompt, the sub-task description, and a cancellation token shared with the parent.

- **Collection** — As sub-agents complete, their results are budget-capped by `ResultBudget`: each result is truncated to a fair share of the parent's available context window, and the full text is spilled to disk. The parent receives a summary + a spill path.

- **Reflection** — For lead-generation tasks, the coordinator runs a gap analysis: if the collected contacts fall short of the target, it spawns additional gap-filling sub-agents with refined queries.

- **Goal Mode** — The coordinator compares the accumulated results against the original goal using an LLM judge. If gaps remain, it spawns new sub-tasks targeting specific missing information. This repeats up to `replan_rounds` (default 3).

- **Synthesis** — All findings are merged into a structured report. The coordinator writes markdown files to the output directory (`index.md`, `summary.md`, `findings/`), absorbs contacts and findings into long-term memory, and triggers the export/notification pipeline.

- **Stall monitoring** — A background task monitors sub-agent progress via timestamps. If a sub-agent hasn't sent an event within `warn_secs` (configurable), a warning is logged. If it exceeds `kill_secs`, the sub-agent is cancelled and its slot is freed for re-execution.

### Multi-Agent Communication

Beyond the coordinator's fan-out pattern, the agent runtime supports several additional multi-agent primitives:

- **IrcBus** — A peer-to-peer message bus that allows agents to send and receive messages directly, without going through the coordinator. Each agent registers via `register_with_bus()` and drains its inbox at every turn boundary. Messages are addressed by `AgentId` and delivered asynchronously.
- **hub tool** — A tool that enables agents to send messages to arbitrary peers, inspect the roster of live agents, and coordinate work across the agent hierarchy. Supports `send`, `wait`, `list`, `inbox`, `jobs`, `cancel`, `start`, `stop`, `restart`, `ps`, `logs`, and `describe` operations.
- **Batch spawn** — The `tasks[]` parameter on `spawn_agent` allows spawning multiple sub-agents in a single tool call. Each sub-task can optionally specify an `output_schema` for structured result validation. The parent collects all results in a single turn.
- **Handoff** — A tool mechanism that transfers control of a conversation or task from one agent to another. The handing-off agent serializes its state and passes it to the recipient, who resumes from that point.
- **daemon tool** — A tool that spawns long-running background agents (`Daemon`). Daemons persist across sessions, restart on failure, and can be discovered via `DaemonRegistry` and the `hub` tool.

### PromptBuilder — 3-Tier Cache Architecture

Inspired by the Hermes cache-stable prompt architecture, `PromptBuilder` maximizes LLM provider cache hit rates by ordering prompt content from most stable to most volatile:

- **Tier 1 — Stable**: Identity, role description, general instructions, output format rules. These change rarely and are likely already cached by the provider.
- **Tier 2 — Context**: Environment info (cwd, platform, model name, date, timezone). Changes per session but is stable across turns within a session.
- **Tier 3 — Volatile**: Tools list (with schemas), loaded skills, memory digest, conversation history. Changes every turn.

When `build()` is called, tiers are concatenated with distinctive separator tokens that help the provider's cache system identify which sections have changed.

### Context Management

1. **Token counting** — accurate BPE (tiktoken cl100k_base), fallback on CJK-aware heuristic when the model isn't in the tiktoken registry
2. **Tool result truncation** — per-tool 50KB/2000 lines, per-turn 200KB, with persistence-to-disk so the full result is retrievable
3. **Micro-compaction** — dedup tool results by content hash, prune old outputs (no LLM call, purely algorithmic)
4. **Full compaction** — LLM summarization of the middle section (head + summary + tail preserved, the middle is summarized into a structured format: Goal/Done/Blocked/Next)
5. **Anti-thrashing** — hysteresis system: after an ineffective compaction (less than 5% reduction), further compactions are suppressed until the transcript grows by 1.2× or a minimum number of rounds pass. This prevents losing the provider's prompt cache by re-sending a compressed version every turn.

Trigger: `estimated_tokens >= context_window * compact_threshold` (default 50%).

Complemented by long-term memory ([MEMORY-KB.md](MEMORY-KB.md)): relevant facts are injected into the prompt as a digest, rather than being stored in the session context.

### Control Plane

The control plane (`control` module) provides two interaction channels between agents and the human operator:

- **Questions** — The `question` tool blocks the agent loop and emits a `QuestionAsked` event on the bus. The operator answers via `POST /api/v1/sessions/:id/answer` (HTTP) or the TUI input field. If no answer arrives within the timeout (configurable, default 5 min), the agent receives an "operator unavailable" notice and continues.

- **Approvals** — Tools listed in `[agent] approval_tools` are gated behind operator approval. The agent emits an `ApprovalRequested` event with a human-readable preview of the call arguments. The operator allows or denies via `POST /api/v1/sessions/:id/approve` or the TUI. Timeout behavior is controlled by `[agent] approval_fallback` (default: deny).

Both channels use `tokio::sync::oneshot` channels for reply, with the host (TUI/HTTP) holding the sender half. Dropping the receiver is treated as "operator went away."

### Lifecycle Hooks (PreToolUse / PostToolUse / Stop)

Hooks are subprocesses invoked with a JSON payload on stdin at defined points of the agent loop. They enable external policy enforcement without modifying the agent code:

- **PreToolUse** — Receives `{"tool", "args"}` → returns `{"decision": "allow"|"deny", "reason"?}`. A `deny` verdict refuses the call and feeds the reason back to the model. Multiple hooks can be registered; the first `deny` wins.

- **PostToolUse** — Receives `{"tool", "args", "result", "success"}` → returns `{"append_context"?}`. Extra context is appended to the tool result before it reaches the model.

- **Stop** — Receives `{"final_summary"}` → returns `{"continue": bool, "reason"?}`. When `continue` is true, the agent gets the reason as a follow-up instruction instead of stopping. Bounded by `MAX_STOP_CONTINUATIONS` (3) to prevent infinite loops.

Hooks are best-effort: a timeout, spawn failure, or unparseable verdict is treated as "allow / no-op" and only logged.

### Session Resumption

When the process crashes or is killed, sessions remain in the database with status `running`. `SessionResumer` finds interrupted sessions (no update for >5 minutes), reconstructs their state from the database, and returns a `ResumeState` that the coordinator uses to continue:

- Completed agent outputs are recovered from the DB
- Unfinished agent tasks are collected for re-execution
- Subtree token counts are computed so the budget is accurate
- The session is marked as `resumed` and the coordinator picks up from where it left off

---

## crates/tools

**57 built-in tools + 6 computer tools** (= 63 total), all implement the `Tool` trait:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> ToolSchema;
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolOutput>;
}
```

Categories (details in [TOOLS.md](TOOLS.md)):
- **Web**: web_search, web_fetch, web_crawl, web_feed
- **Files**: file_read, file_write, file_edit, glob, grep
- **Exec**: shell, python_exec, node_exec
- **Browser (CDP)**: browser_navigate, browser_screenshot, browser_click, browser_type, browser_extract
- **Computer use (Playwright)**: computer_snapshot, computer_navigate, computer_click, computer_type, computer_key, computer_screenshot — operate a real browser via the loopback computer service (see [COMPUTER-USE.md](COMPUTER-USE.md))
- **Vision**: analyze_image
- **Git**: git_status, git_diff, git_log, git_add, git_commit, git_push
- **PDF**: pdf_extract
- **OSINT**: extract_contacts, find_leads, search_business_directory, search_social, parse_corporate_site, search_news
- **Verification**: verify_email, verify_phone, verify_social_profile, suggest_emails
- **Enrichment**: enrich_company, enrich_person
- **Long-term memory**: memory_absorb, memory_search, memory_digest, memory_boost, memory_link, memory_graph
- **Control plane**: question
- **Multi-agent**: spawn_agent (including batch/handoff parameters), hub (including steer), daemon
- **Meta**: memory, skill, scratchpad, undo

Helper modules:
- `registry` — `ToolRegistry` (maps tool names to implementations), `ToolContext` (provides agent id, session id, working directory, config references)
- `search` — `SearchEngine` with 7 backends: Linkup, Exa, Tavily, Serper, Brave, Parallel, and DuckDuckGo
- `guard` — SSRF protection for all agent HTTP requests: validates every URL before fetch, blocks loopback/IPv6-private/RFC1918/link-local addresses, enforces max redirects (5), and re-validates each redirect hop to prevent DNS rebinding attacks
- `injection` — prompt-injection detection in web content: scans fetched pages for known injection patterns and strips/neutralizes them before the content reaches the model
- `truncate` — truncation with persistence-to-disk: spills overflow text to spill files for later retrieval
- `file_history` — undo/redo snapshots: every `file_write` and `file_edit` creates a snapshot; the `undo` tool restores the last snapshot
- `file_lock` — per-path locking: prevents concurrent file writes from colliding
- `autosave` — deterministic contact saving: after every tool call that produces contacts, the runtime auto-saves them to the contact database
- `extract` — contact extraction engine: regex-based + LLM-assisted extraction of emails, phones, social links from text

---

## crates/memory

Long-term semantic memory (mem0/Memora model, detailed in [MEMORY-KB.md](MEMORY-KB.md)):

| Module | Purpose |
|--------|---------|
| `db` | `MemoryDb` — SQLite: facts, FTS5 full-text search, embeddings (384d vectors), version edges, history |
| `absorb` | Write pipeline: secrets detection → consolidation → dedup → classification (5 outcomes: new, related, duplicate, contradicting, updated) |
| `search` | Hybrid search (cosine similarity on vectors + BM25 keyword), freshness decay, LLM-rerank, digest generation |
| `embed` | Embedders: OpenAI-compatible API (text-embedding-3-small) + offline TF-IDF fallback |
| `graph` | Entity graph person↔company: node dedup, multi-hop BFS traversal (up to 6 hops), relationship extraction |
| `distill` | Distillation of session run-facts into durable knowledge (extracts key entities, facts, relationships from a session transcript) |
| `secrets` | Detection of API keys/tokens/PEM on write — prevents sensitive data from being stored in memory |

**Key principles:**
- **Append-only versioning** — facts are never overwritten. Edges carry `supersedes` or `contradicts` relationships, creating a version history that can be inspected and rolled back.
- **Scopes** — `user` (global across sessions), `agent` (per-agent), `run` (per-session). Memory searches scope by default to the current session, then agent, then user.
- **Digest** — Before a depth-0 agent starts, the runtime generates a memory digest: a condensed summary of the most relevant facts for the current query, injected into the system prompt.
- **Auto-absorb** — The runtime automatically absorbs collected contacts and findings into long-term memory after each tool execution cycle.

---

## crates/mcp

Model Context Protocol — enables the agent to expose its tools to external MCP clients and to consume tools from external MCP servers.

- **Client**: stdio + Streamable HTTP transports, OAuth client-credentials flow, dynamic tool discovery (discovers tools from remote MCP servers at runtime), reconnect with exponential backoff
- **Server**: `fathom mcp-serve` — exposes all 63 tools externally via the MCP protocol. Each tool call is serialized and executed through the existing `ToolExecutor`, so MCP clients get the same behavior as in-process agents

The MCP bridge allows the agent to be embedded in IDEs (via the `lsp` crate), CI/CD pipelines, or custom frontends that speak the MCP protocol.

---

## crates/persistence

- **`Persistence`** — SQLite (WAL mode, pool of 4 round-robin connections) for sessions/agents/messages/findings/subtasks. WAL mode allows concurrent readers without blocking the writer, and the connection pool distributes load across connections so that streaming writes from multiple agents don't serialize on a single mutex. In-memory databases use a single connection (each `:memory:` connection is a different database).
- **`ContactDb`** — contact database (SQLite): stores extracted contacts with dedup, source URLs, verification status, and timestamps.
- **`PgContactDb`** — PostgreSQL backend (optional, behind `postgres` feature flag). Used in production deployments where shared access across multiple instances is needed.
- **`JobsDb`** — durable jobs with attempts and self-healing retry. Lives in its own SQLite database (`~/.fathom/jobs.db`). Each job records its task description, attempt count, last error, and status. Failed attempts are retried with an augmented task that carries the previous error, so the agent can diagnose and fix its own failure. Jobs survive process restarts: the runner (`fathom job-run <id>`) is spawned as a detached process with `setsid`, so it outlives the terminal that submitted it.
- **`SessionHistory`** — session history and search (CLI `sessions`): indexes all past sessions with full-text search on queries and summaries.

**Schema migrations** are idempotent: `add_column_if_missing()` checks for column existence before adding, so databases created by older versions upgrade gracefully.

---

## crates/server

Axum HTTP API (details in [HTTP-API.md](HTTP-API.md)):

| Endpoint | Purpose |
|----------|---------|
|| `POST /api/v1/sessions` | Create a research session |
| `GET /api/v1/sessions` | List all sessions |
| `GET /api/v1/sessions/:id` | Get session status |
| `GET /api/v1/sessions/:id/results` | Get session results |
| `DELETE /api/v1/sessions/:id` | Cancel a running session |
| `POST /api/v1/sessions/:id/steer` | Inject a mid-run instruction |
| `POST /api/v1/sessions/:id/answer` | Answer a pending `question` tool |
| `POST /api/v1/sessions/:id/approve` | Allow/deny a pending side-effect tool |
| `GET /api/v1/agents` | List all agents |
| `GET /api/v1/agents/:id` | Get agent status |
| `GET /api/v1/events` | SSE stream of all agent events |
| `GET /api/v1/sessions/:id/events` | SSE stream filtered to one session |
| `POST /api/v1/jobs` | Submit a durable background job |
| `GET /api/v1/jobs` | List all jobs |
| `GET /api/v1/jobs/:id` | Get job status |
| `GET /api/v1/jobs/:id/log` | Tail the job log |
| `DELETE /api/v1/jobs/:id` | Cancel an active job |
| `POST /api/v1/jobs/:id/rerun` | Re-run a finished/stale job |
| `GET /api/v1/computers/:agent_id/*` | Computer use relay (snapshot, navigate, click, type, key, screenshot, screen, files, control, ensure, stop, reset) |
| `GET /api/v1/credentials` | List credential metadata |
| `POST /api/v1/credentials` | Store a credential |
| `GET /api/v1/credentials/:id` | Retrieve a credential |
| `DELETE /api/v1/credentials/:id` | Delete a credential |
| `GET /api/v1/coworkers` | List coworkers |
| `POST /api/v1/coworkers` | Create a coworker |
| `GET /api/v1/coworkers/:id` | Get coworker details |
| `PUT /api/v1/coworkers/:id` | Update coworker |
| `DELETE /api/v1/coworkers/:id` | Delete coworker |
| `POST /api/v1/coworkers/:id/run` | Trigger a coworker run |
| `GET /api/v1/channels` | List channels |
| `POST /api/v1/channels` | Create a channel |
| `GET /api/v1/channels/:id` | Get channel details |
| `PUT /api/v1/channels/:id` | Update channel |
| `DELETE /api/v1/channels/:id` | Delete channel |
| `GET /api/v1/schedules` | List schedules |
| `POST /api/v1/schedules` | Create a schedule |
| `GET /api/v1/schedules/:id` | Get schedule details |
| `PUT /api/v1/schedules/:id` | Update schedule |
| `DELETE /api/v1/schedules/:id` | Delete schedule |
| `POST /api/v1/schedules/claim` | Atomic claim a schedule |
| `GET /governance/audit` | Stream audit records |
| `GET /governance/decide` | Real-time decision logs |
| `GET /api/v1/observability/summary` | Cluster-wide observability summary |
| `POST /api/v1/notifications/test` | Send a test notification |
| `GET /ag-ui/events` | AG-UI versioned event stream |
| `GET /ag-ui/health` | AG-UI health check |
| `GET /health` | Health check |
| `GET /metrics` | Prometheus metrics |
| `GET /dashboard` | Embedded single-file live dashboard |

**Key architectural features:**
- **SSE streaming** — All agent events are broadcast via `tokio::sync::broadcast` and exposed as Server-Sent Events. The `GET /api/v1/events` endpoint streams every event on the server. The `GET /api/v1/sessions/:id/events` endpoint filters to one session using a two-level cache: a positive cache (agents known to belong to this session) and a negative cache (agents proven to belong to other sessions), so a busy multi-session server doesn't re-query the DB for every foreign event.
- **Mid-run steering** — The `POST /api/v1/sessions/:id/steer` endpoint injects a user instruction into a running session. The text reaches all agents at the next turn boundary via a `tokio::sync::mpsc` channel.
- **Control plane** — The `POST /api/v1/sessions/:id/answer` and `POST /api/v1/sessions/:id/approve` endpoints resolve pending `question` and `approval` requests by sending the operator's response through the oneshot channel to the waiting agent.
- **Auth** — API key authentication via `X-API-Key` header, rate limiting (default 120 requests/minute per client, configurable via `FATHOM_RATE_LIMIT` env var), Prometheus metrics for request duration, total requests, and in-flight sessions.
- **Embedded dashboard** — A single-file HTML dashboard (`assets/dashboard.html`) is served at `GET /dashboard`. It provides a read-only live view of all sessions, agents, events, and memory state, consuming the same REST/SSE API that external clients use.
- **AG-UI stream** — `GET /ag-ui/events` exposes versioned AG-UI event envelopes with bounded reconnect replay via `Last-Event-ID`; `GET /ag-ui/health` is the liveness probe.
- **Computer relay** — `GET/POST /api/v1/computers/:agent_id/*` proxies the computer service (snapshot, navigate, click, type, key, screenshot, screen, files, control, ensure, stop, reset) and routes to the right Docker container per agent via the supervisor.
- **Governance** — `GET /governance/audit` and `GET /governance/decide` expose the immutable audit trail of authorization decisions; `/api/v1/credentials` manages the AES-256-GCM encrypted credentials vault (operator-only, plaintext never returned).
- **Coworkers / channels / schedules** — full lifecycle management of persistent autonomous workers: lifelong profiles (`/coworkers`), symbolic delivery channels (`/channels`), and cron-like timers with atomic claim (`/schedules`, `/schedules/claim`).
- **Observability** — `GET /api/v1/observability/summary` aggregates cluster-wide state; `POST /api/v1/notifications/test` exercises notification channels.

---

## crates/governance

Policy engine and audit trail — see [GOVERNANCE.md](GOVERNANCE.md) for the full reference.

| Module | Purpose |
|--------|---------|
| `policy` | `PolicyEngine` — loads allow/deny rules from `policy.toml`, evaluates `<tool, target>` pairs, fail-closed on no match |
| `audit` | `AuditTrail` — immutable append-only SQLite-backed decision log with secret redaction and queryable by tool/agent/verdict/date-range |
| `vault` | `CredentialsVault` — AES-256-GCM encrypted secret store, operator-only access, no secret-input tool in the agent registry |
| `relay` | `ServerRelay` — intercepts tool calls that need authentication, injects vault credentials, adds `x-fathom-operator` claim |

**Key architectural decisions:**
- **Deny wins** — if any matching rule denies, the call is blocked regardless of any allow rules that also match
- **Fail-closed** — unmatched tool+target pairs are denied by default
- **Redact on write** — secret-like values (API keys, tokens, PEM, base64) are detected by regex and replaced with `[REDACTED]` before audit persistence
- **No credential exposure** — the agent registry has no tool for reading or writing credentials; only the operator relay injects them

---

## crates/supervisor

Docker per-agent computer provisioning — see [COMPUTER-USE.md](COMPUTER-USE.md) for the full reference.

| Module | Purpose |
|--------|---------|
| `provision` | `Provisioner` — pulls the computer image, creates per-agent containers with persistent workspace volumes and browser profiles |
| `network` | `NetworkManager` — manages the Docker network, loopback port mapping, and restrictive capabilities |
| `health` | `HealthChecker` — liveness probe on the computer service port, timeout-based container recycling |
| `lifecycle` | `ContainerLifecycle` — create, start, stop, remove containers with RAII cleanup |

**Key architectural decisions:**
- **One container per agent** — each agent gets an isolated computer with its own workspace, browser profile, and network namespace
- **Loopback isolation** — containers are mapped to unique ports (`COMPUTER_BASE_PORT + agent_index`) so agents never share state
- **Restrictive by default** — no `--privileged`, no host networking, no read-write host mounts, limited syscalls via seccomp
- **Auto-cleanup** — containers are stopped and removed when the agent finishes or is cancelled (via cancellation token propagation)

---

## crates/tui

Ratatui terminal interface:

- **Multi-agent tree view** — Shows the agent hierarchy in a collapsible tree: coordinator at the root, sub-agents as children, with status indicators (running, completed, failed, waiting).
- **Streaming buffer** — Line-buffered, live LLM deltas: every `LlmStreamChunk` event is rendered as it arrives, so the user sees the model's response character by character.
- **Jobs panel** — Lists all durable background jobs with status, attempts, and progress.
- **Memory panel** — Three sub-panels: scopes (user/agent/run), entity graph (rendered as an ASCII tree), recent entries (timestamped list).
- **Operator control** — Input field at the bottom for answering `question` tool prompts, `y/n` prompts for approval requests, and general steering commands.
- **Event log** — Scrollable log of all `AgentEvent` types with timestamps and color-coded severity.
- **Thinking display** — When the model emits a `thinking` block, it's rendered in a distinct panel with a dimmed style so the user can see the reasoning process without it cluttering the main output.
- **Vim input modes** — Normal/insert mode for the input field, with vi keybindings.

---

## Data Flow

```
User Query
    │
    ▼
Coordinator ──plans──► [sub-task 1, sub-task 2, ...]
    │      (digest from memory — into depth-0 agent prompt)
    │
    ├──spawn──► AgentRuntime (researcher) ──tools──► web_search, web_fetch, extract_contacts
    ├──spawn──► AgentRuntime (researcher) ──tools──► parse_corporate_site, enrich_person
    ├──spawn──► AgentRuntime (analyst)    ──tools──► file_read, python_exec
    └──spawn──► AgentRuntime (computer)   ──tools──► computer_snapshot, computer_click, computer_type
    │
    │  ◄──── budget-capped summaries ────
    ▼
Synthesize ──► summary.md + findings/
    │
    ├──► Long-term memory (absorb contacts and results)
    │
    ├──► Governance audit (every tool call authorized + redacted
    │     decision record appended to the audit trail)
    ▼
Export (PDF/HTML/JSON) + Notify (webhook/email/Telegram) + CRM sync
```

**Computer use flow.** When a sub-task requires operating a real browser, the runtime routes the `computer_*` tools through the server relay to a supervisor-provisioned Docker container running the Playwright loopback service (`apps/computer`). The agent receives accessibility-tree snapshots with opaque refs, interacts via refs, and a human operator can take over at any time over `/control/ws` or watch the live stream over `/screen` (details in [COMPUTER-USE.md](COMPUTER-USE.md)).

**The event bus ties everything together.** Every component publishes and subscribes to `AgentEvent` via a `tokio::sync::broadcast` channel:

- `AgentRuntime` publishes `ToolCallStarted`, `ToolCallCompleted`, `LlmStreamChunk`, `AgentStateChanged`, `QuestionAsked`, `ApprovalRequested`
- `Coordinator` publishes `AgentSpawned`, `AgentCompleted`, `AgentFailed`, `SessionStarted`, `SessionCompleted`, `SessionFailed`
- `Persistence` subscribes to events and writes them to the database (agents, messages, findings)
- `Server` subscribes to events and forwards them to SSE clients
- `Tui` subscribes to events and updates the live display
- `ProcessManager` converts worker IPC events back into `AgentEvent` for the coordinator

---

## Multi-Process Architecture

By default, all agents run in a single process (tokio tasks). When `use_multiprocess = true`:

```
Coordinator (process 1)
    │ Unix socket IPC
    ├──spawn──► Worker (process 2)  ── agent_id_1
    ├──spawn──► Worker (process 3)  ── agent_id_2
    └──spawn──► Worker (process 4)  ── agent_id_3
```

- Each worker is a separate OS process (`fathom worker ...`)
- **IPC via Unix domain sockets** — JSON-line messages (framed by newlines). The `IpcMessage` enum covers all coordination messages: `StartTask`, `Cancel`, `Progress`, `ToolCall`, `ToolResult`, `Completed`, `Failed`, `LlmChunk`.
- **Socket naming** — Agent UUIDs are hashed into short 16-hex-char filenames to stay within the ~104-byte Unix domain socket path limit.
- **SQLite WAL mode** for concurrent access — multiple processes can read/write the same database without blocking.
- **Lifecycle guarantees:**
  - Workers are spawned with `kill_on_drop(true)` — dropping the `Child` handle kills the OS process.
  - Socket-wait timeout (30s) explicitly kills and reaps the worker on failure.
  - Socket reads are capped at 8 MiB per line — a runaway worker cannot balloon coordinator memory.
  - `shutdown_all` cancels, then explicitly kills and reaps every child that is still alive.
- **Stall monitoring** — The coordinator runs a background task that tracks per-agent event timestamps. If an agent hasn't sent an event within `warn_secs`, a warning is logged. If it exceeds `kill_secs`, the agent is cancelled.

**Benefits**: isolation (one agent crash doesn't bring down others), per-process resource limits (cgroups, ulimits), and the ability to distribute agents across machines in future versions.

---

## Error Handling

- **Retry** — LLM requests with exponential backoff (3 attempts, base delay 1s, jitter)
- **Doom loop detection** — 3 identical tool calls → stop the agent
- **Cascading errors** — shell failure cancels sibling tool calls in the same batch
- **Destructive command protection** — blocking `rm -rf /`, `mkfs`, fork bombs in the `shell` tool
- **Graceful degradation** — tool errors are returned to the model as tool results (the model can retry, adjust, or give up)

---

## Streaming

Streaming is first-class throughout the architecture:

1. **LLM streaming** — `LlmProvider::stream()` returns a `Stream<Item = LlmDelta>`. The `AgentRuntime` forwards each delta as an `LlmStreamChunk` event on the broadcast bus. Tool-call fragments are assembled incrementally from the stream.

2. **SSE streaming** — The HTTP server exposes two SSE endpoints (`/events` and `/sessions/:id/events`) that stream all events in real time. The `event_stream()` function creates a `Stream` from the broadcast receiver, optionally filtering by session.

3. **TUI streaming** — The Ratatui interface renders LLM deltas character by character in a line-buffered buffer, giving the user a live view of the model's response.

4. **IPC streaming** — In multi-process mode, `LlmChunk` IPC messages stream deltas from the worker to the coordinator, which re-publishes them as `AgentEvent` so the server/TUI see them the same way as in-process agents.

---

## How Components Connect

The system is wired together through three shared primitives:

1. **`tokio::sync::broadcast`** — The event bus. Every subsystem that needs observability subscribes to the broadcast channel. The channel is bounded (capacity 1024) and drops the oldest event if a slow subscriber can't keep up — this ensures a slow TUI or HTTP client can't block the agent loop.

2. **`Persistence` (Arc-shared)** — All stateful operations (sessions, agents, messages, findings, jobs, memory, coworkers, credentials, schedules) go through the persistence layer. The connection pool ensures concurrent access doesn't serialize.

3. **`CancellationToken` (tokio_util)** — Every agent and job receives a cancellation token. When the user cancels a session (via HTTP, TUI, or CLI), the token is triggered, and all in-flight operations (LLM calls, tool executions, agent loops) are cancelled at their next await point.

The `AppState` struct in the server crate holds Arc references to the persistence layer, the broadcast sender, the control-plane channels, the governance policy engine, and the supervisor provisioner. The TUI holds similar references, allowing both frontends to operate on the same running sessions interchangeably. The **Tauri v2 desktop app** (`apps/desktop`) and the **Next.js 16 web dashboard** (`apps/web`) are separate frontends that consume the HTTP/SSE surface: live screens and agent trees, human takeover, masked secret entry, policy editing, audit review, and computer lifecycle states.