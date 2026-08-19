<p align="center">
  <strong>Fathom</strong>
  <strong>— autonomous research agent for OSINT, lead generation, and deep-dive analysis.</strong>
</p>

<p align="center">
  <a href="#"><img src="https://img.shields.io/badge/Rust-2021-DEA584?style=flat&colorA=222222&logo=rust&logoColor=white" alt="Rust"></a>
  <a href="#"><img src="https://img.shields.io/badge/LOC-72k-blue?style=flat&colorA=222222" alt="LOC"></a>
  <a href="#"><img src="https://img.shields.io/badge/tests-1407%20passed-3FB950?style=flat&colorA=222222" alt="Tests"></a>
  <a href="#"><img src="https://img.shields.io/badge/tools-46+5%20browser-58A6FF?style=flat&colorA=222222" alt="Tools"></a>
  <a href="#"><img src="https://img.shields.io/badge/crates-10-3178C6?style=flat&colorA=222222" alt="Crates"></a>
  <a href="#"><img src="https://img.shields.io/badge/license-MIT-58A6FF?style=flat&colorA=222222" alt="License"></a>
</p>

---

**Fathom** is an autonomous research agent written in Rust. It accepts a natural-language query, decomposes it into sub-tasks with hierarchical sub-agents, and uses 46+ tools — across 7 search backends, OSINT extraction, browser automation, shell execution, and semantic memory — to gather, verify, and persist information.

> Sub-agents spawn sub-agents. Coordinators plan, researchers gather, analysts cross-reference, verifiers fact-check, writers produce output. All in one binary.

Unlike a single-shot LLM prompt, Fathom treats research as an **ongoing, parallel process**: it plans, branches out across search backends and browser sessions, cross-references findings, fact-checks them against memory and live sources, and finally writes a structured report — all while streaming progress to a TUI, HTTP dashboard, or background job log. Everything is memory-backed, so knowledge accumulates between runs.

## How it works

1. **Submit** a natural-language query via CLI, TUI, HTTP API, or background job.
2. **Plan** — a coordinator agent decomposes the query into sub-tasks and spawns hierarchical sub-agents.
3. **Gather** — researchers fan out across 7 search backends, browser automation, and OSINT extraction tools in parallel.
4. **Verify** — analyst and verifier agents cross-reference findings, check contradictions, and confirm facts.
5. **Persist** — verified facts are absorbed into long-term semantic memory and written to the contact database.
6. **Deliver** — the writer produces the final report in PDF, HTML, JSON, DOCX, or Markdown, optionally pushed to your CRM.

## Metrics

| metric | value |
|---|---|
| **Rust LOC** (source) | **67,588** |
| **Rust LOC** (tests) | **4,443** |
| **Total** | **72,031** |
| **Rust files** | **141** |
| **Crates** | **10** |
| **Tools** | **46 + 5 browser** (CDP) |
| **Search backends** | 7 (Linkup, Exa, Tavily, Serper, Brave, Parallel.ai, DuckDuckGo) |
| **Test annotations** | **204** (`#[test]` / `#[tokio::test]` / `#[proptest]`) |
| **Test files** | **22** |
| **Assertions** | **3,735** |
| **Agent roles** | 5 (coordinator, researcher, analyst, verifier, writer) |
| **CRM integrations** | 3 (amoCRM, Bitrix24, HubSpot) |
| **Export formats** | PDF, HTML, JSON, DOCX, Markdown |
| **LLM providers** | OpenAI-compatible (DeepSeek, any OpenAI API) |

## Benchmarks

| measurement | result |
|---|---|
| Tool dispatch overhead (batched) | ~0.75 µs/call |
| ToolCall args serde round-trip | ~752 ns |
| I/O batch speedup (tokio tasks) | **3.06×** vs sequential |
| CPU batch speedup (tokio tasks) | **3.78×** vs sequential |
| Feed parsing throughput | **1,077,586 items/s** |
| HTML selector throughput | 245–531k rows/s |
| `code_symbols` (240 files) | 7.2 ms |
| `repo_map` (240 files) | 34.2 ms |
| Memory absorb | 94–1020 µs/fact |
| Memory hybrid search @ 1K | 1.6–2.3 ms |
| Memory digest | ~4.8 ms | |

## Features

### 01 · Hierarchical sub-agents — tree of agents, broadcast bus, parallel execution

