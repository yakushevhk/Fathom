# Fathom — Crate Documentation

> Complete technical documentation for each of the **12 crates** in the Fathom workspace. Each document describes **every function** with a full algorithm, SQL queries, edge cases, and interactions with other modules. This index page provides an architectural overview, the dependency graph, CLI entry points, and the design rationale behind the crate decomposition.

---

## Project Architecture

The project is a **Cargo workspace** (`resolver = "2"`) containing 12 crates, each with a well-defined responsibility. Tool inventory is conditional: 51 tools are always registered, with up to 5 CDP browser tools when CDP is reachable and up to 6 computer tools when `COMPUTER_URL` is configured; LSP tools are optional and separately registered. The crate boundaries are drawn to enforce dependency direction: `pr-core` sits at the bottom as a zero-dependency foundation, and everything flows upward through `pr-llm`, `pr-tools`, `pr-persistence`, and `pr-memory` into `pr-agent`, with `pr-server`, `pr-tui`, and `pr-lsp` providing the user-facing interfaces on top. Two standalone crates — `pr-governance` and `pr-supervisor` — provide policy enforcement and Docker lifecycle management respectively, consumed by `pr-agent` and `pr-server`.

```
fathom/
├── src/main.rs                # CLI entry point (run / worker / tui / serve / config / contacts / resume / memory / sessions / mcp-serve)
├── crates/
│   ├── core/      → docs/crates/core.md          # Foundation: types, config, export, notifications, CRM, skills
│   ├── llm/       → docs/crates/llm.md           # LLM abstraction, DeepSeek provider, retry, factory
│   ├── agent/     → docs/crates/agent.md         # Agent loop, Coordinator, compaction, IPC, process manager
│   ├── tools/      → docs/crates/tools.md         # Conditional registry: 51 always + up to 5 CDP + up to 6 computer tools
│   ├── mcp/       → docs/crates/mcp.md           # MCP client (stdio + HTTP), MCP server, bridge adapter
│   ├── persistence/ → docs/crates/persistence.md # SQLite/PostgreSQL, contacts, session history, jobs, audit events
│   ├── memory/    → docs/crates/../MEMORY-KB.md  # Long-term semantic memory: embeddings, entity graph, distillation, GC
│   ├── server/    → docs/crates/server.md        # HTTP API (axum), auth, rate limiting, SSE, Prometheus metrics
│   ├── tui/       → docs/crates/tui.md           # Terminal interface (ratatui), streaming, agent tree
│   ├── lsp/       → ../../crates/lsp/            # Language Server Protocol integration (IDE support; optional/separately registered)
│   ├── governance/ → docs/crates/governance.md   # Policy engine (allow/deny, fail-closed), audit decision records
│   └── supervisor/ → docs/crates/supervisor.md   # Docker per-agent computer provisioning, health checks, lifecycle
└── docs/
    ├── crates/    # ← you are here
    ├── ARCHITECTURE.md
    ├── OPENBOT_ARCHITECTURE.md
    ├── CONFIGURATION.md
    ├── TOOLS.md
    ├── USAGE.md
    ├── HTTP-API.md
    ├── INSTALLATION.md
    ├── DEVELOPMENT.md
    ├── BENCHMARKS.md
    ├── MEMORY-KB.md
    ├── MEMORY-SKILLS.md
    ├── OSINT-LEADGEN.md
    └── ...
```

---

## Crate Dependency Graph

The dependency graph forms a **directed acyclic graph** (DAG) with `pr-core` as the sole root. No crate depends on `pr-server`, `pr-tui`, or `pr-lsp` — they are terminal consumers. The `pr-agent` crate is the central hub that orchestrates LLM, tools, persistence, and memory.

