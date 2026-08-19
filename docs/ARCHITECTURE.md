# Architecture

Parallel Research is a modular system of **9 crates** in a Cargo workspace. The core is built around an async agent loop on `tokio`.

---

## Crate Overview

```
parallel-research/
├── crates/
│   ├── core/          # Fundamental types and domain logic
│   ├── llm/           # LLM provider abstraction
│   ├── agent/         # Agent runtime, coordination, control plane
│   ├── tools/         # 44 tools (web, osint, memory, question...)
│   ├── memory/        # Long-term semantic memory + entity graph
│   ├── mcp/           # Model Context Protocol (client and server)
│   ├── persistence/   # Data storage (SQLite, connection pool, jobs)
│   ├── server/        # HTTP API
│   └── tui/           # Terminal interface
└── src/main.rs        # CLI entry point
```

### Crate Dependencies

```
core  ←──  llm  ←──  agent  ←──  server
  ↑          ↑          ↑           ↑
  └── tools ─┴── mcp ───┴── tui ────┘
       ↑          └── persistence ──┘
       └── memory ──┘ (depends on core + llm)
```

`core` depends on nothing (foundation). `agent` combines `llm`, `tools`, `memory`, `persistence`.

---

## crates/core

Fundamental types and domain logic. **Does not depend on other crates.**

| Module | Purpose |
|--------|---------|
| `ids` | `SessionId`, `AgentId`, `FindingId` (UUID v7) |
| `message` | OpenAI-compatible `Message`, `ToolCall` |
| `agent` | `AgentRole`, `AgentState`, `AgentStatus`, `AgentRecord` |
| `event` | `AgentEvent` — events for the bus (broadcast) |
| `finding` | `Finding`, `Source` — research results |
| `config` | `AppConfig` and all 10 config sections |
| `memory` | `MemoryStore` (MEMORY.md/USER.md), typed memories |
| `skill` | `Skill`, `SkillRegistry` (SKILL.md) |
| `export` | `Exporter` — PDF/HTML/JSON/DOCX |
| `notify` | `Notifier` — webhook/email/Telegram |
| `crm` | `CrmSync` — amoCRM/Bitrix24/HubSpot |
| `session` | `SessionOutput` — session result |
| `token` | Accurate token counting (tiktoken cl100k_base) + heuristic-fallback |
| `error` | `PrError`, `PrResult` |

---

## crates/llm

Abstraction of LLM providers.

- **`LlmProvider` trait** — `complete()` and `stream()` methods
- **`DeepSeekProvider`** — OpenAI-compatible API
  - Retry with exponential backoff (3 attempts)
  - Streaming fallback for large responses
  - Response size limits (50MB)
  - HTTP timeout (5 min)
- **`retry`** — generic `with_retry()` helper

---

## crates/agent

The heart of the system — the agent runtime.

| Module | Purpose |
|--------|---------|
| `runtime` | `AgentRuntime` — loop LLM → tools → repeat, streaming, approval/question |
| `coordinator` | `Coordinator` — planning, fan-out, Goal Mode, synthesis |
| `compaction` | Hermes-style context compression |
| `prompt` | `PromptBuilder` — 3 cache tiers, role prompts |
| `tool_executor` | Smart parallelism (read-only in parallel, write sequentially) |
| `budget` | Result budget capping |
| `control` | Control plane: question/approval request types to the operator |
| `ipc` | IPC protocol for multi-process |
| `process_manager` | `ProcessManager` — spawn/monitor workers |
| `doom_loop` | `DoomLoopDetector` — protection against infinite looping |
| `resume` | `SessionResumer` — session resumption |
| `hooks` | PreToolUse/PostToolUse/Stop subprocess hooks |

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

---

## crates/tools

**44 tools** (+5 browser), all implement the `Tool` trait:

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
- **Vision**: analyze_image
- **Git**: git_status, git_diff, git_log, git_add, git_commit, git_push
- **PDF**: pdf_extract
- **OSINT**: extract_contacts, find_leads, search_business_directory, search_social, parse_corporate_site, search_news
- **Verification**: verify_email, verify_phone, verify_social_profile, suggest_emails
- **Enrichment**: enrich_company, enrich_person
- **Long-term memory**: memory_absorb, memory_search, memory_digest, memory_boost, memory_link, memory_graph
- **Control plane**: question
- **Meta**: spawn_agent, memory, skill, scratchpad, undo

