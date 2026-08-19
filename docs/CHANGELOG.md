# Changelog

All notable changes to the Fathom project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.3.0] — 2026-08-19

### Added
- **IrcBus + AgentRegistry + Hub tool** — peer-to-peer messaging and agent discovery across the bus.
- **Agent lifecycle park/revive** — `AgentLifecycleManager` and `IrcReviver` for pausing and resuming agent execution.
- **Steering** — `SteerRegistry` with peer-steering via `hub steer:` directive.
- **Batch spawn** — `tasks[]` array support, `output_schema`, isolated execution, and handoff between agents.
- **AsyncJobManager** — in-process job tracking, delivery sinks, `hub jobs` for monitoring.
- **DaemonBroker** — `DaemonRegistry` and `daemon` tool with port readiness checks.
- **Auto-reply / side-channel** — LLM-generated auto-reply on `await_reply` for non-blocking agent interaction.
- **Handoff** — session transfer between agents via `IrcBus`.

### Fixed
- **AsyncJobManager cleanup** — `unregister_sink` and removal of duplicate deliveries.

---

## [0.1.0] — 2026-08-19

### Added

#### Core architecture
- **10-crate Rust workspace** (`pr-core`, `pr-llm`, `pr-agent`, `pr-tools`, `pr-mcp`, `pr-persistence`, `pr-memory`, `pr-server`, `pr-tui`, `pr-lsp`) with shared workspace versioning and dependency resolution.
- **CLI entry point** (`src/main.rs`) with command routing via `clap` — `run`, `tui`, `serve`, `mcp-serve`, `memory`, `contacts`, `jobs` subcommands.
- **TOML-based configuration system** (`pr-core`) covering LLM providers, agent parameters, search backends, memory, contacts, CRM, export, notifications, MCP, context management, and lifecycle hooks.
- **Dotted-key config value access** (`set_config_value` / `lookup_value`) for runtime overrides.
- **Multi-stage Dockerfile** for production builds with LTO and stripped binaries.

#### Agent system
- **Hierarchical sub-agents** — coordinator agent decomposes queries into sub-tasks and spawns researcher/analyst/verifier/writer agents via `spawn_agent` tool, forming a live tree of parallel agents.
- **JoinSet-based runtime** — every spawned agent runs as a tokio task; the coordinator awaits results concurrently.
- **Broadcast message bus** — agents emit typed events (`plans`, `findings`, `errors`, `completions`) that parents and siblings subscribe to.
- **5 agent roles**:
  - `coordinator` — plans research strategy, delegates, synthesizes output (can spawn children, fan-out).
  - `researcher` — executes searches, scrapes pages, runs browser automation (depth-limited spawning).
  - `analyst` — cross-references findings, identifies contradictions, enriches entities (depth-limited spawning).
  - `verifier` — fact-checks claims against live sources and memory, assigns confidence scores.
  - `writer` — produces final deliverable (PDF, HTML, JSON, DOCX, Markdown).
- **Goal Mode** — LLM judge evaluates completeness against a goal specification; runs gap-filling rounds (search → assess → re-focus) until the goal is satisfied or the round limit is reached.
- **Stall detection** — agents that produce no output for a configurable period (default 60s) are warned, then killed if unresponsive.
- **Mid-run steering** — inject instructions into a running session at the next turn boundary.
- **Agent IPC** — inter-process communication primitives for agent coordination.
- **Task tree** — hierarchical tracking of sub-tasks, parent-child relationships, and completion status.

#### Search backends
- **7 search backends** behind a unified `web_search` tool interface:
  - **Linkup** — production-grade search API with deep research and citations.
  - **Exa** — neural semantic search with content extraction and `findSimilar`.
  - **Tavily** — AI-optimized search with extraction and crawling.
  - **Serper** — Google Search API with images, news, places, shopping, scholar.
  - **Brave** — privacy-first search engine integration.
  - **Parallel.ai** — web search, content extraction, deep research (`Tasks`), entity discovery (`FindAll`), monitoring.
  - **DuckDuckGo** — lightweight fallback search without API keys.