When you submit a query, Fathom doesn't just feed it to a single LLM call. A **coordinator agent** first analyzes the request, decomposes it into discrete sub-tasks, and spawns **researcher agents** via the `spawn_agent` tool. Each researcher can itself spawn child agents (depth-limited to prevent runaway recursion), forming a live tree of agents that work in parallel.

Under the hood, this is powered by a **`JoinSet`-based runtime**: every spawned agent runs as a tokio task, and the coordinator awaits their results concurrently. Communication happens over a **broadcast message bus** — agents emit typed events (plans, findings, errors, completions) that any parent or sibling can subscribe to. Background agents deliver results as structured notifications rather than blocking the parent.

The architecture is designed for **branching research**: one branch searches the web, another scrapes specific pages, a third queries a database — all simultaneously. The coordinator merges results, detects conflicts, and hands off to analyst or verifier agents as needed.

### 02 · 7 search backends, one unified interface

Fathom integrates **7 search backends** — **Linkup, Exa, Tavily, Serper, Brave, Parallel.ai, and DuckDuckGo** — all behind a single `web_search` tool with a consistent interface. You don't need to think about which backend to use; Fathom handles it.

Two operation modes give you control:

- **Hybrid mode** — queries all backends simultaneously and merges results, deduplicating and ranking by relevance. This maximizes coverage for broad research.
- **Smart mode** — analyzes the query type (news, company research, academic, technical) and selects the optimal backend. For example, Exa for neural semantic search, Tavily for real-time news, DuckDuckGo for lightweight fallback without API keys.

Each backend returns structured results with titles, snippets, URLs, and metadata. The tool normalizes these into a common schema, so agent code never touches backend-specific formats.

### 03 · OSINT / Lead generation — extract, deduplicate, enrich, push to CRM

Fathom is built for **open-source intelligence (OSINT)** and **lead generation** workflows. Given a target profile or company description, it extracts:

- **Email addresses** — from web pages, social profiles, and public directories, with format validation
- **Phone numbers** — international format parsing and verification
- **Social profiles** — LinkedIn, GitHub, Twitter, Facebook, Telegram, and more
- **Company info** — name, domain, industry, HQ location, funding, technologies used
- **Role/position** — job titles, department, seniority level

All extracted contacts are written to a **deduplicated contact database** (SQLite locally, PostgreSQL for production) via the `save_contacts` tool. The dedup pipeline uses fuzzy matching on name + domain to avoid duplicates across runs.

**CRM push** is a single command: `contacts push-crm`. Supported CRMs include **amoCRM, Bitrix24, and HubSpot** — contacts are mapped to the target CRM's schema and created as leads or contacts with associated metadata.

**Goal Mode** is the crown jewel of OSINT workflows. An LLM judge evaluates the completeness of gathered data against a goal specification (e.g., "find CEO email and LinkedIn at Acme Corp"). If the goal is unmet, the system runs **gap-filling rounds**: the judge identifies what's missing, and agents re-focus their search on the gaps. This continues until the goal is satisfied or the maximum round limit is reached. The result is a structured report showing what was found, what's still missing, and the confidence level for each field.

### 04 · 52 tools (+5 browser) — extensible tool registry

Fathom ships with **52 built-in tools** plus **5 browser automation tools** (CDP-based), all managed through a central **tool registry** with typed schemas, validation, and automatic documentation generation.

**Tool categories:**

| Category | Tools |
|---|---|
| **Web search** | `web_search`, `web_fetch`, `web_crawl`, `web_feed` |
| **Browser** | `browser_navigate`, `browser_click`, `browser_type`, `browser_extract`, `browser_screenshot` (CDP) |
| **File system** | `file_read`, `file_write`, `file_edit`, `glob`, `grep` |
| **Shell** | `shell` (sandboxed) |
| **Code analysis** | `code_symbols`, `repo_map` (AST-based codebase understanding) |
| **OSINT** | `verify_email`, `suggest_emails`, `verify_phone`, `verify_social_profile`, `search_social`, `search_business_directory`, `find_leads`, `enrich_company`, `enrich_person`, `extract_contacts`, `parse_corporate_site`, `search_news` |
| **Memory** | `memory_absorb`, `memory_search`, `memory_digest`, `memory_boost`, `memory_link`, `memory_graph`, `memory` (basic) |
| **Data** | `parse_html`, `extract_json` |
| **Vision** | `analyze_image` (image understanding via LLM) |
| **Git** | `git_status`, `git_diff`, `git_log`, `git_add`, `git_commit`, `git_push` |
| **PDF** | `pdf_extract` |
| **REPL** | `python_exec`, `node_exec` (interactive runtimes) |
| **Contacts** | `save_contacts` |
| **Agent control** | `spawn_agent`, `question`, `skill`, `scratchpad`, `undo` |
| **Coordination** | `hub`, `daemon` |