Helper modules:
- `registry` — `ToolRegistry`, `ToolContext`
- `search` — `SearchEngine` (7 backends)
- `guard` — SSRF protection for all agent HTTP requests
- `injection` — prompt-injection detection in web content
- `truncate` — truncation with persistence-to-disk
- `file_history` — undo/redo snapshots
- `file_lock` — per-path locking
- `autosave` — deterministic contact saving
- `extract` — contact extraction engine

---

## crates/memory

Long-term semantic memory (mem0/Memora model, detailed in [MEMORY-KB.md](MEMORY-KB.md)):

| Module | Purpose |
|--------|---------|
| `db` | `MemoryDb` — SQLite: facts, FTS5, embeddings, version edges, history |
| `absorb` | Write pipeline: secrets → consolidation → dedup → classification (5 outcomes) |
| `search` | Hybrid search (vectors + BM25), freshness decay, LLM-rerank, digest |
| `embed` | Embedders: OpenAI-compatible + offline TF-IDF fallback |
| `graph` | Entity graph person↔company: node dedup, multi-hop BFS |
| `distill` | Distillation of session run-facts into durable knowledge |
| `secrets` | Detection of API keys/tokens/PEM on write |

Key principles: append-only versioning (edges `supersedes`/`contradicts`,
nothing is overwritten), scopes `user/agent/run`, digest in prompt before start,
auto-absorb of collected contacts by the runtime.

---

## crates/mcp

Model Context Protocol:
- **Client**: stdio + Streamable HTTP transports, OAuth client-credentials, dynamic tool discovery, reconnect
- **Server**: `parallel-research mcp-serve` — exposes all tools externally and actually executes `tools/call`

---

## crates/persistence

- **`Persistence`** — SQLite (WAL mode, pool of 4 round-robin connections) for sessions/agents/messages/findings/subtasks
- **`ContactDb`** — contact database (SQLite)
- **`PgContactDb`** — PostgreSQL backend (optional)
- **`JobsDb`** — durable jobs with attempts and self-healing retry
- **`SessionHistory`** — session history and search (CLI `sessions`)

---

## crates/server

Axum HTTP API (details in [HTTP-API.md](HTTP-API.md)):
- REST endpoints for sessions, agents, jobs, **memory**
- Control plane: `POST /sessions/:id/answer`, `POST /sessions/:id/approve`
- SSE streaming of events, mid-run steering
- API key auth + rate limiting, Prometheus metrics, health checks

---

## crates/tui

Ratatui interface:
- Multi-agent tree view
- Streaming buffer (line-buffered, live LLM deltas)
- Jobs panel, Memory panel (scopes, graph, recent entries)
- Operator control: answer to `question` via input, `y/n` for approval
- Event log, thinking display, vim input modes

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
    └──spawn──► AgentRuntime (analyst)    ──tools──► file_read, python_exec
    │
    │  ◄──── budget-capped summaries ────
    ▼
Synthesize ──► summary.md + findings/
    │
    ├──► Long-term memory (absorb contacts and results)
    ▼
Export (PDF/HTML/JSON) + Notify (webhook/email/Telegram) + CRM sync
```

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

- Each worker is a separate OS process (`parallel-research worker ...`)
- IPC via Unix domain sockets (JSON-line messages)
- SQLite WAL mode for concurrent access
- Coordinator receives progress/tool-call/completion events

**Benefits**: isolation (one agent crash doesn't bring down others), per-process resource limits.

---

## Context Management

1. **Token counting** — accurate BPE (tiktoken cl100k_base), fallback on CJK-aware heuristic
2. **Tool result truncation** — per-tool 50KB/2000 lines, per-turn 200KB, persistence-to-disk
3. **Micro-compaction** — dedup tool results by hash, prune old outputs (without LLM)
4. **Full compaction** — LLM summarization of the middle section (head + summary + tail)
5. **Anti-thrashing** — cooldown after ineffective compressions

Trigger: `estimated_tokens >= context_window * compact_threshold` (default 50%).

Complemented by long-term memory ([MEMORY-KB.md](MEMORY-KB.md)): relevant
facts are injected into the prompt as a digest, rather than being stored in the session context.

---

## Error Handling

- **Retry** — LLM requests with exponential backoff
- **Doom loop detection** — 3 identical tool calls → stop
- **Cascading errors** — shell failure cancels sibling tool calls
- **Destructive command protection** — blocking `rm -rf /`, `mkfs`, fork bombs
- **Graceful degradation** — tool errors are returned to the model as tool results