- **Hybrid mode** — queries all backends simultaneously, merges and deduplicates results.
- **Smart mode** — analyzes query type (news, company, academic, technical) and selects the optimal backend.
- **Normalized result schema** — all backends return structured results with titles, snippets, URLs, and metadata.

#### Tool system
- **51 built-in tools** + **5 browser automation tools** (CDP-based), managed through a central `ToolRegistry` with typed schemas, validation, and automatic documentation generation.
- **Tool categories:**
  | Category | Tools |
  |---|---|
  | Web search | `web_search` (7 backends), `web_crawl`, `feed_parse`, `extract_links` |
  | Browser | `browser_navigate`, `browser_click`, `browser_extract`, `browser_screenshot`, `browser_pdf` (CDP) |
  | File system | `read_file`, `write_file`, `edit_file`, `list_dir`, `grep` |
  | Shell | `run_command`, `run_script` (sandboxed) |
  | Code analysis | `code_symbols`, `repo_map` (AST-based) |
  | OSINT | `email_verify`, `phone_verify`, `social_search`, `enrich_entity` |
  | Memory | `memory_absorb`, `memory_search`, `memory_digest`, `memory_boost`, `memory_link`, `memory_graph`, `memory_stats` |
  | Data | `read_csv`, `read_json`, `sql_query`, `kv_get`, `kv_set` |
  | Vision | `vision_analyze` (image understanding via LLM) |
  | Git | `git_log`, `git_diff`, `git_clone` |
  | PDF | `pdf_extract`, `pdf_search` |
  | REPL | `python_repl`, `node_repl` (interactive runtimes) |
  | Contacts | `save_contacts`, `get_contacts`, `push_crm` |
  | Agent control | `spawn_agent`, `send_message`, `set_goal` |
- **Tool dispatch overhead** ~0.75 µs per call.
- **Tool call args serde round-trip** ~752 ns.
- **Fetch cache and MxCache** for deduplication of external API calls.
- **File history and lock manager** for safe concurrent file access.
- **ReadTracker** — validates files are read before editing, detects stale reads.

#### OSINT and lead generation
- **Email extraction** — from web pages, social profiles, and public directories with format validation.
- **Phone number extraction** — international format parsing and verification.
- **Social profile discovery** — LinkedIn, GitHub, Twitter, Facebook, Telegram, and more.
- **Company info extraction** — name, domain, industry, HQ location, funding, technologies.
- **Role/position extraction** — job titles, department, seniority level.
- **Deduplicated contact database** — SQLite locally, PostgreSQL for production; fuzzy matching on name + domain.
- **CRM push** (`contacts push-crm`) — supports **amoCRM**, **Bitrix24**, and **HubSpot** with schema mapping.
- **Contact enrichment** — `enrich_entity` tool for augmenting extracted data.
- **Email and phone verification** tools.

#### Semantic memory
- **Hybrid search engine** combining:
  - **Vector similarity** (embeddings via configured LLM) — semantic search.
  - **BM25 text ranking** — keyword precision.
  - **Reciprocal rank fusion** — merged result set.
- **Absorb pipeline** (Memora-inspired):
  1. Validation (min/max fact size enforcement).
  2. Secret detection (regex scanning for API keys, passwords, tokens).
  3. Consolidation (N→1 merge of near-identical facts within a batch).
  4. Embedding (metadata-influenced vector generation).
  5. Dedup by content hash.
  6. Candidate search (cosine similarity, threshold 0.55).
  7. LLM-based classification: `duplicate`, `supersede`, `contradict`, `coexist`, `related`, `new`.
- **Append-only supersession chains** — nothing is ever overwritten; old facts are marked superseded with provenance edges.
- **Entity graph** — typed relationships (`works_at`, `leads`, `reports_to`, `invests_in`, `competes_with`, custom) between entities.
- **`memory_graph` tool** — subgraph visualizations of entity connections.
- **Memory classes** — durability classes for fact retention.
- **Digest** — pre-session summary of relevant facts into agent context.
- **Garbage collection** — periodic pruning of stale facts (configurable TTL, default 90 days), merging of duplicate entity records, confidence decay.
- **Memory operations:**
  - Absorb: 94–1020 µs/fact.
  - Hybrid search @ 1K facts: 1.6–2.3 ms.
  - Digest: ~4.8 ms.

