# Development

Developer guide: project structure, building, testing, adding tools, adding LLM providers, benchmarking, debugging agents, CI/CD, and architectural principles.

---

## Project Structure

The project is a Rust workspace organised into 12 crates (`crates/`), each with a single responsibility. The binary entry point is `src/main.rs`, which dispatches to CLI subcommands (`run`, `worker`, `tui`, `serve`, `config`, `jobs`, `profiles`, `contacts`, `sessions`, `memory`, `mcp-serve`, `bench`, `stats`).

```
fathom/
├── Cargo.toml              # Workspace root + binary
├── src/
│   ├── main.rs             # CLI: run/worker/tui/serve/config
│   └── bench.rs            # Offline benchmark harness + session stats
├── crates/
│   ├── core/               # Types, config, memory, export, CRM
│   │   └── src/
│   │       ├── ids.rs
│   │       ├── message.rs
│   │       ├── agent.rs
│   │       ├── event.rs
│   │       ├── config.rs
│   │       ├── memory.rs
│   │       ├── skill.rs
│   │       ├── export.rs
│   │       ├── notify.rs
│   │       ├── crm.rs
│   │       ├── token.rs
│   │       └── error.rs
│   ├── llm/                # LLM providers
│   │   └── src/
│   │       ├── provider.rs     # LlmProvider trait
│   │       ├── deepseek.rs     # DeepSeek implementation
│   │       ├── retry.rs        # Retry logic
│   │       ├── types.rs        # CompletionRequest/Response
│   │       ├── factory.rs      # Provider factory
│   │       └── concurrency.rs  # ModelSemaphore, FallbackCooldown
│   ├── agent/              # Agent runtime
│   │   └── src/
│   │       ├── runtime.rs      # AgentRuntime
│   │       ├── coordinator.rs  # Coordinator
│   │       ├── compaction.rs   # Context compaction
│   │       ├── prompt.rs       # PromptBuilder
│   │       ├── tool_executor.rs
│   │       ├── budget.rs
│   │       ├── ipc.rs
│   │       ├── process_manager.rs
│   │       ├── doom_loop.rs
│   │       ├── resume.rs
│   │       ├── hooks.rs
│   │       ├── control.rs
│   │       ├── task_tree.rs
│   │       ├── improvement.rs
│   │       └── reflection.rs
│   ├── tools/              # 51 always + up to 5 CDP + up to 6 computer tools
│   │   └── src/
│   │       ├── registry.rs     # ToolRegistry, ToolContext
│   │       ├── web.rs
│   │       ├── file.rs
│   │       ├── shell.rs
│   │       ├── browser.rs
│   │       ├── vision.rs
│   │       ├── git.rs
│   │       ├── pdf.rs
│   │       ├── repl.rs
│   │       ├── extract.rs      # Contact extraction
│   │       ├── directories.rs
│   │       ├── social_search.rs
│   │       ├── corporate.rs
│   │       ├── news.rs
│   │       ├── lead_finder.rs
│   │       ├── verify_email.rs
│   │       ├── verify_phone.rs
│   │       ├── verify_social.rs
│   │       ├── enrich_company.rs
│   │       ├── enrich_person.rs
│   │       ├── search.rs       # SearchEngine (7 backends)
│   │       ├── memory_tool.rs
│   │       ├── spawn.rs
│   │       ├── truncate.rs
│   │       ├── file_history.rs
│   │       └── file_lock.rs
│   ├── mcp/                # Model Context Protocol
│   │   └── src/
│   │       ├── client.rs
│   │       └── server.rs
│   ├── persistence/        # Data storage
│   │   └── src/
│   │       ├── db.rs           # Persistence (SQLite WAL)
│   │       ├── contacts.rs     # ContactDb
│   │       ├── pg.rs           # PgContactDb
│   │       └── history.rs      # SessionHistory
│   ├── server/             # HTTP API
│   │   └── src/
│   │       ├── lib.rs          # Axum routes
│   │       ├── auth.rs         # API key + rate limiting
│   │       └── metrics.rs      # Prometheus metrics
│   └── tui/                # Terminal UI
│       └── src/
│           ├── app.rs          # App state
│           ├── ui.rs           # Rendering
│           ├── event.rs        # Event handling
│           └── streaming.rs    # Streaming buffer
│   ├── governance/         # Policy engine (allow/deny decisions)
│   │   └── src/
│   │       ├── lib.rs          # ActionContext, PolicyEngine, AuditEvent
│   │       └── …
│   └── supervisor/         # Per-agent Docker computer provisioning
│       └── src/
│           └── lib.rs          # SupervisorConfig, ComputerSupervisor
├── apps/
│   ├── computer/           # Playwright loopback computer service (Node/TS)
│   ├── desktop/            # Tauri v2 desktop app (Rust + TypeScript)
│   └── web/                # Next.js web dashboard (SSE, chat, agents, jobs)
├── tests/
│   ├── e2e/                # End-to-end tests
│   │   ├── basic_research.rs
│   │   ├── multi_agent.rs
│   │   ├── hub_messaging.rs
│   │   └── real_tools.rs
│   ├── integration/        # Integration tests
│   │   ├── export_notify.rs
│   │   └── history.rs
│   └── support/
│       └── mock_llm.rs     # MockLlm for offline tests
├── docs/                   # Documentation
└── Dockerfile
```