Each tool declares its JSON schema, a natural-language description, and cost/rate-limit metadata. The LLM sees these schemas as tool definitions and can invoke any tool in the same turn. Tool dispatch overhead is ~0.75 µs per call — negligible even in complex chains.

The browser tools control a **headless Chromium** instance via the Chrome DevTools Protocol (CDP), enabling JavaScript-rendered page interaction, screenshot capture, PDF generation, and form filling — crucial for sites that require client-side rendering.

### 05 · Long-term semantic memory — hybrid search, entity graph, absorb pipeline

Fathom's memory system persists knowledge across sessions, building a growing knowledge base from every research run. It's powered by a **hybrid search engine** combining:

- **Vector similarity** (embeddings via the configured LLM) — for semantic search ("companies that use Rust")
- **BM25 text ranking** — for keyword precision ("CEO email Acme Corp")
- **Reciprocal rank fusion** — merging both rankings into a single result set

The **absorb pipeline** processes new facts before storage:

1. **Deduplication** — fuzzy matching against existing facts by content hash and semantic similarity
2. **Supersedes/contradicts chains** — if a new fact supersedes an old one (e.g., updated job title), the old fact is marked as superseded, preserving provenance
3. **Secret detection** — regex-based scanning for API keys, passwords, and tokens; flagged facts are stored with restricted access
4. **Entity extraction** — people, organizations, locations, and roles are extracted and linked

The **entity graph** stores typed relationships between entities: `works_at`, `leads`, `reports_to`, `invests_in`, `competes_with`, and custom relation types. The `memory_graph` tool returns subgraph visualizations showing connections between entities.

**Digest** runs before each research session: the system summarizes the most relevant facts from memory into the agent's context window, giving it a running-start knowledge of the domain. **GC** periodically prunes stale facts (configurable TTL, default 90 days) and merges duplicate entity records.

Memory operations are fast: absorb takes 94–1020 µs per fact, hybrid search at 1K facts runs in 1.6–2.3 ms, and a full digest completes in ~4.8 ms.

### 06 · Durable background jobs — survive restarts, retry, full HTTP API

Long-running research tasks don't need to block the terminal. Fathom's **durable job system** lets you submit, monitor, and manage asynchronous research operations.

```
fathom jobs submit "Analyze market X"
fathom jobs list
fathom jobs status <id>
fathom jobs logs <id>
fathom jobs cancel <id>
fathom jobs rerun <id>
```

Jobs are **SQLite-backed** — they survive process restarts, server reboots, and crashes. The scheduler uses **attack-count retries** with exponential backoff: if a job fails, it's retried up to the configured limit with increasing delays between attempts.

Each job tracks its full lifecycle: `queued → running → completed / failed / cancelled`. The job log captures every agent event, tool call, and output chunk, streamable via `jobs logs`.

The same job system is exposed over the **HTTP API** (`POST /jobs`, `GET /jobs/:id`, `DELETE /jobs/:id`), enabling external tools (cron, CI/CD, dashboards) to schedule research programmatically.

### 07 · MCP (Model Context Protocol) — client and server

Fathom implements the **Model Context Protocol** (MCP), making it both a consumer and provider of MCP-compatible tools.

**MCP Client** — Fathom agents can connect to external MCP servers over **stdio**, **HTTP**, or **OAuth2** transports. This means any tool exposed by an external MCP server (a database query tool, a Slack notifier, a custom API wrapper) becomes available to Fathom agents as if it were a built-in tool. The MCP client handles:

- Transport negotiation and lifecycle management
- Tool discovery and schema caching
- Automatic reconnection on transport failure
- OAuth token refresh for authenticated servers

**MCP Server** (`mcp-serve`) — Fathom can expose its own 46-tool arsenal to external MCP clients. Run `fathom mcp-serve` to start an MCP server that any MCP-compatible host (IDE agents, automation platforms, other AI systems) can connect to and use. This turns Fathom into a **research backend** for other AI tools.

### 08 · HTTP API + dashboard — real-time streaming, mid-run steering, approvals

Fathom's HTTP server is built on **Axum** with a full REST API, real-time event streaming, and operational controls.