#### Context management
- **CJK-aware token counting** via `tiktoken-rs` (cl100k_base encoding).
- **Tool output truncation** — intelligently keeps beginning and end, removes redundant whitespace, strips HTML tags; CJK-aware and word-boundary respecting.
- **Hermes-style compaction** — summarizes older conversation turns into a condensed representation, replacing full history in the context window.
- **Per-session token budgets** — configurable (default 128K tokens); "economy mode" when near exhaustion.
- **Stall detection** — configurable warn/kill thresholds (default 60s warn, 120s kill).

#### HTTP API and server
- **Axum-based HTTP server** with full REST API:
  - `POST /api/v1/sessions` — create research session.
  - `GET /api/v1/sessions` — list sessions.
  - `GET /api/v1/sessions/:id` — session status.
  - `GET /api/v1/sessions/:id/results` — session results.
  - `DELETE /api/v1/sessions/:id` — cancel session.
  - `POST /api/v1/sessions/:id/steer` — mid-run instruction injection.
  - `POST /api/v1/sessions/:id/answer` — answer pending `question` tool.
  - `POST /api/v1/sessions/:id/approve` — allow/deny pending side-effect tool.
  - `GET /api/v1/agents` — list all agents.
  - `GET /api/v1/agents/:id` — agent status.
  - `GET /api/v1/events` — SSE stream of all agent events.
  - `GET /api/v1/sessions/:id/events` — SSE stream filtered to a session.
  - `POST /api/v1/jobs` — submit durable background job.
  - `GET /api/v1/jobs` — list jobs.
  - `GET /api/v1/jobs/:id` — job status.
  - `GET /api/v1/jobs/:id/log` — tail job log.
  - `DELETE /api/v1/jobs/:id` — cancel job.
  - `POST /api/v1/jobs/:id/rerun` — re-run finished/stale job.
  - `GET /health` — health check.
  - `GET /metrics` — Prometheus metrics.
- **Authentication** — API key or JWT-based auth with role-based access control.
- **Rate limiting** — per-key and per-IP rate limiting with configurable windows (default 120 req/min).
- **Prometheus metrics** — tool call counts, latency histograms, agent spawn rates, memory hit rates, error counters.
- **SSE streaming** — real-time agent events with typed JSON payloads.
- **Mid-run steering** — agents receive injected instructions at next turn boundary.
- **Approval endpoints** — tools configurable as "approval-required"; server pauses, sends SSE request, waits for approve/reject.
- **Question/answer** — agents emit questions mid-run; REST endpoints for polling and answering.
- **Embedded dashboard** — Astro-based web UI at `/dashboard` with live session trees, agent status, token usage, search results.

#### Durable background jobs
- **SQLite-backed job persistence** — survives process restarts, server reboots, crashes.
- **Full lifecycle**: `queued → running → completed / failed / cancelled`.
- **Attack-count retries** with exponential backoff.
- **Job log** — captures every agent event, tool call, and output chunk; streamable via `jobs logs`.
- **CLI commands**: `jobs submit`, `jobs list`, `jobs status`, `jobs logs`, `jobs cancel`, `jobs rerun`.

#### TUI (Terminal User Interface)
- **Ratatui-based interactive TUI** with:
  - **Tree view** — full agent hierarchy with role, status, token count, elapsed time. Arrow key navigation, `+`/`-` expand/collapse.
  - **Token sparkline** — real-time chart of token consumption per agent.
  - **Live streaming** — select any agent node to see output stream, tool calls, LLM reasoning.
  - **Session replay** — saved sessions replayed in TUI, stepping through timeline.
  - **Color-coded roles** — coordinators (blue), researchers (green), analysts (yellow), verifiers (red), writers (cyan).
- **Detachable/reattachable** — suitable for long-running SSH sessions.
- **Pending question and approval dialogs** — interactive modal dialogs for operator round-trips.
- **Memory snapshot** — periodically refreshed view of long-term memory store.

#### MCP (Model Context Protocol)
- **MCP Client** — connect to external MCP servers over **stdio**, **HTTP**, or **OAuth2** transports:
  - Transport negotiation and lifecycle management.
  - Tool discovery and schema caching.
  - Automatic reconnection on transport failure.
  - OAuth token refresh for authenticated servers.
