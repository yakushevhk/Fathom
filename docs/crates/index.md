# Parallel Research — Crate Documentation

> Complete technical documentation for each of the 8 crates of the Parallel Research system. Each document describes **every function** with a full algorithm, SQL queries, edge cases, and interactions with other modules.

---

## Project Architecture

```
parallel-research/
├── src/main.rs                # CLI entry point (run / worker / tui / serve / config / contacts / resume)
├── crates/
│   ├── core/      → docs/crates/core.md          # Base types, configuration, export
│   ├── llm/       → docs/crates/llm.md           # LLM abstraction, DeepSeek, retry
│   ├── agent/     → docs/crates/agent.md          # Agent loop, Coordinator, compaction, IPC
│   ├── tools/     → docs/crates/tools.md          # 39+ tools
│   ├── mcp/       → docs/crates/mcp.md            # MCP client + server + bridge
│   ├── persistence/ → docs/crates/persistence.md  # SQLite/PostgreSQL, contacts, history
│   ├── server/    → docs/crates/server.md         # HTTP API, auth, metrics
│   └── tui/       → docs/crates/tui.md            # Terminal interface
└── docs/
    ├── crates/    # ← you are here
    ├── ARCHITECTURE.md
    ├── CONFIGURATION.md
    ├── TOOLS.md
    └── ...
```

---

## Crate Dependency Graph

```
                    ┌──────────┐
                    │ pr-core  │  ← foundation: types, config, events
                    └────┬─────┘
                         │
           ┌─────────────┼─────────────┐
           │             │             │
      ┌────▼────┐   ┌────▼────┐  ┌────▼──────────┐
      │ pr-llm  │   │ pr-tools│  │pr-persistence │
      └────┬────┘   └────┬────┘  └────┬──────────┘
           │             │             │
           └──────┬──────┘             │
                  │                    │
             ┌────▼────┐               │
             │ pr-agent│◄──────────────┘
             └────┬────┘
                  │
        ┌─────────┼─────────┐
        │                   │
   ┌────▼────┐         ┌────▼────┐
   │pr-server│         │ pr-tui  │
   └─────────┘         └─────────┘

   ┌─────────┐
   │ pr-mcp  │  ← depends on core + tools
   └─────────┘
```

---

## Crate Documentation

Each document contains: full signatures of all functions, step-by-step algorithms, SQL queries, edge cases, cross-references.

| Crate | Document | What is described |
|---|---|---|
| `pr-core` | [core.md](core.md) | All 17 modules: ids, message, agent, event, finding, tool, config (set_config_value algorithm), error, token (CJK-aware), memory (§ separator, budget), skill, session, export (HTML/PDF/DOCX/CSV/vCard/XLSX), notify (webhook/email/Telegram), contact (normalization), crm (amoCRM/Bitrix24/HubSpot API) |
| `pr-llm` | [llm.md](llm.md) | LlmProvider trait, DeepSeek complete/stream algorithms, SSE parsing, streaming fallback, retry with exponential backoff + jitter, error classification, factory |
| `pr-agent` | [agent.md](agent.md) | AgentRuntime::run() (14-step loop), Coordinator::execute() (full lifecycle), CompactionEngine (micro + LLM), PromptBuilder (3-tier), hooks (JSON protocol), IPC (6 messages), ProcessManager (socket handshake), BackgroundManager, TurnBudget, DoomLoopDetector (sliding window), CrashRecovery, SessionResumer, ToolExecutor (partition + path-overlap) |
| `pr-tools` | [tools.md](tools.md) | 39+ tools: web_search (7 backends, hybrid/smart, RRF ranking), web_fetch (SSRF-guard, prompt injection scan), file tools (read-before-write, locking), shell (8 guard regex), browser (CDP WebSocket), OSINT (4-stage pipeline), contacts (atomic dedup), injection detection (12 patterns) |
| `pr-mcp` | [mcp.md](mcp.md) | MCP client (stdio spawn, HTTP + OAuth), initialize handshake, list_tools caching, call_tool, SSE parsing, auto-reconnect, MCP server (stdio loop), bridge (McpBridgeTool adapter) |
| `pr-persistence` | [persistence.md](persistence.md) | Exact SQL CREATE TABLE (11 tables), PRAGMA settings, all CRUD with SQL, save_deduped algorithm (TOCTOU-safe), merge_contacts, phone_norm backfill, batch loading IN (...), PostgreSQL differences (BIGSERIAL/ILIKE/deadpool), ContactStore trait, SessionHistory |
| `pr-server` | [server.md](server.md) | 12 REST endpoints, spawn_session with RAII cleanup, SSE filtering (positive/negative cache), path traversal validation, API key auth (Bearer + X-Api-Key), RateLimiter sliding window, Counter/Gauge/Histogram atomic metrics, Prometheus format |
| `pr-tui` | [tui.md](tui.md) | handle_key (Normal/Insert/Paste), handle_agent_event (12 variants), thinking auto-hide (30s), EventHandler (100ms poll + broadcast→mpsc), StreamingBuffer (character-wise push + \n publish), UI layout (header/body/footer), agent tree with depth indentation |

---

## CLI Entry Point (`src/main.rs`)

| Subcommand | Description |
|---|---|
| `run <query>` | Headless research mode. `--output` for directory, `--repeat N` for scheduled harvesting |
| `worker` | Internal mode — launched by the coordinator as a separate OS process (IPC via Unix socket) |
| `tui` | Interactive terminal interface |
| `serve` | HTTP API server. `--port`, `--host` |
| `contacts` | Contact management: `list`, `export --format`, `push-crm` |
| `resume` | Resume an interrupted session |
| `config` | Configuration management: `show`, `set <key> <value>` |

### Bootstrap Flow (`run`)

```
1. AppConfig::load() — load configuration
2. Validate api_key
3. SessionId::new() — UUID v7
4. build_registry() — ToolRegistry + MCP servers
5. Persistence::open() — SQLite
6. ContactStore + CrmSync (best-effort)
7. Coordinator::new() → coordinator.execute()
8. finalize_session() — export + notifications
```

---

## Agent Roles

| Role | Can spawn? | Typical tools |
|---|---|---|
| `coordinator` | Yes (fan-out) | spawn_agent, memory |
| `researcher` | Yes | web_search, web_fetch, extract_contacts, save_contacts |
| `analyst` | Yes | file_read, grep, analyze_image |
| `verifier` | Yes | web_search, web_fetch, verify_email, verify_phone |
| `writer` | Yes | file_write, file_read |

---

## Key Architectural Decisions

| Decision | Description |
|---|---|
| **8 crates** | Clear separation of responsibilities between layers |
| **Hierarchical agents** | Agents can spawn children (up to max_depth levels) |
| **Two-phase tool execution** | Parallel-safe → concurrent, Sequential → ordered |
| **Doom loop detection** | Sliding window + two-stage strategy (nudge → stop) |
| **Cooperative cancellation** | CancellationToken hierarchy: session → agent → child |
| **Context compaction** | Micro-compaction (dedup) + LLM summarization (split/summarize/reassemble) |
| **3-tier prompts** | Stable (cache) / Context (session) / volatile (turn) |
| **TOCTOU-safe dedup** | Atomic find-or-insert in a single transaction |
| **WAL mode** | SQLite in Write-Ahead Logging for concurrent reads |
| **Lock-free metrics** | Atomic operations without mutexes |