### Crate dependency graph

The workspace uses a strict layering strategy to minimise compilation units and avoid circular dependencies:

| Crate | Depends on | Responsibility |
|-------|-----------|----------------|
| **pr-core** | *(none)* | Fundamental types, config, error types, memory models, export, CRM, skills, notifications |
| **pr-llm** | pr-core | `LlmProvider` trait, DeepSeek/OAI-compatible impl, retry, concurrency, factory |
| **pr-persistence** | pr-core | SQLite (WAL) + PostgreSQL storage, session history, contact DB, jobs DB |
| **pr-tools** | pr-core, pr-llm | 51 always-registered tools, optional CDP/computer tools, registry, search backends |
| **pr-memory** | pr-core, pr-persistence | Long-term semantic memory (hybrid vector + BM25, mem0/Memora-inspired) |
| **pr-mcp** | pr-core, pr-tools | Model Context Protocol client + server |
| **pr-agent** | pr-core, pr-llm, pr-tools, pr-persistence, pr-memory | Agent runtime, coordinator, compaction, IPC, prompts, doom-loop detection, hooks, resume, budget |
| **pr-governance** | pr-core | Policy engine (ActionContext, PolicyRule, PolicyConfig, decide/allow/deny) |
| **pr-supervisor** | *(none)* | Docker-based per-agent computer provisioning (SupervisorConfig, ComputerSupervisor, AgentContainer) |
| **pr-server** | pr-core, pr-llm, pr-agent, pr-persistence, pr-governance, pr-supervisor | Axum HTTP API with auth and Prometheus metrics |
| **pr-tui** | pr-core, pr-llm, pr-agent | Ratatui terminal UI |
| **pr-lsp** | pr-core | LSP-based code intelligence tool |
| **binary** | all crates + ratatui + crossterm | CLI entry point, contacts, jobs, sessions, memory, profiles, bench, stats |

**Key design constraint**: `pr-core` depends on **nothing** beyond the standard library and common serde/tokio ecosystem crates. This ensures the fundamental types compile in isolation and any crate can reference them without pulling in the rest of the stack.

---

## Setting up the development environment

### Prerequisites

