---
name: fathom-core
description: Core architecture, Rust workspace crates, tool registry, agent runtime, memory, and execution invariants for Fathom. Use when working on crates/ (core, agent, tools, server, memory, persistence, governance, supervisor) or main.rs.
---

# Fathom Core Architecture & Engineering Standards

Fathom is a compiled native Rust multi-agent autonomous workforce platform designed for zero-overhead, sovereign, on-premise execution.

## 1. Workspace Layout (13 Crates)

The project uses Cargo workspace with `resolver = "2"`. Dependencies point strictly downward:

```
crates/
├── core/         # PrError, PrResult, Session, AgentId, ToolCall, ToolOutput, Finding, Contact
├── llm/          # LlmProvider trait, DeepSeek / OpenAI-compatible streaming, retries, token counting
├── agent/        # Coordinator (DAG swarms), AgentRuntime (14-step loop), ToolExecutor, compaction
├── tools/        # 63 built-in tools + CDP + Computer Use + LSP (up to 75 tools)
├── mcp/          # Model Context Protocol: stdio/http client + mcp-serve server + bridge tool
├── persistence/  # SQLite (WAL) + optional PostgreSQL (ContactDb, SessionHistory, JobsDb)
├── memory/       # Long-term semantic memory (hybrid SQLite FTS5 + vector graph, absorb pipeline)
├── server/       # Axum HTTP/SSE server (fathom serve --port 8080), auth, rate limiting
├── tui/          # Terminal User Interface on ratatui with DAG tree & live sparklines
├── lsp/          # Language Server Protocol adapter & client integration
├── governance/   # PolicyEngine, action evaluation, allow/deny rules, cryptographic verification
├── supervisor/   # HostSandbox (bwrap/sandbox-exec) & Docker per-agent container supervisor
└── desktop/      # Lightweight native desktop client integration
```

## 2. Tool Registry & Invariants

* **Base Built-in Tools**: Exactly **63** unconditional tools registered in `ToolRegistry::with_builtins()` in `crates/tools/src/registry.rs`.
* **Extensions (Up to 75)**:
  * 6 Computer Use tools (`computer_*`) when `COMPUTER_URL` is set.
  * 5 CDP Browser tools (`browser_*`) when Chrome/CDP endpoint is reachable.
  * 1 LSP tool (`lsp`) via `register_lsp()`.
* **Execution Safety**:
  * Parallel tools (`parallel_safe: true`) execute concurrently via `tokio::spawn`.
  * Sequential tools (`parallel_safe: false`) execute in strict order.
  * File writes require `ReadTracker` validation (read-before-write).
  * Shell cascade cancellation: if a shell command fails in a batch, sibling shell executions are canceled.
  * Direct host execution in `ShellTool` MUST use `pr_supervisor::HostSandbox` to restrict execution to the active workspace.

## 3. Server & Network Invariants

* Binary CLI: `fathom serve --port 8080 --host 127.0.0.1`.
* Non-loopback addresses (e.g. `0.0.0.0`) are refused unless `FATHOM_API_KEYS` is defined.
* API key verification MUST use constant-time byte comparisons (`diff |= a ^ b`).
* Event stream: Server-Sent Events (SSE) at `/api/v1/sessions/{id}/events`.

## 4. Development & Testing Commands

```bash
# Workspace check and unit test suite
cargo check --workspace
cargo test --workspace

# Run single agent task CLI
./target/release/fathom run "Task instruction" --output ./results/

# Launch interactive TUI
./target/release/fathom tui
```
