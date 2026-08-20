<p align="center">
  <img src="assets/fathom.png" alt="Fathom" width="75%">
</p>

<p align="center">
  <strong>Fathom</strong>
  <strong>— universal autonomous AI worker. Research, outreach, code, computer use — any task, autonomously.</strong>
</p>

<p align="center">
  <a href="#"><img src="https://img.shields.io/badge/Rust-2021-DEA584?style=flat&colorA=222222&logo=rust&logoColor=white" alt="Rust"></a>
  <a href="#"><img src="https://img.shields.io/badge/LOC-72k+-blue?style=flat&colorA=222222" alt="LOC"></a>
  <a href="#"><img src="https://img.shields.io/badge/tests-1407%20passed-3FB950?style=flat&colorA=222222" alt="Tests"></a>
  <a href="#"><img src="https://img.shields.io/badge/tools-63-58A6FF?style=flat&colorA=222222" alt="Tools"></a>
  <a href="#"><img src="https://img.shields.io/badge/crates-12-3178C6?style=flat&colorA=222222" alt="Crates"></a>
  <a href="#"><img src="https://img.shields.io/badge/license-MIT-58A6FF?style=flat&colorA=222222" alt="License"></a>
</p>

---

**Fathom** is a universal autonomous AI worker written in Rust. It accepts a natural-language task, decomposes it into sub-tasks with hierarchical sub-agents, and executes them autonomously — **research, outreach, code development, computer use, data processing, scheduled operations, lead generation, or anything you can describe in plain language**.

> Sub-agents spawn sub-agents. Coordinators plan, researchers gather, analysts cross-reference, verifiers fact-check, writers produce output. All in one binary.

Fathom is your **virtual remote AI assistant and employee**: deploy it on a server, give it tasks via CLI, HTTP API, TUI, background jobs, or cron schedules — and it works autonomously, with a governed policy engine, an encrypted credentials vault, scheduled runs, notification channels, and a **real browser-based computer it can operate just like a human**. Everything is memory-backed, so knowledge accumulates between runs.

## How it works

1. **Submit** a natural-language task via CLI, TUI, HTTP API, background job, or scheduled cron.
2. **Plan** — a coordinator agent decomposes the task into sub-tasks and spawns hierarchical sub-agents (`spawn_agent` tool).
3. **Execute** — researchers, coders, and outreach agents fan out across search backends, browser automation (CDP), computer use (Playwright), shell execution, code analysis, and OSINT extraction tools — all in parallel.
4. **Verify** — analyst and verifier agents cross-reference findings, check contradictions, and confirm facts against live sources.
5. **Persist** — verified facts, contacts, and notes are absorbed into long-term semantic memory and databases.
6. **Deliver** — the writer produces a report, code, an exported contact list, or a notification — in PDF, HTML, JSON, DOCX, Markdown, or pushed straight to your CRM.

Unlike a single-shot LLM prompt, Fathom treats work as an **ongoing, parallel process**: it plans, branches out across search backends, browser sessions, computer use, and shell commands, cross-references results, and delivers — all while streaming progress to a TUI, an HTTP dashboard, or a background job log.

## Capabilities

| Area | What Fathom can do |
|------|--------------------|
| **Research** | 7 search backends, web scraping, browser automation, PDF extraction, OSINT, lead generation, company/person enrichment |
| **Outreach** | Email/phone verification, contact extraction, CRM push (amoCRM, Bitrix24, HubSpot), Telegram notifications, scheduled campaigns |
| **Code** | Git operations, AST code analysis, file editing, shell execution, Python/Node REPL, codebase mapping |
| **Computer use** | Playwright browser automation, screen snapshots, click/type/navigate, file workspace, human takeover, screen sharing |
| **Scheduling** | Cron-like autonomous runs, scheduled harvesting, atomic claim, retries with exponential backoff |
| **Governance** | Allow/deny policy engine, audit trail, AES-256-GCM credentials vault, operator-only secret entry |
| **Memory** | Long-term semantic memory, hybrid search (vectors + BM25), entity graph, knowledge accumulation across sessions |

## Metrics

| metric | value |
|---|---|
| **Rust LOC** (source) | **67,588+** |
| **Rust LOC** (tests) | **4,443+** |
| **Rust files** | **141+** |
| **Crates** | **12** |
| **Tools** | **63** (57 built-in + 6 computer) |
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
| ToolCall args serde round-trip | ~751 ns |
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

