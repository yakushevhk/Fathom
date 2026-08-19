<p align="center">
  <strong>Fathom</strong>
  <strong>— autonomous research agent for OSINT, lead generation, and deep-dive analysis.</strong>
</p>

<p align="center">
  <a href="#"><img src="https://img.shields.io/badge/Rust-2021-DEA584?style=flat&colorA=222222&logo=rust&logoColor=white" alt="Rust"></a>
  <a href="#"><img src="https://img.shields.io/badge/LOC-72k-blue?style=flat&colorA=222222" alt="LOC"></a>
  <a href="#"><img src="https://img.shields.io/badge/tests-92%20suites-3FB950?style=flat&colorA=222222" alt="Tests"></a>
  <a href="#"><img src="https://img.shields.io/badge/tools-46+5%20browser-58A6FF?style=flat&colorA=222222" alt="Tools"></a>
  <a href="#"><img src="https://img.shields.io/badge/crates-10-3178C6?style=flat&colorA=222222" alt="Crates"></a>
  <a href="#"><img src="https://img.shields.io/badge/license-MIT-58A6FF?style=flat&colorA=222222" alt="License"></a>
</p>

---

**Fathom** is an autonomous research agent written in Rust. It accepts a natural-language query, decomposes it into sub-tasks with hierarchical sub-agents, and uses 46+ tools — across 7 search backends, OSINT extraction, browser automation, shell execution, and semantic memory — to gather, verify, and persist information.

> Sub-agents spawn sub-agents. Coordinators plan, researchers gather, analysts cross-reference, verifiers fact-check, writers produce output. All in one binary.

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
| **Test annotations** | **92** (`#[test]` / `#[tokio::test]` / `#[proptest]`) |
| **Test files** | **22** |
| **Assertions** | **3,531** |
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
| Memory digest | ~4.8 ms |

## Features

### 01 · Hierarchical sub-agents

A coordinator decomposes the query, then spawns researchers via `spawn_agent`. Any agent can spawn children (depth-limited). Children run in parallel via `JoinSet`; background agents deliver results as notifications. Architecture: tree + broadcast bus.

### 02 · 7 search backends, one interface

Linkup, Exa, Tavily, Serper, Brave, Parallel.ai, DuckDuckGo — all behind a unified `web_search` tool. Hybrid mode mixes results; smart mode picks the best backend per query.

### 03 · OSINT / Lead generation

Extract emails, phones, social profiles, company info from the web. `save_contacts` writes to a deduplicated contact database (SQLite/PostgreSQL). Push to CRM (amoCRM, Bitrix24, HubSpot) with one command. Goal Mode: LLM judge evaluates completeness, runs gap-filling rounds.

### 04 · 46 tools (+5 browser)

File ops, shell, web search, browser (CDP), vision, git, PDF, REPL, memory (absorb/search/digest/boost/link/graph), crawling, feed parsing, code analysis (`code_symbols`, `repo_map`), social search, email/phone verification, entity enrichment, and more.

### 05 · Long-term semantic memory

Hybrid search (vectors + BM25) over absorbed facts. Entity graph with typed edges (`works_at`, `leads`, …). Absorb pipeline with dedup, supersedes/contradicts chains, secret detection. Digest before each run. GC for stale facts.

### 06 · Durable background jobs

`parallel-research jobs submit/list/status/logs/cancel/rerun`. SQLite-backed, survive restarts, attack-count retries. Full HTTP API for external scheduling.

### 07 · MCP (Model Context Protocol)

Client: stdio + HTTP + OAuth — external MCP server tools available to agents. Server: `mcp-serve` exposes agent's 46 tools to external MCP clients.

### 08 · HTTP API + dashboard

Axum server with auth, rate limiting, Prometheus metrics, SSE streaming of agent events, mid-run steering (`POST /sessions/:id/steer`), approval endpoints, question/answer.

### 09 · TUI (ratatui)

Interactive tree view of agents with cursor navigation, collapse/expand, token sparkline, live streaming of agent output, replay saved sessions.

### 10 · Context management

CJK-aware token counting (tiktoken-rs), tool output truncation, Heremes-style compaction, per-session token budgets, stall detection (warn/kill).

## Quick start

```bash
# Build
cargo build --release

# Run a research query
./target/release/parallel-research run "Your query" --output ./results/

# Interactive TUI
./target/release/parallel-research tui

# HTTP API server
./target/release/parallel-research serve --port 8080

# MCP server — expose tools to external MCP clients
./target/release/parallel-research mcp-serve

# Semantic memory
./target/release/parallel-research memory search "CEO email at Acme"
./target/release/parallel-research memory stats

# Contacts
./target/release/parallel-research contacts list
./target/release/parallel-research contacts push-crm

# Background jobs
./target/release/parallel-research jobs submit "Analyze market X"
```

## OSINT example

```bash
./target/release/parallel-research run \
  "Find contacts of executives (CEO, CTO) at IT companies in Moscow. \
   Extract emails, phones, LinkedIn profiles." \
  --output ./leads/
```

## Agent roles

| Role | Description | Can spawn children? |
|------|-------------|-------------------|
| `coordinator` | Plans, decomposes, synthesizes | Yes (fan-out) |
| `researcher` | Searches and gathers | Yes (depth-limited) |
| `analyst` | Cross-references findings | Yes (depth-limited) |
| `verifier` | Fact-checks | No |
| `writer` | Produces output | No |

## Documentation

| Document | Description |
|----------|-------------|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Crate design, data flow, message bus |
| [docs/INSTALLATION.md](docs/INSTALLATION.md) | Setup, build, Docker, systemd |
| [docs/CONFIGURATION.md](docs/CONFIGURATION.md) | Full config reference |
| [docs/USAGE.md](docs/USAGE.md) | CLI commands and examples |
| [docs/TOOLS.md](docs/TOOLS.md) | All 46 tools reference |
| [docs/HTTP-API.md](docs/HTTP-API.md) | HTTP API reference |
| [docs/OSINT-LEADGEN.md](docs/OSINT-LEADGEN.md) | OSINT and lead generation guide |
| [docs/MEMORY-KB.md](docs/MEMORY-KB.md) | Semantic memory: absorb, search, graph |
| [docs/BENCHMARKS.md](docs/BENCHMARKS.md) | Performance benchmarks |
| [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) | Development guide |

## Repo structure

```
Fathom/
├── src/                    # CLI entry point (4227 LOC)
├── crates/                 # 10-crate workspace
│   ├── core/               # Types, config, memory, skills, export, CRM
│   ├── llm/                # LLM abstraction, providers, retry, streaming
│   ├── agent/              # Agent loop, coordinator, goal mode, IPC
│   ├── tools/              # 46 tools (web, file, shell, browser, OSINT…)
│   ├── memory/             # Semantic memory + entity graph
│   ├── mcp/                # MCP client (stdio/HTTP/OAuth) + server
│   ├── persistence/        # SQLite, contact DB, session history
│   ├── server/             # Axum HTTP API, dashboard, metrics
│   ├── tui/                # TUI (ratatui)
│   └── lsp/                # LSP client integration
├── tests/                  # E2E + integration tests
├── docs/                   # Architecture, config, benchmarks, guides
├── website/                # Astro website (project landing)
├── Dockerfile              # Multi-stage Docker build
└── Cargo.toml              # Workspace manifest
```

## License

MIT