```
                    ┌──────────┐
                    │ pr-core  │  ← foundation: types, config, events, errors, export, CRM, skills
                    └────┬─────┘
                         │
           ┌─────────────┼─────────────┬──────────────┐
           │             │             │              │
      ┌────▼────┐   ┌────▼────┐  ┌────▼──────────┐ ┌─▼──────────┐
      │ pr-llm  │   │ pr-tools│  │pr-persistence │ │ pr-memory  │
      └────┬────┘   └────┬────┘  └────┬──────────┘ └─────┬──────┘
           │             │             │                  │
           └──────┬──────┘             │                  │
                  │                    │                  │
             ┌────▼────────────────────▼──────────────────▼────┐
             │                    pr-agent                     │
             └────────────────────────┬───────────────────────┘
                                      │
        ┌──────────────┬──────────────┼──────────────┬──────────┐
        │              │              │              │          │
   ┌────▼────┐    ┌────▼────┐   ┌────▼────┐    ┌────▼────┐    │
   │pr-server│    │ pr-tui  │   │ pr-lsp  │    │ pr-mcp  │    │
   └─────────┘    └─────────┘   └─────────┘    └─────────┘    │
                                                               │
   ┌─────────┐                                      ┌─────────┘
   │ pr-mcp  │  ← depends on core + tools,           │
   └─────────┘    bridges MCP tools into agent        │
                  registry                            │
                                                      │
   ┌══════════════════════════════════════════════════┘
   ║  pr-memory also depends on core + llm (for
   ║  embeddings via the LLM provider)
   └──────────────────────────────────────────────────
```

**Key dependency relationships:**

| Crate | Depends on | Purpose of dependency |
|-------|-----------|----------------------|
| `pr-core` | *(none)* | Zero-dependency foundation — all other crates depend on it |
| `pr-llm` | `pr-core` | Uses `Message`, `ToolCall`, `PrError` types |
| `pr-tools` | `pr-core` | Uses `ToolSchema`, `ToolOutput`, `SearchConfig`, `ContactStore` |
| `pr-persistence` | `pr-core` | Uses `SessionId`, `AgentId`, `Finding`, `Contact`, `PrError` |
| `pr-memory` | `pr-core`, `pr-llm` | Uses core types + LLM provider for embeddings + classification |
| `pr-agent` | `pr-core`, `pr-llm`, `pr-tools`, `pr-persistence`, `pr-memory` | Central orchestrator — depends on everything below |
| `pr-mcp` | `pr-core`, `pr-tools` | Uses `ToolSchema`, `ToolRegistry`, `ToolContext` for bridge |
| `pr-server` | `pr-core`, `pr-llm`, `pr-agent`, `pr-persistence`, `pr-tools` | HTTP API that launches agents, queries persistence, streams events |
| `pr-tui` | `pr-core`, `pr-agent`, `pr-persistence` | Terminal UI that subscribes to events, queries history |
| `pr-lsp` | `pr-core`, `pr-agent` | IDE integration through the Language Server Protocol |

---

## Crate Documentation

Each document contains: full signatures of all functions, step-by-step algorithms, SQL queries, edge cases, cross-references, and design rationale. The documentation is generated from direct source code analysis and is kept in sync with the codebase.