**API features:**

- **Authentication** — API key or JWT-based auth with role-based access control
- **Rate limiting** — per-key and per-IP rate limiting with configurable windows
- **Prometheus metrics** — `/metrics` endpoint exposing tool call counts, latency histograms, agent spawn rates, memory hit rates, and error counters
- **SSE streaming** — `GET /sessions/:id/stream` pushes agent events to connected clients in real-time: plans, tool calls, results, errors, completions. Each event is JSON with typed payloads.
- **Mid-run steering** — `POST /sessions/:id/steer` lets you inject instructions into a running session. The agent receives your steer as a high-priority message and adjusts its plan accordingly.
- **Approval endpoints** — tools can be configured as "approval-required." When an agent tries to use one, the server pauses, sends an approval request via SSE, and waits for `POST /approve/:id` or `POST /reject/:id` before proceeding.
- **Question/answer** — agents can emit questions to the user mid-run via the `ask_user` mechanism. The server exposes `GET /sessions/:id/questions` and `POST /sessions/:id/answer/:question_id`.

The **dashboard** is a bundled web UI (Astro-based) showing live session trees, agent status, token usage, and search results — accessible at `http://localhost:8080/dashboard`.

### 09 · TUI (ratatui) — interactive tree view, live streaming, session replay

For terminal-first users, Fathom provides a full **TUI** built with **ratatui** (the Rust terminal UI framework).

**TUI features:**

- **Tree view** — see the full agent hierarchy as an interactive tree. Each node shows the agent's role, status, token count, and elapsed time. Navigate with arrow keys, expand/collapse branches with `+`/`-`.
- **Token sparkline** — a real-time sparkline chart showing token consumption per agent, helping you spot runaway agents before they hit budgets.
- **Live streaming** — select any agent node to see its output stream in real-time: tool calls, results, LLM reasoning. The stream updates as events arrive.
- **Session replay** — saved sessions (from `jobs log` or `sessions archive`) can be replayed in the TUI, stepping through the timeline of events at your own pace.
- **Color-coded roles** — coordinators in blue, researchers in green, analysts in yellow, verifiers in red, writers in cyan. At a glance you know who's doing what.

The TUI runs alongside active research and can be detached and re-attached, making it suitable for long-running SSH sessions.

### 10 · Context management — token budgets, stall detection, compaction, CJK-aware

LLM context windows are the bottleneck of any agent system. Fathom's context management subsystem ensures you get the most out of every token.

**CJK-aware token counting** — uses `tiktoken-rs` with configured encoding (cl100k_base by default). Chinese, Japanese, and Korean characters are counted correctly, so mixed-language queries don't silently overflow.

**Tool output truncation** — when a tool returns a large result (e.g., a full web page), the system truncates it intelligently: it keeps the beginning and end (where important content typically lives), removes redundant whitespace, and strips HTML tags. Truncation is CJK-aware and respects word boundaries.

**Hermes-style compaction** — inspired by the Hermes project, Fathom can compact conversation history by summarizing older turns into a condensed representation. The LLM generates a compact summary of the conversation so far, which replaces the full history in the context window. This preserves semantic continuity while dramatically reducing token usage.

**Per-session token budgets** — each session has a configurable token budget (default 128K tokens). When the budget is near exhaustion, the system enters "economy mode": fewer parallel branches, shorter tool outputs, aggressive compaction. If the budget is exceeded, the coordinator is forced to wrap up.

**Stall detection** — agents that produce no output or tool calls for a configurable period (default 60s) are flagged. The system first warns the agent ("You haven't produced output in 60s. Please continue."). If the stall persists, the agent is killed and its parent notified. This prevents deadlocked agents from consuming resources indefinitely.

## Quick start

```bash
# Build from source
cargo build --release

# Run a research query — full pipeline: plan, gather, verify, persist, deliver
./target/release/fathom run "Your query" --output ./results/

# Interactive TUI — tree view of live agents, token sparklines, session replay
./target/release/fathom tui

# HTTP API server — REST endpoints, SSE streaming, Prometheus metrics, dashboard
./target/release/fathom serve --port 8080

# MCP server — expose all 46 tools to external MCP clients (IDE agents, other AI)
./target/release/fathom mcp-serve

# Semantic memory — search, inspect, and manage the knowledge base
./target/release/fathom memory search "CEO email at Acme"
./target/release/fathom memory stats

# Contacts — list, deduplicate, or push to CRM
./target/release/fathom contacts list
./target/release/fathom contacts push-crm

# Background jobs — submit, monitor, cancel, rerun durable research tasks
./target/release/fathom jobs submit "Analyze market X"
```