- **Rust toolchain**: Install via [rustup](https://rustup.rs/). The project uses edition 2021 and declares Rust **1.97** as its MSRV and supported build baseline; `rust-toolchain.toml` pins reproducible local builds to **1.97.1**, while Docker uses `rust:1.97-bookworm`. Resolved dependencies may advertise a lower metadata floor (currently around 1.88); that floor is informational only and is not a supported Fathom toolchain.
- **Cargo** (comes with rustup).
- **pandoc** (optional, for PDF/DOCX export): `brew install pandoc` on macOS, `apt install pandoc` on Debian/Ubuntu.

### Quick start

```bash
# Clone the repository
git clone <repo-url> && cd fathom

# Debug build (fast iteration)
cargo build

# Verify the CLI works
./target/debug/fathom --help
```

### Configuration

The project expects a `~/.fathom/config.toml` file. A minimal config looks like:

```toml
[llm]
provider = "openai-compatible"
base_url = "https://api.openai.com/v1"
api_key = "sk-..."
model = "gpt-4o"
# Optional cheaper model for high-volume auxiliary calls
fast_model = "gpt-4o-mini"
```

The config system supports dotted-key overrides via `fathom config set llm.model gpt-4o-mini`. Every section uses `#[serde(default)]` for backward compatibility — adding new config keys never breaks existing files.

### Running a research session

```bash
# Basic usage
cargo run -- run "What are the latest developments in Rust?"

# With a specific profile/persona
cargo run -- run "Research vector databases" --profile researcher

# JSON output
cargo run -- run "Compare cloud providers" --output report.json
```

---

## Building

```bash
# Debug (fast iteration)
cargo build

# Release (optimized, with LTO and symbol stripping)
cargo build --release

# Check without building (fastest feedback)
cargo check

# Clippy (lints)
cargo clippy --workspace

# Formatting
cargo fmt
```

The release profile uses `lto = true` and `strip = true` for minimal binary size and maximum runtime performance. The `reqwest` dependency is built with `rustls-tls` (not `native-tls`) so the binary runs on Alpine/musl-based containers without a system CA bundle or libssl.

---

## Testing

**1,499 test annotations** (all passing) across the workspace:

| Crate | Tests | What they cover |
|-------|-------|-----------------|
| pr-tools | 457 | Tool execution, search backends, browser automation, OSINT tools, contact extraction, file operations, schema validation |
| pr-core | 229 | Config parsing, error handling, memory model, export formats, CRM sync, token accounting |
| pr-agent | 190 | Runtime loop, coordinator, compaction, doom-loop detection, IPC, resume, hooks, budget, task tree, improvement, reflection |
| pr-memory | 135 | Memory absorb, dedup, classification, hybrid search, vector + BM25, GC |
| pr-server | 123 | HTTP routes, authentication, rate limiting, metrics, computer relay |
| pr-llm | 68 | Provider factory, retry logic, concurrency, semaphore, streaming |
| pr-tui | 59 | UI rendering, event handling, streaming buffer |
| pr-persistence | 57 | SQLite WAL, PostgreSQL, session history, jobs DB, contact store, credentials |
| pr-lsp | 53 | LSP client, language server detection, code intelligence |
| pr-mcp | 31 | MCP client, server, tool discovery |
| pr-governance | 4 | Policy engine, rules, audit events |
| pr-supervisor | 2 | Container lifecycle, config validation |
| binary (`src/`) | 79 | CLI subcommands, config set, bench harness, session stats |
| E2E + Integration | 12 | Full research pipeline, multi-agent coordination, hub messaging, real-tool execution, export/notify, session history |

### Running tests

```bash
# All tests across all crates
cargo test --workspace

# Specific crate
cargo test -p pr-tools

# Specific test by name (supports substring matching)
cargo test -p pr-tools test_extract_emails_plain

# With output (for debugging)
cargo test -p pr-tools test_name -- --nocapture

# E2E tests (require network for live variant)
cargo test --test basic_research

# Ignored live tests (with real API keys)
PR_CDP_LIVE=1 cargo test -p pr-tools live_cdp -- --ignored
```

### Test categories

1. **Unit tests** — inline in each module (`#[cfg(test)] mod tests { ... }`). Fast, deterministic, no network.
2. **Integration tests** — `tests/integration/` — test export/notify pipelines and session history. Use a real SQLite database (tempfile).
3. **E2E tests** — `tests/e2e/` — test the full research pipeline with `MockLlm`. `basic_research.rs` tests a single-agent flow; `multi_agent.rs` tests hierarchical delegation; `hub_messaging.rs` tests inter-agent communication via the IrcBus; `real_tools.rs` tests the tool execution layer with real file system and network tools.
4. **Live tests** — annotated with `#[ignore]`, run only with environment variables. They hit real APIs and require configured credentials.

### MockLlm for offline tests

`tests/support/mock_llm.rs` provides a deterministic LLM mock that returns pre-programmed responses. This is the backbone of E2E testing — it lets you verify agent behaviour (tool selection, delegation, recovery) without a real API:

```rust
let mock = MockLlm::new(vec![
    MockResponse::text("Response without tool calls"),
    MockResponse::tool_call("web_search", json!({"query": "Rust 2024"})),
    MockResponse::text("Here are the results..."),
]);
```

Each response is consumed in order. When the agent exhausts the pre-programmed responses, the mock repeats the last one (so deep or recursive agent trees don't deadlock).

---

## Adding a new tool

The tool system is designed for extensibility. Each tool implements the `Tool` trait (defined in `crates/tools/src/registry.rs`), which provides schema generation, execution, and metadata.

### 1. Create the tool file

`crates/tools/src/my_tool.rs`:

```rust
use async_trait::async_trait;
use pr_core::{ToolSchema, ToolOutput};
use crate::registry::{Tool, ToolContext};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct MyToolParams {
    /// Parameter description
    input: String,
}

pub struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &str { "my_tool" }

    fn description(&self) -> &str {
        "Detailed description: what it does, when to use, when NOT to use."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(
                &schemars::schema_for!(MyToolParams).schema
            ).unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: MyToolParams = serde_json::from_value(args)?;

        // Tool logic
        let result = do_something(&params.input).await?;

        Ok(ToolOutput::ok(result))
        // Or: Ok(ToolOutput::err("Error"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_tool() {
        // Tests
    }
}
```

### 2. Register the module

`crates/tools/src/lib.rs`:
```rust
pub mod my_tool;
```

### 3. Add to registry

`crates/tools/src/registry.rs` in `with_builtins()`:
```rust
registry.register(Arc::new(crate::my_tool::MyTool));
```

The `with_builtins()` method is the single registration point for all built-in tools. It currently registers 51 always-available tools, plus up to 5 CDP browser tools and 6 computer-use tools when their services are configured, across categories: web fetch/search, file I/O, shell, browser automation, computer use, vision, git, PDF, code REPLs, OSINT lead generation, contact extraction, memory, coordination, and hierarchical delegation.

### 4. Classify for parallelism

`crates/agent/src/tool_executor.rs`:
- **Read-only tools** → add to `parallel_safe` (`web_search`, `web_fetch`, `file_read`, `glob`, `grep`, all OSINT verification tools, `web_crawl`, `web_feed`, `code_symbols`, `repo_map`)
- **Write/state tools** → add to `sequential_only` (`file_write`, `file_edit`, `shell`, `spawn_agent`, `save_contacts`)

The current batch executor classifies calls using `parallel_safe` and `sequential_only` name sets. Parallel-safe tools run concurrently, while sequential-only and unknown tools run sequentially. Overlapping file paths demote otherwise parallel-safe file calls to sequential execution.

Unknown tools default to sequential for safety.

### 5. Tests

```bash
cargo test -p pr-tools my_tool
```

### Tool execution flow

1. The LLM returns a message with one or more tool calls.
2. The batch executor partitions calls into parallel-safe and sequential groups.
3. **Phase 1**: Parallel-safe calls execute concurrently.
4. **Phase 2**: Sequential-only and unknown calls execute one at a time, in order.
5. Results are collected, truncated (if they exceed `ContextConfig.tool_output_max_bytes`), and fed back to the LLM.
6. If a tool returns an error, the error message is returned to the model as a tool result — it does **not** crash the agent loop.

---

## Adding a new LLM provider

All current providers speak the OpenAI-compatible chat-completions protocol, so any endpoint implementing it (DeepSeek, OpenAI, OpenRouter, vLLM, Ollama, LM Studio, etc.) works by pointing `base_url` at it.

### 1. Add config in `crates/core/src/config.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyProviderConfig {
    pub api_key: String,
    pub endpoint: String,
}
```

Add the new section to `AppConfig` with `#[serde(default)]` for backward compatibility.

### 2. Implement the `LlmProvider` trait

`crates/llm/src/my_provider.rs`:

```rust
use async_trait::async_trait;
use pr_core::PrResult;
use crate::provider::LlmProvider;
use crate::types::{CompletionRequest, CompletionResponse, StreamChunk};
use futures::Stream;

pub struct MyProvider { ... }

#[async_trait]
impl LlmProvider for MyProvider {
    fn name(&self) -> &str { "my_provider" }
    fn model(&self) -> &str { &self.model }

    async fn complete(&self, req: &CompletionRequest) -> PrResult<CompletionResponse> { ... }

    async fn stream(
        &self,
        req: &CompletionRequest,
    ) -> PrResult<Box<dyn Stream<Item = PrResult<StreamChunk>> + Send + Unpin>> { ... }
}
```

The trait requires two methods: `complete()` for non-streaming (used by the coordinator for simple completions) and `stream()` for SSE streaming (used by the agent runtime and TUI). Both receive a `CompletionRequest` containing the message history, tool schemas, and generation parameters.

### 3. Register in the factory

`crates/llm/src/factory.rs`:

```rust
const KNOWN_PROVIDERS: &[&str] = &[
    "deepseek",
    "openai",
    "openrouter",
    "ollama",
    "vllm",
    "lmstudio",
    "openai-compatible",
    "my_provider",  // <-- add
];

pub fn build_provider(cfg: &LlmConfig) -> anyhow::Result<Arc<dyn LlmProvider>> {
    // ...
    Ok(Arc::new(MyProvider::new(&cfg.base_url, &cfg.api_key, &cfg.model)))
}
```

### 4. Add module to `lib.rs`

```rust
pub mod my_provider;
```

### Providers that don't speak OpenAI-compatible protocol

If your provider uses a different protocol (e.g. Anthropic Messages API, Google Gemini API), you need to:

1. Implement the `LlmProvider` trait with its own HTTP client and serialisation.
2. Add provider-specific config fields in `LlmConfig`.
3. Handle the non-standard protocol in `factory.rs` with a conditional branch.
4. Ensure streaming (SSE) is mapped to the `Stream<Item = PrResult<StreamChunk>>` trait.

### Retry and concurrency

The `retry.rs` module handles exponential backoff with jitter for all providers. It retries only *retryable* errors (network/timeout, HTTP 408/429/5xx) and respects `Retry-After` headers. Permanent errors (400/401/403/404) return immediately.

The `concurrency.rs` module provides `ModelSemaphore` — a per-model concurrency limiter that prevents overloading rate-limited endpoints. It also provides `FallbackCooldown` for model-level backoff when a provider signals capacity limits.

---

## Adding a new search backend

The search engine (`crates/tools/src/search.rs`) currently supports seven named backends: LinkUp, Parallel AI, Exa, Tavily, Serper, Brave, and DuckDuckGo, plus `hybrid` and `smart` combination modes.

1. Add config in `crates/core/src/config.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MySearchConfig {
    pub api_key: String,
}
```
Add it to `SearchConfig` with `#[serde(default)]`.

2. Implement search in `crates/tools/src/search.rs`:
```rust
async fn search_my_backend(&self, query: &str, limit: u32) -> Vec<SearchResult> {
    // HTTP request to API
}
```

3. Add to `search()` dispatch and to both combination modes: `hybrid` tries configured backends sequentially and returns the first non-empty result; `smart` queries configured backends in parallel and merges them with reciprocal-rank fusion.

---

## Architectural principles

1. **`core` depends on nothing** — fundamental types (messages, agent IDs, config, errors) live in `pr-core` with zero intra-workspace dependencies. Every other crate depends on `pr-core`.

2. **Tools return `ToolOutput`** — do not propagate `Err` to the model. Tool failures are serialised as structured error results and returned to the LLM for recovery.

3. **Errors → model** — tool failures are returned as tool results, do not interrupt the loop. The agent can retry, try a different tool, or report the failure to the user. Only fatal errors (config, panic, cancellation) break the loop.

4. **Char-boundary safe** — all string slices check UTF-8 boundaries. The `safe_prefix()` function in `deepseek.rs` and the compaction engine never split a multi-byte character.

5. **Graceful degradation** — network errors are logged, do not crash agents. If a search backend is unreachable, the engine falls back to the next available backend. If the LLM is unreachable, the agent retries with backoff. If memory store is unavailable, the agent continues without it.

6. **Config backward compatibility** — new sections use `#[serde(default)]`. The config is parsed with `toml` and validated against the `AppConfig` struct. Unknown keys or wrong types are rejected at write time via `set_config_value()`.

7. **Context compaction** — the `CompactionEngine` uses a hybrid necessity-vs-utility gate. When the transcript exceeds 50% of the context window, it triggers micro-compaction (pruning old tool results without LLM calls). If that's insufficient, it uses LLM summarisation to condense older conversation turns. A cooldown mechanism prevents re-compacting when the last pass was ineffective, preserving the LLM prefix cache.

8. **Doom-loop detection** — the `DoomLoopDetector` tracks the last N tool call signatures. If 3 consecutive calls are identical (same tool, same argument hash), the agent is nudged to try a different approach. A second offense stops the agent.

9. **Session resume** — when the process crashes, sessions remain in the database with status `running`. The `SessionResumer` finds sessions stale for >5 minutes, reconstructs `ResumeState` (completed agent outputs + pending tasks), and the coordinator re-executes unfinished work while preserving recovered results.

10. **Lifecycle hooks** — hooks are external subprocesses invoked at PreToolUse, PostToolUse, and Stop stages. They receive JSON on stdin and return a JSON verdict. A PreToolUse hook can deny a tool call (e.g. security policy). A Stop hook can force a continuation when the agent tries to stop prematurely.

---

## Debugging

### Logging

The project uses `tracing` with the `env-filter` subscriber for structured, hierarchical logging:

```bash
# Verbose logs
RUST_LOG=debug ./target/release/fathom run "..."

# Specific module only
RUST_LOG=pr_agent=debug,pr_tools=trace ./target/release/fathom run "..."

# Backtrace on panic
RUST_BACKTRACE=1 ./target/release/fathom run "..."
```

### Debugging the agent runtime

The agent runtime emits structured `AgentEvent` values on a broadcast channel. These are consumed by the TUI for live display and by the `serve` command for WebSocket streaming. The event types include:

- `AgentStarted` / `AgentCompleted` — lifecycle events
- `MessageReceived` — each LLM round-trip
- `ToolCallStarted` / `ToolCallCompleted` — per-tool timing
- `TokenUsage` — running token counts
- `CompactionTriggered` — when context compaction fires
- `DoomLoopWarning` / `DoomLoopStopped` — doom-loop detection
- `StallWarning` / `StallKilled` — stall detection

### Debugging tool execution

- Use `RUST_LOG=pr_tools=trace` to see every tool call, its arguments, result, and duration.
- The `--output` flag writes the full session report to a JSON file, including all tool calls and their results.
- Session history is stored in SQLite (`~/.fathom/sessions.db`) — query it with `fathom sessions list` and `fathom sessions show <id>`.

### Debugging the coordinator

The coordinator manages hierarchical agent trees. Key debugging techniques:

- **Stall monitoring**: The coordinator runs a background task that checks every 30 seconds. If an agent hasn't produced a new message in `stall_warn` seconds (default 450), a warning is logged. After `stall_kill` seconds (default 1200), the agent is killed.
- **Heartbeat**: Each session writes a heartbeat timestamp every 60 seconds. Sessions without recent heartbeats are considered interrupted and eligible for resume.
- **Token budget tracking**: The coordinator tracks total tokens across all agents. When `session_token_limit` is set, it caps per-agent budgets and skips unstarted tasks once the limit is reached.

### Debugging context compaction

- `RUST_LOG=pr_agent::compaction=debug` shows compaction decisions (necessity/utility, micro-pruning, LLM summarisation, cooldown/hysteresis).
- The compaction engine logs: tokens before/after, number of micro-pruned messages, whether LLM summarisation was used, and any cooldown triggers.

---

## Benchmarking

The `bench` subcommand (`src/bench.rs`) measures the tool-execution layer without any LLM or network involvement. It focuses on:

- **Dispatch overhead**: serialising/deserialising tool arguments and routing through the registry (benchmark baseline is approximately 753 µs, or 0.75 ms).
- **Parallel vs sequential throughput**: how many parallel-safe tool calls can be dispatched per second.
- **CPU-bound parsing throughput**: HTML parsing, JSON extraction, RSS/Atom feed parsing, code symbol extraction.
- **Argument serde cost**: Schema generation and JSON round-trip overhead.

The `stats` subcommand analyses a recorded session database from real runs, producing per-tool duration histograms, success rates, peak concurrency, and batching profiles.

```bash
# Run all benchmarks
cargo run -- bench

# Specific benchmark scenario
cargo run -- bench dispatch

# Analyse a real session
cargo run -- stats --output ~/.fathom/sessions/<id>.research.db
```

### Benchmark scenarios

| Scenario | What it measures |
|----------|-----------------|
| `dispatch` | Tool registry lookup + argument serde round-trip |
| `parallel_io` | Concurrent parallel-safe tool execution |
| `parallel_cpu` | CPU-bound parsing under concurrent load |
| `mixed` | Mixed parallel + sequential batches |
| `parse_scale` | HTML parse throughput at varying input sizes |
| `extract_json` | JSON extraction throughput |
| `feed_parse` | RSS/Atom feed parsing |
| `code_map` | Code symbol extraction |
| `memory` | Memory absorb/search pipeline |

### Performance characteristics

- **Context compaction** prevents context overflow by pruning old tool results and summarising conversation turns. Micro-compaction prunes without LLM calls; full compaction uses the LLM to condense.
- **Tool result truncation** limits large outputs to `ContextConfig.tool_output_max_bytes` (default 50 KB) and `tool_output_max_lines` (default 2000 lines).
- **Smart parallelism** speeds up read-only operations by running them concurrently. The partitioner also detects overlapping file paths to prevent data races.
- **SQLite WAL mode** for concurrent read/write access from multiple agents without write contention.
- **Frozen memory snapshot** preserves the LLM prefix cache between rounds by keeping the system prompt and earlier conversation turns stable.
- **Result budget** (`budget.rs`) caps sub-agent outputs to a fair share of the parent's context window, spilling full text to disk when it exceeds the cap.

---

## CI/CD

The project is designed for GitHub Actions but the pipeline works with any CI provider. The recommended pipeline:

```yaml
# .github/workflows/ci.yml
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.97.1
      - run: cargo test --workspace
      - run: cargo clippy --workspace -- -D warnings

  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.97.1
      - run: cargo build --release
      - uses: actions/upload-artifact@v4
        with:
          name: fathom
          path: target/release/fathom
```

### CI best practices

1. **Caching**: Use `Swatinem/rust-cache@v2` to cache `target/` and `~/.cargo/` between runs. This drops CI times from ~15 minutes to ~2 minutes for most changes.
2. **Clippy deny**: `-D warnings` ensures no lint warnings slip through.
3. **Formatting check**: Add `cargo fmt --check` to enforce consistent formatting.
4. **MSRV check**: The project declares Rust `1.97` as its MSRV. Keep a CI matrix entry for `1.97` (matching the `rust:1.97-bookworm` Docker build image) to avoid accidental regressions; do not use dependency metadata floors as a supported-toolchain claim.

### Docker deployment

The `Dockerfile` provides a containerised build; no compose file is shipped:

```bash
docker build -t fathom .
docker run --rm -v ~/.fathom:/root/.fathom fathom run "Your query"
```

The Docker image uses `rustls-tls` for HTTPS (no system CA bundle needed) and `musl` targets for truly static binaries that run on any Linux distro.

---

## Known limitations

- **LinkedIn** requires proxies/cookies (HTTP 999 anti-bot). The `social_search` tool works with Google dorking and public profiles but cannot reliably scrape LinkedIn profiles.
- **2GIS/Google Maps** require API keys for stability. The geocoding and directory search tools work without keys but at reduced reliability.
- **PDF/DOCX export** requires pandoc installed on the system. HTML export works without any external dependencies.
- **Vision model** requires `PARALLEL_VISION_API_KEY` environment variable. The `VisionTool` is disabled when this key is not set.
- **Browser automation** requires a Chrome DevTools Protocol (CDP) endpoint. The `PARALLEL_CDP_ENDPOINT` environment variable or `[browser] cdp_url` config key must point to a running Chrome/Chromium instance with `--remote-debugging-port`.

---

## Use cases

- **Research automation**: Decompose a broad question into sub-topics, delegate to research agents, synthesise findings into a report.
- **Lead generation**: Search directories, social networks, and corporate databases for contacts, verify email/phone/social profiles, enrich with company data, save to CRM.
- **Code analysis**: Explore repositories, extract symbols, map dependencies, search for patterns, and generate summaries.
- **Competitive intelligence**: Monitor news, track pricing changes, analyse product pages, and generate reports.
- **Due diligence**: Research companies, people, and markets with multi-source verification and cross-referencing.