| Crate | Document | What is described |
|---|---|---|
| **`pr-core`** | [core.md](core.md) | All 17 modules: `ids` (UUID v7 for SessionId, AgentId, FindingId), `message` (OpenAI-compatible Message enum with System/User/Assistant/Tool variants), `agent` (AgentRole, AgentState, AgentStatus, AgentRecord — 5 roles with spawn permissions), `event` (AgentEvent — 20+ variants for the broadcast bus), `finding` (Finding, Source — research results with confidence and metadata), `tool` (ToolSchema, ToolOutput — generic tool interface), `config` (AppConfig with 10+ sections, `set_config_value` algorithm), `error` (PrError with 12+ variants, PrResult), `token` (CJK-aware token counting via tiktoken cl100k_base with heuristic fallback), `memory` (MemoryStore with § separator, budget, USER.md/MEMORY.md), `skill` (Skill, SkillRegistry — SKILL.md discovery), `session` (SessionOutput — session result structure), `export` (Exporter — HTML/PDF/DOCX/CSV/vCard/XLSX with pandoc fallback), `notify` (Notifier — webhook/email/Telegram with SMTP relay), `contact` (Contact — normalization of email, phone, social links), `crm` (CrmSync — amoCRM/Bitrix24/HubSpot API integration), `profile` (Persona profiles — system prompt overrides, role-LLM assignment), `capability` (Capability detection — what the agent can do), `protected` (Protected content — PII redaction) |
| **`pr-llm`** | [llm.md](llm.md) | `LlmProvider` trait (complete/stream with Send+Sync), `DeepSeekProvider` (OpenAI-compatible: complete/stream algorithms, SSE parsing, streaming fallback for large responses >10MB, 3 retries with exponential backoff + jitter, 5-minute HTTP timeout, 50MB response limit), `CompletionRequest` and `CompletionResponse` types, `StreamChunk` enum (Text/ToolCallDelta/Done/Error), `Usage` tracking, `retry::with_retry()` generic helper, `factory::build_provider()` — config-based provider selection with `fast_model` support for auxiliary calls (extraction, classification, rerank), `concurrency` module for managing concurrent LLM requests, `types` module with error classification and response parsing |
| **`pr-agent`** | [agent.md](agent.md) | `AgentRuntime::run()` (14-step main loop: steering → background results → cancellation → compaction → LLM call → tool execution → doom loop detection → hooks), `Coordinator::execute()` (full lifecycle: plan → fan-out → collect → reflect → goal mode → synthesize → write), `CompactionEngine` (micro-compaction by dedup + LLM summarization split/summarize/reassemble with anti-thrashing cooldown), `PromptBuilder` (3-tier: stable cache tier / context tier / volatile tier, memory digest injection for depth-0), `ToolExecutor` (parallel-safe tools run concurrently, sequential tools ordered, read-before-write enforcement via ReadTracker, shell cascade cancellation), `DoomLoopDetector` (sliding window of 3 identical calls → nudge → stop), `TurnBudget` (per-turn byte cap with persistence-to-disk overflow), `CrashRecovery` (session resume after crash), `SessionResumer` (atomic claim via `UPDATE ... WHERE status='running'`), `BackgroundManager` (background child agents with result collection), `ProcessManager` (Unix socket IPC handshake, 6 message types, multi-process isolation), `ControlPlane` (question/approval request types, steering channel), `Hooks` (JSON-based subprocess hooks: PreToolUse/PostToolUse/Stop with verdicts Allow/Deny/Continue), `Reflection` (gap analysis for lead generation), `Improvement` (self-improvement loop), `TaskTree` (agent hierarchy tracking), `Prompts` directory (prompt templates for each role) |
| **`pr-tools`** | [tools.md](tools.md) | **Conditional tool registry**: 51 always-registered tools, up to 5 CDP browser tools when CDP is reachable, and up to 6 computer tools when `COMPUTER_URL` is configured; LSP tools are optional/separately registered. Organized into categories: **Web** (web_search with 7 backends: Linkup/Exa/Tavily/Serper/Brave/Parallel.ai/DuckDuckGo, hybrid sequential fallback, smart parallel RRF ranking; web_fetch with 2MB limit, SSRF guard, prompt injection scan against 12 patterns, manual redirect following with max 5 hops; web_crawl; web_feed), **Files** (file_read, file_write, file_edit with read-before-write gate via ReadTracker, glob, grep), **Shell** (shell with 8 security guard regexes blocking dangerous commands, timeout, cascade cancellation), **Browser** (browser_navigate/browser_screenshot/browser_click/browser_type/browser_extract via CDP WebSocket), **Vision** (analyze_image via external vision API), **Git** (git_status/git_diff/git_log/git_add/git_commit/git_push), **PDF** (pdf_extract), **REPL** (python_exec, node_exec with persistent kernels), **OSINT/LeadGen** (extract_contacts with 4-stage pipeline: regex → LLM → validation → dedup; find_leads; search_business_directory; search_social; parse_corporate_site; search_news), **Verification** (verify_email with MX check + SMTP handshake; verify_phone; verify_social_profile; suggest_emails), **Enrichment** (enrich_company; enrich_person), **Memory** (memory_absorb/memory_search/memory_digest/memory_boost/memory_link/memory_graph — long-term semantic memory tools), **Control** (question, spawn_agent, scratchpad, undo, load_skill), **Contacts** (save_contacts with atomic TOCTOU-safe dedup). Helper modules: `registry` (ToolRegistry, ToolContext with shared state), `search` (SearchEngine with 7 backends, hybrid/smart modes, RRF ranking), `guard` (SSRF protection — DNS resolution against blocklists, IP range checks), `injection` (12 prompt-injection patterns detection), `truncate` (per-tool and per-turn byte limits with persistence-to-disk), `file_history` (undo/redo snapshots), `file_lock` (per-path file locking), `autosave` (deterministic contact saving after extraction), `extract` (contact extraction engine), `cache` (TTL-based HTTP response cache, MX DNS cache), `coordination` (skill loading, scratchpad, undo) |
| **`pr-mcp`** | [mcp.md](mcp.md) | **MCP Client** (`McpClient`): stdio transport (child process spawn, stdin/stdout JSON-RPC, 60s read timeout, heartbeat monitoring) and Streamable HTTP transport (POST with SSE response parsing, OAuth 2.0 client-credentials grant with lazy token refresh 30s before expiry, session-id management via `mcp-session-id` header), initialize handshake, `list_tools` caching with dirty-invalidation on `notifications/tools/list_changed`, `call_tool` execution, auto-reconnect. **MCP Server** (`McpServer`): stdio loop exposing all agent tools via the shared `ToolRegistry`, JSON-RPC request/response dispatch, `tools/list` and `tools/call` methods. **Bridge** (`McpBridgeTool`): wraps a remote MCP tool as a local `Tool` trait implementation, seamlessly integrates into the agent's `ToolRegistry` alongside built-in tools. Sequential execution model (shared `Arc<Mutex<McpClient>>`). |
| **`pr-persistence`** | [persistence.md](persistence.md) | **SQLite** (`Persistence`): 6 tables (sessions, agents, messages, findings, tool_results, subtasks) with WAL mode, `synchronous=NORMAL`, `busy_timeout=5000`, foreign keys, idempotent migration via `PRAGMA table_info` + `ALTER TABLE ADD COLUMN`. **ContactDb** (SQLite): 5 tables (contacts, contact_emails, contact_phones, contact_socials, contact_sources) with `phone_norm` index for fast dedup, `save_deduped` algorithm (TOCTOU-safe atomic find-or-insert in a single transaction), `merge_contacts` (field-level resolution), `phone_norm` backfill. **PgContactDb** (PostgreSQL, optional `postgres` feature): connection pool via `deadpool_postgres`, `BIGSERIAL` vs `INTEGER`, `ILIKE` vs `LIKE`, identical schema. **ContactStore trait** (`save_contact`/`find_contact`/`search_contacts`/`merge_contacts` — unified async interface). **SessionHistory** (facade for listing/searching sessions, loading agents+findings, batch loading `IN (...)`). **JobsDb** (durable jobs with attempts, self-healing retry, status tracking). Exact SQL CREATE TABLE statements, PRAGMA settings, all CRUD with parameterized queries, edge cases for concurrent access. |
| **`pr-memory`** | [MEMORY-KB.md](../MEMORY-KB.md) | **Long-term semantic memory** (mem0/Memora model): `MemoryDb` — SQLite-backed store with facts, FTS5 full-text search, vector embeddings, version edges (`supersedes`/`contradicts`), append-only history. **Absorb pipeline**: secrets detection (API keys, tokens, PEM on write) → consolidation → dedup → LLM classification (5 outcomes: new_fact/update/contradiction/duplicate/irrelevant). **Hybrid search**: vectors (OpenAI-compatible or offline TF-IDF) + BM25 with configurable `semantic_weight`, freshness decay (`temporal_decay`), LLM-rerank as second pass, digest generation. **Entity graph**: person↔company relationships, node dedup, multi-hop BFS traversal. **Distillation**: session run-facts → durable knowledge. **Secrets detection**: regex-based PII/credential identification on write. **GC**: archiving expired/stale facts (`gc_ttl_days`), N→1 compaction of groups (`gc_compact_above`). Scopes: `user/agent/run`. Auto-digest injection into top-agent prompts. |
| **`pr-server`** | [server.md](server.md) | **30+ REST endpoints** on axum: `POST/GET /api/v1/sessions`, `GET/DELETE /api/v1/sessions/:id`, `POST /api/v1/sessions/:id/steer`, `GET /api/v1/sessions/:id/results`, `GET /api/v1/sessions/:id/events` (SSE), `GET /api/v1/events` (global SSE), `GET /api/v1/agents`, `GET /api/v1/agents/:id`, `GET /health`, `GET /metrics`. **Spawn lifecycle** with RAII cleanup (`SessionCleanup` Drop guard: atomic gauge decrement, active_sessions map removal, guaranteed on panic). **SSE streaming** with positive/negative cache filtering for session-scoped events. **Path traversal validation** (single-component check, no `..`, no absolute paths). **API key authentication** (Bearer + X-Api-Key headers, env `FATHOM_API_KEYS`, mandatory for non-loopback binding). **RateLimiter** (sliding window, default 120 req/min, env `FATHOM_RATE_LIMIT`). **Prometheus metrics** (Counter/Gauge/Histogram via atomic operations, no mutexes, `fetch_update` with AcqRel for f64 histogram sum). CORS: permissive with auth, restrictive without. |
| **`pr-tui`** | [tui.md](tui.md) | **Ratatui terminal interface**: `handle_key` (Normal/Insert/Paste input modes, vim-like navigation with `j/k/g/G`, tab switching, session cancellation with 2-second confirmation window), `handle_agent_event` (12+ event variants: AgentSpawned/Thinking/ToolCall/Completed/Failed, SessionCompleted/Failed, FindingDiscovered, StreamDelta, etc.), thinking auto-hide after 30s inactivity, `EventHandler` (250ms tick rate, broadcast→mpsc channel adapter), `StreamingBuffer` (character-wise push with `\n` line publishing), `UI layout` (header with sparkline of token usage, body with agent tree with depth indentation + output/thinking panel, footer with input bar), `App::new` spawning sessions inline, `--replay` mode for viewing saved sessions. |
| **`pr-lsp`** | *(in crate source; optional/separately registered)* | **Language Server Protocol** integration: `client.rs` — LSP client connection management, `detect.rs` — automatic detection of LSP-capable language servers, `install.rs` — server installation and configuration, `tool.rs` — LSP tool integration for code intelligence (go-to-definition, references, hover, completion). Enables IDE-like features within the agent's tool ecosystem. |