When you submit a task, Fathom doesn't just feed it to a single LLM call. A **coordinator agent** first analyzes the request, decomposes it into discrete sub-tasks, and spawns **worker agents** via the `spawn_agent` tool. Each worker can itself spawn child agents (depth-limited to prevent runaway recursion), forming a live tree of agents that work in parallel.

Under the hood, this is powered by a **`JoinSet`-based runtime**: every spawned agent runs as a tokio task, and the coordinator awaits their results concurrently. Communication happens over a **broadcast message bus** — agents emit typed events (plans, findings, errors, completions) that any parent or sibling can subscribe to. Background agents deliver results as structured notifications rather than blocking the parent.

The architecture is designed for **branching work**: one branch searches the web, another scrapes specific pages, a third edits code, a fourth operates a browser — all simultaneously. The coordinator merges results, detects conflicts, and hands off to analyst or verifier agents as needed.

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

### 04 · 63 tools — extensible tool registry

Fathom ships with **57 built-in tools** plus **6 computer-use tools**, all managed through a central **tool registry** with typed schemas, validation, and automatic documentation generation.

**Tool categories:**

| Category | Tools |
|---|---|
| **Web search** | `web_search`, `web_fetch`, `web_crawl`, `web_feed` |
| **Browser (CDP)** | `browser_navigate`, `browser_click`, `browser_type`, `browser_extract`, `browser_screenshot` |
| **Computer use** | `computer_snapshot`, `computer_navigate`, `computer_click`, `computer_type`, `computer_key`, `computer_screenshot` |
| **File system** | `file_read`, `file_write`, `file_edit`, `glob`, `grep` |
| **Shell** | `shell` (sandboxed) |
| **Code analysis** | `code_symbols`, `repo_map` (AST-based codebase understanding) |
| **OSINT** | `verify_email`, `suggest_emails`, `verify_phone`, `verify_social_profile`, `search_social`, `search_business_directory`, `find_leads`, `enrich_company`, `enrich_person`, `extract_contacts`, `parse_corporate_site`, `search_news` |
| **Memory** | `memory_absorb`, `memory_search`, `memory_digest`, `memory_boost`, `memory_link`, `memory_graph`, `memory` |
| **Data** | `parse_html`, `extract_json` |
| **Vision** | `analyze_image` (image understanding via LLM) |
| **Git** | `git_status`, `git_diff`, `git_log`, `git_add`, `git_commit`, `git_push` |
| **PDF** | `pdf_extract` |
| **REPL** | `python_exec`, `node_exec` (interactive runtimes) |
| **Contacts** | `save_contacts` |
| **Agent control** | `spawn_agent`, `question`, `skill`, `scratchpad`, `undo` |
| **Coordination** | `hub`, `daemon` |

Each tool declares its JSON schema, a natural-language description, and cost/rate-limit metadata. The LLM sees these schemas as tool definitions and can invoke any tool in the same turn. Tool dispatch overhead is ~0.75 µs per call — negligible even in complex chains.

### 05 · Computer use (Playwright) — a real browser the agent operates

Beyond headless CDP scraping, Fathom can **operate a full real browser** — a loopback Playwright service (`apps/computer`) that controls a persistent Chromium profile:

- **Accessibility-tree snapshots** with opaque refs — the agent "sees" the page like a screen-reader and interacts via refs, never brittle CSS.
- **Multiple tab-scoped snapshots**, stale-ref rejection, `navigate` / `click` / `type` / `key` / `screenshot`.
- **Screen streaming** (`/screen` WebSocket) and **human takeover** over `/control/ws` — a human can grab control at any time to unblock the agent.
- **Confined file workspace** — bounded, path-confined reads/writes.

**Browser egress is guarded**: localhost, private, link-local, multicast, and metadata targets are rejected by default. `COMPUTER_ALLOW_PRIVATE_HOSTS=true` is for local development only and never bypasses metadata/multicast denies.

With `crates/supervisor` and Docker, Fathom provisions **one isolated computer per agent** — persistent workspace/profile volumes, loopback ports, restrictive capabilities, health checks.

### 06 · Governance & safety — policy engine, audit trail, encrypted secrets