## OSINT example

```bash
./target/release/fathom run \
  "Find contacts of executives (CEO, CTO) at IT companies in Moscow. \
   Extract emails, phones, LinkedIn profiles." \
  --output ./leads/
```

This single command triggers the full pipeline: a coordinator decomposes the request into per-company research tasks, researchers query multiple search backends for each company's executive listing, OSINT tools extract emails, phones, and LinkedIn URLs from the discovered pages, email/phone verification tools validate the results, and the validated contacts are saved to the deduplicated contact database under `./leads/`. Run `contacts push-crm` afterwards to push everything to amoCRM, Bitrix24, or HubSpot — or use Goal Mode to let Fathom iterate until every requested field is filled in.

## Agent roles

| Role | Description | Can spawn children? |
|------|-------------|-------------------|
| `coordinator` | Plans the research strategy, decomposes the query into sub-tasks, delegates to researchers, and synthesizes final output. The brain of the operation. | Yes (fan-out) |
| `researcher` | Executes searches across backends, scrapes pages, runs browser automation, and gathers raw data. Each researcher owns a focused sub-task. | Yes (depth-limited) |
| `analyst` | Cross-references findings from multiple researchers, identifies contradictions, enriches entities with additional context, and produces structured observations. | Yes (depth-limited) |
| `verifier` | Fact-checks claims against live sources and memory, flags unverifiable statements, and assigns confidence scores to each finding. | No |
| `writer` | Produces the final deliverable — PDF, HTML, JSON, DOCX, or Markdown — with citations, structured sections, and executive summary. | No |

## Documentation

| Document | Description |
|----------|-------------|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Crate design, data flow, message bus — how the pieces fit together |
| [docs/INSTALLATION.md](docs/INSTALLATION.md) | Setup, build, Docker, systemd deployment |
| [docs/CONFIGURATION.md](docs/CONFIGURATION.md) | Full configuration reference — LLM, memory, tools, server, memory TTLs |
| [docs/USAGE.md](docs/USAGE.md) | CLI commands and real-world examples |
| [docs/TOOLS.md](docs/TOOLS.md) | All 46 tools reference with schemas and usage |
| [docs/HTTP-API.md](docs/HTTP-API.md) | HTTP API reference — auth, sessions, streaming, approvals |
| [docs/OSINT-LEADGEN.md](docs/OSINT-LEADGEN.md) | OSINT and lead generation guide — extraction, verification, CRM push |
| [docs/MEMORY-KB.md](docs/MEMORY-KB.md) | Semantic memory — absorb pipeline, hybrid search, entity graph |
| [docs/BENCHMARKS.md](docs/BENCHMARKS.md) | Performance benchmarks and methodology |
| [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) | Development guide — workspace layout, testing, contributing |

## Repo structure

```
Fathom/
├── src/                    # CLI entry point (4227 LOC) — parses commands, dispatches to crates
├── crates/                 # 10-crate workspace
│   ├── core/               # Types, config, memory, skills, export, CRM — foundational types and shared infrastructure
│   ├── llm/                # LLM abstraction, providers, retry, streaming — unified interface to DeepSeek, OpenAI, etc.
│   ├── agent/              # Agent loop, coordinator, goal mode, IPC — the brain: plan, spawn, monitor, steer
│   ├── tools/              # 46 tools (web, file, shell, browser, OSINT…) — tool registry, schema, dispatch
│   ├── memory/             # Semantic memory + entity graph — hybrid search, absorb pipeline, GC
│   ├── mcp/                # MCP client (stdio/HTTP/OAuth) + server — connect to or expose tools via Model Context Protocol
│   ├── persistence/        # SQLite, contact DB, session history — durable storage for jobs, contacts, sessions
│   ├── server/             # Axum HTTP API, dashboard, metrics — REST, SSE streaming, Prometheus
│   ├── tui/                # TUI (ratatui) — interactive tree view, live streaming, session replay
│   └── lsp/                # LSP client integration — language server protocol for IDE-based agent interaction
├── tests/                  # E2E + integration tests
├── docs/                   # Architecture, config, benchmarks, guides
├── website/                # Astro website (project landing)
├── Dockerfile              # Multi-stage Docker build
└── Cargo.toml              # Workspace manifest
```

## License

MIT