---

## CLI Entry Point (`src/main.rs`)

The binary provides a rich CLI with 10 subcommands, each mapped to a specific workflow:

| Subcommand | Description | Crate(s) involved |
|---|---|---|
| `run <query>` | Headless research mode. `--output` for directory, `--repeat N` for scheduled harvesting with diff-based alerting, `--profile` for persona presets | agent, tools, llm, persistence, memory |
| `worker` | Internal mode — launched by the coordinator as a separate OS process (IPC via Unix socket). **Do not run manually.** | agent (process_manager, ipc) |
| `tui` | Interactive terminal interface. Optional `[QUERY]` for initial query, `--profile`, `--replay <SESSION-ID>` for viewing past sessions | tui, agent, persistence |
| `serve` | HTTP API server. `--port`, `--host` (default 127.0.0.1:8080). Binds to 0.0.0.0 only if API keys are configured | server, agent, llm, tools, persistence |
| `mcp-serve` | MCP server — exposes the tools registered by the runtime to external MCP clients over stdio. The default registry is conditional (51 always, plus optional CDP/computer tools); LSP is separately registered. | mcp, tools |
| `contacts` | Contact management: `list`, `export --format` (csv/vcard/json), `push-crm` (sync to amoCRM/Bitrix24/HubSpot) | persistence, core (crm, export) |
| `memory` | Long-term semantic memory operations: `search` (hybrid), `list`, `get` (with version chain), `stats` (scopes, graph, DB size), `rebuild` (re-embedding), `distill` (session → knowledge), `gc` (archival + compaction), `nuke` (scope deletion) | memory, llm |
| `sessions` | Session history: `list` (with search by query substring), `show <id-or-prefix>` (agents + findings) | persistence |
| `resume` | Resume an interrupted session. Atomic claim via `UPDATE ... WHERE status='running'` | agent (resume), persistence |
| `config` | Configuration management: `show`, `set <key> <value>` (writes to `~/.fathom/config.toml`) | core (config) |
| `profiles` | Persona management: `list`, `show <name>`, `new <name>` (creates template in `~/.fathom/profiles/`) | core (profile) |