Fathom's agent runtime is **governed by an explicit policy engine** (`crates/governance`):

- **Allow/deny rules** on tool + target (e.g., allow `browser.*` on `example.com`, deny `browser.type` on `/admin/*`). Deny wins; an empty or unmatched policy **fails closed**.
- **Every tool call is authorized** before execution; redacted authorization events are persisted to an immutable audit trail (`/governance/audit`, `/governance/decide`).
- **Credentials vault** — secrets are stored AES-256-GCM encrypted via `/api/v1/credentials`; list responses never include plaintext, and there is **no secret-input tool** in the agent registry. Operators enter secrets through the UI; the relay adds an `x-fathom-operator` claim.
- Secret-like values are redacted before audit persistence.

### 07 · Coworkers & schedules — autonomous recurring workers

Fathom can run as **persistent remote employees**:

- **Coworkers** are durable profiles (persisted in SQLite) — each with its own identity, goals, and optional linked Fathom session / channel.
- **Channels** link coworkers to surfaces (CLI, HTTP, messaging) where they can be addressed.
- **Schedules** trigger coworker runs on cron-like timers (`/api/v1/schedules`), **atomically claimed** so concurrent runners never execute the same task twice.
- **Notifications** deliver results to configured symbolic channels (Telegram, email, webhooks).

### 08 · Long-term semantic memory — hybrid search, entity graph, absorb pipeline

Fathom's memory system persists knowledge across sessions, building a growing knowledge base from every run. It's powered by a **hybrid search engine** combining:

- **Vector similarity** (embeddings via the configured LLM) — for semantic search ("companies that use Rust")
- **BM25 text ranking** — for keyword precision ("CEO email Acme Corp")
- **Reciprocal rank fusion** — merging both rankings into a single result set

The **absorb pipeline** processes new facts before storage:

1. **Deduplication** — fuzzy matching against existing facts by content hash and semantic similarity
2. **Supersedes/contradicts chains** — if a new fact supersedes an old one (e.g., updated job title), the old fact is marked as superseded, preserving provenance
3. **Secret detection** — regex-based scanning for API keys, passwords, and tokens; flagged facts are stored with restricted access
4. **Entity extraction** — people, organizations, locations, and roles are extracted and linked

The **entity graph** stores typed relationships between entities: `works_at`, `leads`, `reports_to`, `invests_in`, `competes_with`, and custom relation types. The `memory_graph` tool returns subgraph visualizations showing connections between entities.

**Digest** runs before each session: the system summarizes the most relevant facts from memory into the agent's context window, giving it a running-start knowledge of the domain. **GC** periodically prunes stale facts (configurable TTL, default 90 days) and merges duplicate entity records.

Memory operations are fast: absorb takes 94–1020 µs per fact, hybrid search at 1K facts runs in 1.6–2.3 ms, and a full digest completes in ~4.8 ms.

### 09 · Durable background jobs — survive restarts, retry, full HTTP API

Long-running tasks don't need to block the terminal. Fathom's **durable job system** lets you submit, monitor, and manage asynchronous operations.

```
fathom jobs submit "Analyze market X"
fathom jobs list
fathom jobs status <id>
fathom jobs logs <id>
fathom jobs cancel <id>
fathom jobs rerun <id>
```

Jobs are **SQLite-backed** — they survive process restarts, server reboots, and crashes. The scheduler uses **attempt-count retries with exponential backoff**: if a job fails, it's retried up to the configured limit with increasing delays. From the second attempt on, the agent receives the original task **plus the previous failure** — so it can diagnose its partial workspace and fix its own mistake instead of blindly rerunning.

Each job tracks its full lifecycle: `queued → running → completed / failed / cancelled`. The job log captures every agent event, tool call, and output chunk, streamable via `jobs logs`.

### 10 · MCP (Model Context Protocol) — client and server

Fathom implements the **Model Context Protocol** (MCP), making it both a consumer and provider of MCP-compatible tools.

**MCP Client** — Fathom agents can connect to external MCP servers over **stdio**, **HTTP**, or **OAuth2** transports. This means any tool exposed by an external MCP server (a database query tool, a Slack notifier, a custom API wrapper) becomes available to Fathom agents as if it were a built-in tool. The MCP client handles:

- Transport negotiation and lifecycle management
- Tool discovery and schema caching
- Automatic reconnection on transport failure
- OAuth token refresh for authenticated servers

**MCP Server** (`mcp-serve`) — Fathom can expose its whole toolbox to external MCP clients. Run `fathom mcp-serve` and any MCP-compatible host (IDE agents, automation platforms, other AI systems) can connect and use it. This turns Fathom into an **agent backend** for other AI tools.

### 11 · HTTP API + dashboard — real-time streaming, steering, approvals, observability

Fathom's HTTP server is built on **Axum** with a full REST API, real-time event streaming, and operational controls.

**API features:**

- **Authentication** — API key or JWT-based auth with role-based access control; non-loopback binds require `FATHOM_API_KEYS`.
- **Rate limiting** — per-key and per-IP rate limiting with configurable windows (`FATHOM_RATE_LIMIT`).
- **Prometheus metrics** — `/metrics` endpoint exposing tool call counts, latency histograms, agent spawn rates, memory hit rates, and error counters.
- **SSE streaming** — `GET /sessions/:id/events` pushes agent events to connected clients in real-time: plans, tool calls, results, errors, completions. **AG-UI** (`/ag-ui/events`) provides versioned event envelopes with bounded reconnect replay via `Last-Event-ID`.
- **Mid-run steering** — `POST /sessions/:id/steer` lets you inject instructions into a running session.
- **Approval endpoints** — tools can be approval-required; the server pauses, emits an approval request via SSE, and waits for `POST /sessions/:id/approve` or `/reject` before proceeding.
- **Question/answer** — agents emit questions mid-run via the `ask_user` mechanism.
- **Coworkers / channels / schedules / credentials / replay / observability** — full lifecycle management of autonomous workers under `/api/v1/…`.
- **Computer relay** — `/api/v1/computers/:agent_id/*` proxies the computer service (snapshot, click, type, key, screen, files, control) and routes to the right Docker container per agent.

The bundled **Astro dashboard** (`/dashboard`), the **Tauri v2 desktop app** (`apps/desktop`), and the **Next.js 16 web panel** (`apps/web`) provide live screens, agent trees, human takeover, masked secret entry, policy editing, audit review, and computer lifecycle states.

### 12 · TUI (ratatui) — interactive tree view, live streaming, session replay

For terminal-first users, Fathom provides a full **TUI** built with **ratatui**.

**TUI features:**

- **Tree view** — the full agent hierarchy as an interactive tree: role, status, token count, elapsed time per node.
- **Token sparkline** — real-time token consumption per agent, so you spot runaway agents before they hit budgets.
- **Live streaming** — select any agent node to see its output stream in real-time.
- **Session replay** — saved sessions replayed in the TUI, stepping through the timeline at your own pace.
- **Color-coded roles** — coordinators blue, researchers green, analysts yellow, verifiers red, writers cyan.

### 13 · Context management — token budgets, stall detection, compaction, CJK-aware

LLM context windows are the bottleneck of any agent system. Fathom's context management subsystem ensures you get the most out of every token.

**CJK-aware token counting** — uses `tiktoken-rs` with configured encoding (cl100k_base by default).

**Tool output truncation** — large tool results are truncated intelligently (keep head/tail, strip redundant whitespace and HTML), CJK-aware and word-boundary respecting.

**Hermes-style compaction** — conversation history is condensed into LLM-generated summaries, replacing older turns while preserving semantic continuity.

**Per-session token budgets** — default 128K tokens; near exhaustion the system enters "economy mode" (fewer branches, shorter outputs, aggressive compaction).

**Stall detection** — agents silent for a configurable period are warned, then killed and reported to their parent.

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

# MCP server — expose all 63 tools to external MCP clients (IDE agents, other AI)
./target/release/fathom mcp-serve

# Semantic memory — search, inspect, and manage the knowledge base
./target/release/fathom memory search "CEO email at Acme"
./target/release/fathom memory stats

# Contacts — list, deduplicate, or push to CRM
./target/release/fathom contacts list
./target/release/fathom contacts push-crm

# Background jobs — submit, monitor, cancel, rerun durable research tasks
./target/release/fathom jobs submit "Analyze market X"