- **MCP Server** (`mcp-serve`) — exposes all 51 tools to external MCP clients (IDE agents, automation platforms, other AI systems).

#### LLM abstraction
- **Unified LLM interface** (`pr-llm`) — pluggable provider abstraction.
- **OpenAI-compatible API** — works with DeepSeek, OpenAI, and any OpenAI-compatible endpoint.
- **Retry logic** with configurable backoff.
- **Streaming support** — token-by-token streaming for real-time output.
- **Provider factory** — auto-selects provider based on configuration.

#### Persistence
- **SQLite** — local storage for sessions, jobs, contacts, memory, and configuration.
- **PostgreSQL** — production-grade backend for contacts and session history (via `deadpool-postgres` connection pool).
- **Contact database** — deduplicated contact storage with fuzzy matching.
- **Session history** — full session recording with agent events, tool calls, and outputs.

#### LSP integration
- **LSP client** (`pr-lsp`) — Language Server Protocol integration for IDE-based agent interaction.
- **Auto-detection and auto-installation** of language servers.
- **LLM-facing tool adapter** — wraps LSP tools for use by agents.

#### Export and notifications
- **Export formats**: PDF, HTML, JSON, DOCX, Markdown (`rust_xlsxwriter` for Excel, `pulldown-cmark` for Markdown).
- **SMTP notifications** — configurable email alerts for session completion/failure.
- **Lifecycle hooks** — configurable pre/post hooks with timeout (default 5000ms).
- **Session output** — structured output with metadata, citations, and executive summary.

#### Testing
- **204 test annotations** across 22 test files (3,735 assertions).
- **E2E tests**: `e2e_basic_research`, `e2e_multi_agent`, `e2e_real_tools`.
- **Integration tests**: `integration_export_notify`, `integration_history`.
- **Property-based testing** with `proptest`.
- **LLM live agent tests** — full integration tests against real LLM providers.

#### Documentation
- **Comprehensive docs directory**: `ARCHITECTURE.md`, `INSTALLATION.md`, `CONFIGURATION.md`, `USAGE.md`, `TOOLS.md`, `HTTP-API.md`, `OSINT-LEADGEN.md`, `MEMORY-KB.md`, `BENCHMARKS.md`, `DEVELOPMENT.md`.
- **Crate-level documentation**: `crates/agent/ARCHITECTURE.md`, `crates/llm/docs.md`, `crates/mcp/docs.md`, `crates/tools/TOOLS_DOCUMENTATION.md`.
- **Article series**: `docs/article/` — architecture, tool calling, benchmarks, parsing, case studies.

### Performance benchmarks
| Measurement | Result |
|---|---|
| Tool dispatch overhead (batched) | ~0.75 µs/call |
| ToolCall args serde round-trip | ~752 ns |
| I/O batch speedup (tokio tasks) | 3.06× vs sequential |
| CPU batch speedup (tokio tasks) | 3.78× vs sequential |
| Feed parsing throughput | 1,077,586 items/s |
| HTML selector throughput | 245–531k rows/s |
| `code_symbols` (240 files) | 7.2 ms |
| `repo_map` (240 files) | 34.2 ms |
| Memory absorb | 94–1020 µs/fact |
| Memory hybrid search @ 1K | 1.6–2.3 ms |
| Memory digest | ~4.8 ms |

---

## [0.2.0] — 2026-08-19

### Changed
- **Renamed project** from `parallel-research` to **Fathom** — new binary name, workspace root, and all internal references updated.
- **README overhaul** — added metrics dashboard (LOC, tests, tools, crates), performance benchmarks table, expanded feature descriptions, quick-start examples, and repository structure diagram.
- **Documentation enrichment** — all docs files expanded with detailed explanations, code examples, and architectural diagrams.

### Removed
- **Website** (`website/` directory) — Astro landing page removed from the main repository.
- **`pr-core-api`** — standalone core API crate removed.

---

[0.2.0]: https://github.com/yakushevhk/Fathom/releases/tag/v0.2.0
[0.1.0]: https://github.com/yakushevhk/Fathom/releases/tag/v0.1.0