### Bootstrap Flow (`run`)

```
1. AppConfig::load() — load configuration from ~/.fathom/config.toml
   (or $PR_CONFIG override). All sections optional — missing fields get defaults.
2. Validate api_key — warn if missing (sessions will fail at LLM call)
3. SessionId::new() — UUID v7 (millisecond-precision timestamp for sortability)
4. build_registry() — ToolRegistry::with_builtins() + MCP server connections
   (from [[mcp.servers]] config array)
5. Persistence::open() — SQLite with WAL mode, connection pool, schema init
6. MemoryDb::open() — long-term semantic memory (best-effort, non-fatal)
7. ContactStore + CrmSync (best-effort — non-fatal if unavailable)
8. SkillRegistry::discover() — scan ~/.fathom/skills/ for SKILL.md
9. Coordinator::new() → coordinator.execute()
   ├── Plan (LLM decomposes query into sub-tasks; stored in DB)
   ├── Fan-out (spawn sub-agents in parallel via JoinSet or ProcessManager)
   ├── Collect (budget-capped summaries)
   ├── Reflection (lead-gen gap analysis)
   ├── Goal Mode (up to replan_rounds rounds of gap-filling)
   ├── Synthesize (LLM merges findings)
   └── Write output (index.md, summary.md, findings/, sources.md)
10. finalize_session() — export (PDF/HTML/JSON/DOCX) + notifications
    (webhook/email/Telegram) + CRM sync + memory absorption
```