# Profiling — personas for different kinds of work (hunter, analyst, validator…)
./target/release/fathom profiles list
```

## OSINT / outreach example

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
| `coordinator` | Plans the strategy, decomposes the task into sub-tasks, delegates to workers, and synthesizes final output. The brain of the operation. | Yes (fan-out) |
| `researcher` | Executes searches across backends, scrapes pages, runs browser automation, and gathers raw data. | Yes (depth-limited) |
| `analyst` | Cross-references findings from multiple workers, identifies contradictions, enriches entities, and produces structured observations. | Yes (depth-limited) |
| `verifier` | Fact-checks claims against live sources and memory, flags unverifiable statements, and assigns confidence scores. | No |
| `writer` | Produces the final deliverable — PDF, HTML, JSON, DOCX, or Markdown — with citations, structured sections, and executive summary. | No |

## Repo structure

```
Fathom/
├── src/                       # CLI entry point (clap) — run | tui | serve | mcp-serve | memory | contacts | jobs | profiles | sessions | config
├── crates/                    # 12-crate Cargo workspace (strict DAG, no circular deps)
│   ├── core/                  # shared types: IDs, messages, events, findings, config, skills, export, notify, CRM
│   ├── llm/                   # LlmProvider trait, OpenAI-compatible providers, retry/backoff
│   ├── agent/                 # agent runtime, coordinator, tool executor, compaction, budgets, hooks, resume
│   ├── tools/                 # 63 tools, registry, SSRF guard, injection defense, computer client
│   ├── memory/                # long-term semantic memory — hybrid search, entity graph, absorb pipeline
│   ├── mcp/                   # MCP client + server (stdio / HTTP / OAuth2)
│   ├── persistence/           # SQLite + PostgreSQL — jobs, sessions, contacts, coworkers, credentials, schedules
│   ├── server/                # Axum HTTP API — auth, SSE, approvals, metrics, dashboard, AG-UI, computer relay
│   ├── tui/                   # ratatui terminal interface
│   ├── lsp/                   # Language Server Protocol integration
│   ├── governance/            # policy engine — allow/deny rules, audit decision records
│   └── supervisor/            # Docker per-agent computer provisioning
├── apps/
│   ├── computer/              # Playwright loopback computer service (isolated browser + workspace)
│   ├── desktop/               # Tauri v2 desktop app — live screen, human takeover, policy, audit
│   └── web/                   # Next.js 16 web dashboard — chat, agents, jobs, memory, events
├── docs/                      # full documentation set
├── website/                   # marketing/docs site (Astro)
└── Cargo.toml                 # workspace manifest
```

## Documentation

| Document | Description |
|----------|-------------|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Crate design, data flow, message bus — how the pieces fit together |
| [docs/INSTALLATION.md](docs/INSTALLATION.md) | Setup, build, Docker, systemd deployment |
| [docs/CONFIGURATION.md](docs/CONFIGURATION.md) | Full configuration reference — LLM, memory, tools, server, governance, computer |
| [docs/USAGE.md](docs/USAGE.md) | CLI commands and real-world examples |
| [docs/TOOLS.md](docs/TOOLS.md) | All tools reference with schemas and usage |
| [docs/HTTP-API.md](docs/HTTP-API.md) | HTTP API reference — auth, sessions, streaming, approvals, coworkers, computer |
| [docs/COMPUTER-USE.md](docs/COMPUTER-USE.md) | Computer use — Playwright service, snapshots, human takeover, Docker supervisor |
| [docs/GOVERNANCE.md](docs/GOVERNANCE.md) | Policy engine, audit trail, credentials vault — safety and compliance |
| [docs/OSINT-LEADGEN.md](docs/OSINT-LEADGEN.md) | OSINT and lead generation guide — extraction, verification, CRM push |
| [docs/COWORKERS.md](docs/COWORKERS.md) | Coworkers, channels, schedules — autonomous recurring workers |
| [docs/MEMORY-KB.md](docs/MEMORY-KB.md) | Semantic memory — absorb pipeline, hybrid search, entity graph |
| [docs/MCP-GUIDE.md](docs/MCP-GUIDE.md) | MCP client & server guide |
| [docs/BENCHMARKS.md](docs/BENCHMARKS.md) | Performance benchmarks and methodology |
| [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) | Development guide — workspace layout, testing, contributing |
| [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) | Common issues and fixes |

## License
MIT