---

## Agent Roles

The system defines 5 agent roles, each with specific spawn permissions, tool access, and behavioral characteristics. Role assignment determines which tools are available (via `deny_tools` per-role configuration) and which LLM model is used (via `role_models` configuration).

| Role | Can spawn? | Max depth | Typical tools | Purpose |
|---|---|---|---|---|
| `coordinator` | Yes (fan-out) | 0 (top-level) | spawn_agent, memory, scratchpad | Manages the session: decomposes the query into sub-tasks, launches sub-agents, collects results, synthesizes the final answer. Runs the Goal Mode loop. |
| `researcher` | Yes | 1+ | web_search, web_fetch, extract_contacts, save_contacts, search_business_directory, search_social, parse_corporate_site, search_news, find_leads | Performs information gathering and extraction. Can spawn sub-agents for parallel exploration of different sources. |
| `analyst` | Yes | 1+ | file_read, grep, analyze_image, python_exec, node_exec, pdf_extract | Analyzes and cross-references collected data. Can spawn sub-agents for deeper analysis of specific findings. |
| `verifier` | No | 0 (leaf) | web_search, web_fetch, verify_email, verify_phone, verify_social_profile, suggest_emails | Validates findings: checks email deliverability, phone number validity, social profile existence. Cannot spawn children — pure verification leaf. |
| `writer` | No | 0 (leaf) | file_write, file_read, scratchpad | Produces the final report. Cannot spawn children — single-purpose output generation. |

**Role configuration** (`[agent]` section in config.toml):
- `deny_tools` — per-role tool deny-lists: `researcher = ["shell", "git_push"]` prevents shell access for web researchers
- `role_models` — per-role model overrides: `analyst = "deepseek-reasoner"` assigns a reasoning model for analysis tasks
- `approval_tools` — tools requiring operator approval (default: `["save_contacts", "git_push"]`), configurable per role

---

## Key Architectural Decisions

| Decision | Description | Rationale |
|---|---|---|
| **12 crates** | Clear separation of responsibilities into 12 workspace crates | Enforces dependency direction, enables independent compilation and testing, allows crate-level feature flags (e.g., `postgres` feature in `pr-persistence`) |
| **Hierarchical agents** | Agents can spawn children (up to `max_depth` levels, default 2) | Enables structured decomposition: coordinator → researchers → verifiers. Each level narrows focus. `max_agents` (default 20) caps total agents per session. |
| **Two-phase tool execution** | Tools with `parallel_safe: true` run concurrently; `parallel_safe: false` run sequentially | Maximizes throughput for safe operations (web_search, file_read) while preventing race conditions on stateful operations (file_write, git_push). Shell cascade cancellation: if one shell tool fails, sibling shell tools are cancelled. |
| **Doom loop detection** | Sliding window of 3 identical (tool_name, argument_hash) calls → nudge → stop | Prevents the LLM from getting stuck repeating the same tool call. Two-stage response: first nudge (gentle correction), then hard stop (force agent termination). |
| **Cooperative cancellation** | `CancellationToken` hierarchy: session → agent → child | Clean shutdown without orphaned tasks. A cancelled agent stops at the next turn boundary, not mid-tool-execution. Children inherit `child_token()` from parent. |
| **Context compaction** | Two-stage: micro-compaction (dedup by hash, prune old outputs, no LLM) + LLM summarization (split middle section / summarize / reassemble) | Keeps context within the token budget without losing information. Anti-thrashing cooldown prevents repeated ineffective compactions. Triggered at `compact_threshold` (default 50% of `context_window`). |
| **3-tier prompts** | Stable tier (cache-friendly instructions) / Context tier (session env, date, git status) / Volatile tier (tool schemas, memory digest, skills) | Maximizes LLM cache hits (stable tier changes rarely), while keeping dynamic information fresh. Memory digest injected only for depth-0 agents. |
| **TOCTOU-safe dedup** | Atomic find-or-insert in a single SQLite transaction | Prevents duplicate contacts when multiple agents discover the same person simultaneously. Uses `INSERT ... WHERE NOT EXISTS` or `SELECT ... FOR UPDATE` pattern. |
| **WAL mode** | SQLite in Write-Ahead Logging with `synchronous=NORMAL`, `busy_timeout=5000` | Allows concurrent reads during writes — essential for multi-agent scenarios where the coordinator reads while sub-agents write. `busy_timeout` prevents SQLITE_BUSY errors. |
| **Lock-free metrics** | Atomic operations (AtomicU64, AtomicF64 via bit representation) without mutexes | Zero-contention metrics collection for the HTTP API. `fetch_update` with `AcqRel` ordering for f64 histogram sum. |
| **Multi-process isolation** | Optional (`use_multiprocess = true`): each agent runs as a separate OS process, IPC via Unix domain sockets (JSON-line messages, 6 message types) | Process-level isolation: one agent crash doesn't bring down others. Per-process resource limits. Coordinator receives progress/tool-call/completion events via socket. Enabled only when isolation is critical. |
| **Streaming fallback** | If a non-streaming response exceeds 10MB (STREAMING_THRESHOLD_BYTES), automatically switches to streaming | Prevents OOM on large responses. The streaming fallback reassembles the full response from SSE chunks. Tool calls are lost during fallback (text-only responses). |
| **Fast model** | `fast_model` config field for auxiliary LLM calls (extraction, classification, rerank) | Reduces cost and latency for non-critical LLM operations. The main model handles reasoning and tool selection; the fast model handles utility tasks. |
| **Append-only memory** | Version edges (`supersedes`/`contradicts`), nothing is overwritten | Full audit trail of knowledge evolution. Scopes (`user/agent/run`) isolate facts by source. GC archives old versions after `gc_ttl_days` (default 30). |
| **SSE filtering with caching** | Positive cache (known agent IDs) + negative cache (known non-membership) for session-scoped event streams | Avoids O(n) DB queries per event. The cache is populated on first lookup and invalidated when new agents are spawned. |
| **RAII session cleanup** | `SessionCleanup` Drop guard: atomic gauge decrement + active_sessions map removal | Guarantees cleanup even on task panic. No dangling sessions in the active sessions map. |
| **Path traversal protection** | Output directory validation: single component, no `..`, no absolute paths, no `/` | Prevents path traversal attacks via the HTTP API. Output is always within the configured base directory. |
| **CORS gating** | Permissive CORS only when API keys are configured; restrictive without auth | Without auth (local dev), CORS is blocked to prevent malicious web pages from controlling agents. With auth, CORS is permissive for API clients. |