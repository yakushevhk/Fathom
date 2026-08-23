# Contributing to Fathom

Welcome to Fathom — the universal autonomous AI worker. This guide covers everything you need to start contributing: building from source, understanding the architecture, adding tools and integrations, and submitting pull requests.

## 1. Getting Started

### Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| **Rust** | ≥ 1.97 (stable) | Primary language |
| **cargo** | Ships with Rust | Build system, package manager |
| **Node.js** | ≥ 18 | Website, Tauri desktop app (optional) |
| **Python 3** | ≥ 3.10 | Some tooling scripts (optional) |

Optional tools that unlock specific features or tests:

- `pandoc` — document conversion
- `ripgrep` (`rg`) — fast code search (tests use the `grep` tool internally)
- `protobuf-compiler` — if you touch proto definitions (rare)

### Clone & Build

```bash
git clone https://github.com/yakushevhk/Fathom.git
cd Fathom

# Build everything (workspace)
cargo build

# Run the binary
cargo run -- --help

# Build in release mode (LTO + strip)
cargo build --release
```

### Run Tests

```bash
# Unit tests across all crates
cargo test --workspace

# A single crate
cargo test -p pr-tools

# E2E tests (deterministic mock LLM, no API key needed)
cargo test --test e2e_basic_research

# Live-API tests (costs tokens, requires network + API key)
cargo test --test e2e_basic_research -- --ignored
```

### Project Structure

Fathom is a Rust workspace with 12 crates plus a top-level binary:

```
Fathom/
├── src/main.rs              # CLI entry point (clap)
├── Cargo.toml               # Workspace root
├── rust-toolchain.toml      # Pinned to 1.97.1
├── crates/
│   ├── core/                # Shared types, config, error, events, agent IDs
│   ├── llm/                 # LlmProvider trait, OpenAI-compatible clients
│   ├── agent/               # Coordinator, agent runtime, sub-agent spawning
│   ├── tools/               # Tool trait, ToolRegistry, all built-in tools
│   ├── mcp/                 # MCP client (stdio/http) + bridge + server
│   ├── persistence/         # SQLite/Postgres session storage
│   ├── memory/              # Long-term semantic memory (mem0-style)
│   ├── server/              # HTTP API server (fathom serve)
│   ├── tui/                 # Terminal UI (ratatui)
│   ├── lsp/                 # Language Server Protocol tool
│   ├── governance/          # Policy engine, approval gates
│   └── supervisor/          # Process supervision, restart policies
├── apps/
│   ├── web/                 # Next.js dashboard
│   ├── desktop/             # Tauri v2 desktop app
│   └── computer/            # Playwright browser automation service
└── tests/
    ├── e2e/                 # End-to-end (mock LLM)
    ├── integration/         # Cross-crate integration tests
    └── support/             # Test utilities (MockLlm, etc.)
```

**Crate dependency direction** (no cycles):

```
core ← llm ← agent ← tools
core ← persistence ← agent
core ← memory ← agent
tools ← mcp (bridge)
agent ← supervisor
agent ← governance
core ← server ← tui
```

`core` is the leaf — it depends on nothing internal. Every other crate depends on `core` for shared types (`ToolSchema`, `ToolOutput`, `AppConfig`, `PrError`, `AgentEvent`, etc.).

## 2. Adding a New Tool

Tools live in `crates/tools/src/`. Each tool category gets its own file (e.g., `search.rs`, `shell.rs`, `file.rs`). The `Tool` trait is defined in `crates/tools/src/registry.rs`.

### Step 1: Create the Tool File

Create a new file in `crates/tools/src/` — pick a descriptive name:

```
crates/tools/src/my_feature.rs
```

### Step 2: Implement the `Tool` Trait

The trait has four methods:

```rust
use async_trait::async_trait;
use pr_core::{ToolSchema, ToolOutput};
use pr_tools::registry::{Tool, ToolContext};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct MyToolParams {
    /// What to operate on
    target: String,
    /// Optional flag
    #[serde(default)]
    verbose: bool,
}

pub struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &str {
        "my_tool"
    }

    fn description(&self) -> &str {
        "Does something useful.\n\n## Parameters\n\n- `target` (required): What to operate on.\n- `verbose` (optional): Enable verbose output."
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

        // Your logic here. Use ctx.working_dir for the current directory,
        // ctx.http_client for outbound HTTP, etc.

        Ok(ToolOutput::ok(format!("Handled: {}", params.target)))
    }
}
```

**Key points:**

- `schema()` generates the JSON Schema that the LLM sees. Use `schemars::schema_for!()` from a `#[derive(JsonSchema)]` struct — don't hand-write JSON Schema.
- `execute()` returns `anyhow::Result<ToolOutput>`. Use `ToolOutput::ok()` for success, `ToolOutput::err()` for failures, or `ToolOutput::err_code()` with a machine-readable class (`rate_limited`, `timeout`, `blocked`, `not_found`, `network`, `parse`, `other`).
- The `ToolContext` provides shared subsystems: HTTP client, file history/lock manager, search config, optional LLM providers, contact DB, CRM, memory, receipt ledger, and the calling agent's ID.

### Step 3: Register the Tool

Add to the module list in `crates/tools/src/lib.rs`:

```rust
pub mod my_feature;
```

Then register in `ToolRegistry::with_builtins()` in `crates/tools/src/registry.rs`:

```rust
registry.register(Arc::new(crate::my_feature::MyTool));
```

### Parallel vs Sequential Classification

The `Tool` trait does not have a `parallel()` method — the agent runtime decides execution strategy. However, you should be aware:

- **Safe to parallelize**: read-only tools (search, fetch, read, grep, git status/log/diff)
- **Must serialize**: write tools (file_write, file_edit, shell, git commit/push) — the runtime uses file locks (`FileLockManager`) and the `ReadTracker` to enforce read-before-edit
- Tools that mutate shared state (memory absorb, contact save) should document their concurrency expectations in the description

### Unit Tests

Add `#[cfg(test)]` at the bottom of your tool file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use pr_tools::registry::ToolContext;
    use std::path::PathBuf;

    fn test_ctx() -> ToolContext {
        ToolContext::new(
            PathBuf::from("/tmp"),
            pr_core::SearchConfig::default(),
        )
    }

    #[tokio::test]
    async fn test_my_tool_basic() {
        let tool = MyTool;
        let args = serde_json::json!({ "target": "hello" });
        let result = tool.execute(args, &test_ctx()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("hello"));
    }

    #[tokio::test]
    async fn test_my_tool_missing_target() {
        let tool = MyTool;
        let args = serde_json::json!({});
        let result = tool.execute(args, &test_ctx()).await;
        assert!(result.is_err());
    }
}
```

### Integration Tests

For tests that exercise the tool through the registry or against real APIs, add a file in `tests/integration/`:

```rust
// tests/integration/my_tool.rs

use std::sync::Arc;
use pr_tools::ToolRegistry;

#[tokio::test]
async fn test_my_tool_via_registry() {
    let registry = ToolRegistry::with_builtins();
    let ctx = pr_tools::registry::ToolContext::new(
        std::path::PathBuf::from("."),
        pr_core::SearchConfig::default(),
    );

    let result = registry
        .execute("my_tool", serde_json::json!({ "target": "test" }), &ctx)
        .await
        .unwrap();

    assert!(result.success);
}
```

Then add the test entry in the workspace `Cargo.toml`:

```toml
[[test]]
name = "integration_my_tool"
path = "tests/integration/my_tool.rs"
```

## 3. Adding a Search Backend

Search backends are configured via the `[search]` section in `config.toml` and dispatched in `crates/tools/src/search.rs`.

### Step 1: Add Config Struct

In `crates/core/src/config.rs`, add a new config struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MySearchConfig {
    pub api_key: String,
}
```

Add it as an `Option` field to `SearchConfig`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    pub backend: String,
    // ... existing fields ...
    #[serde(default)]
    pub my_search: Option<MySearchConfig>,
}
```

### Step 2: Implement the Search Method

In `crates/tools/src/search.rs`, add a method to `SearchEngine`:

```rust
impl SearchEngine {
    async fn search_my_search(&self, query: &str, limit: u32) -> Vec<SearchResult> {
        let api_key = match &self.config.my_search {
            Some(c) => &c.api_key,
            None => return vec![],
        };

        // Call the API
        let resp = self.http
            .get("https://api.mysearch.com/v1/search")
            .query(&[("q", query), ("limit", &limit.to_string())])
            .header("Authorization", format!("Bearer {api_key}"))
            .send()
            .await;

        // Parse and return Vec<SearchResult>
        // ...
    }
}
```

### Step 3: Register in the Dispatcher

Add a match arm in `SearchEngine::search()`:

```rust
pub async fn search(&self, query: &str, limit: u32) -> Vec<SearchResult> {
    match self.config.backend.as_str() {
        // ... existing arms ...
        "my_search" => self.search_my_search(query, limit).await,
        _ => self.search_duckduckgo(query, limit).await,
    }
}
```

Also add it to the `search_hybrid` and `smart_search` methods if the backend should participate in fallback or parallel aggregation.

### API Key Handling

API keys live in `config.toml` under `[search.my_search]`:

```toml
[search]
backend = "my_search"

[search.my_search]
api_key = "sk-..."
```

Never hardcode keys. Use the `SearchConfig` struct which is deserialized from the config file.

## 4. Adding an MCP Server Integration

MCP (Model Context Protocol) lets you connect external tool servers to Fathom. The bridge in `crates/mcp/src/bridge.rs` automatically exposes MCP tools through the `ToolRegistry` as `mcp__{server}__{tool}`.

### MCP Client Configuration

Add a server to `config.toml`:

```toml
[[mcp.servers]]
name = "my_tools"
transport = "stdio"
command = "npx"
args = ["-y", "@my-org/mcp-server"]

# Or HTTP transport:
# [[mcp.servers]]
# name = "remote_tools"
# transport = "http"
# url = "https://my-server.example.com/mcp"
```

### Transport Types

| Transport | Config Fields | Protocol |
|-----------|--------------|----------|
| **stdio** | `command`, `args` | JSON-RPC over stdin/stdout, one message per line |
| **http** | `url` | JSON-RPC over HTTP POST; responses may be SSE (`text/event-stream`) |

HTTP transport supports optional OAuth 2.0 client-credentials auth (handled by `McpClient`).

### How Tool Bridging Works

1. On startup, `McpClient` connects to each configured server.
2. It calls `tools/list` to discover available tools (each has a name, description, and JSON Schema).
3. `McpBridgeTool` wraps each discovered tool and registers it in the `ToolRegistry` with a namespaced name: `mcp__my_tools__some_tool`.
4. When the agent calls `mcp__my_tools__some_tool`, `McpBridgeTool::execute()` forwards the call to the MCP server via `McpClient::call_tool()`.
5. The result is converted from MCP's `{content: [{type:"text", text}]}` format to `ToolOutput`.

If the MCP server process dies, `McpBridgeTool` attempts one automatic reconnect before failing.

### Adding a Custom Bridge

If you need to adapt a non-standard MCP tool (e.g., custom result format), implement `Tool` directly and wrap the `McpClient`:

```rust
use pr_tools::registry::{Tool, ToolContext};
use pr_core::{ToolSchema, ToolOutput};
use pr_mcp::client::McpClient;

pub struct MyBridgeTool {
    client: Arc<tokio::sync::Mutex<McpClient>>,
    server: String,
    tool_name: String,
}

#[async_trait]
impl Tool for MyBridgeTool {
    fn name(&self) -> &str { "mcp__custom__my_tool" }
    fn description(&self) -> &str { "Custom MCP bridge" }

    fn schema(&self) -> ToolSchema {
        // Use the schema discovered from the MCP server
        todo!()
    }

    async fn execute(&self, args: serde_json::Value, _ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let mut client = self.client.lock().await;
        let result = client.call_tool(&self.server, &self.tool_name, args).await?;
        // Custom result handling
        Ok(ToolOutput::ok(serde_json::to_string(&result)?))
    }
}
```

## 5. Code Style & Conventions

### Rust Edition & Toolchain

- **Edition**: 2021
- **MSRV**: 1.97 (see `rust-toolchain.toml` — pinned to `1.97.1`)
- **Components**: `rustfmt`, `clippy`

### Formatting & Linting

```bash
cargo fmt --all
cargo clippy --workspace -- -D warnings
```

Clippy warnings are treated as errors in clean builds. Address them before submitting.

### Error Handling

Fathom uses two crates for error handling, with a clear boundary:

| Crate | Used In | Pattern |
|-------|---------|---------|
| **thiserror** | Library crates (`core`, `llm`, `tools`, etc.) | Define typed error enums with `#[derive(Error)]` |
| **anyhow** | Binary (`src/main.rs`), `Tool::execute()` | `anyhow::Result<T>` for propagation with context |

**Core error type** (`crates/core/src/error.rs`):

```rust
#[derive(Debug, Error)]
pub enum PrError {
    #[error("LLM error: {0}")]
    Llm(String),
    #[error("Tool error: {0}")]
    Tool(String),
    #[error("Agent error: {0}")]
    Agent(String),
    #[error("HTTP error {status}: {message}")]
    Http { status: u16, message: String },
    #[error("Timeout after {0}s")]
    Timeout(u64),
    // ...
}

pub type PrResult<T> = Result<T, PrError>;
```

- Use `PrResult` in crate-to-crate boundaries.
- Use `anyhow::Result` inside tools and application code where you want `?` to work seamlessly.
- `PrError::is_retryable()` helps the agent runtime decide whether to retry.

### Async Runtime

All async code runs on **tokio** (full features). Use `#[tokio::test]` for async tests.

```rust
use tokio;

#[tokio::test]
async fn my_async_test() {
    // ...
}
```

For long-running tasks, the runtime uses `tokio::time::timeout` to enforce deadlines (see `ToolContext::http_client` — 30s timeout, 10s connect).

### Naming Conventions

| Element | Convention | Example |
|---------|-----------|---------|
| Functions / methods | `snake_case` | `execute()`, `search_duckduckgo()` |
| Types / structs / enums | `CamelCase` | `ToolRegistry`, `AgentEvent`, `SearchResult` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_SNIPPET_CHARS`, `RRF_K` |
| Crates | `kebab-case` (`pr-` prefix) | `pr-core`, `pr-tools` |
| Files | `snake_case.rs` | `search.rs`, `file_history.rs` |
| Modules | `snake_case` | `pub mod search;` |

### Module Structure

One file per logical unit. Keep files under 500 lines where possible — if a tool file grows past that, split helpers into a submodule.

### Dependencies

Use workspace dependencies defined in the root `Cargo.toml`:

```rust
// GOOD — reuses workspace dep
use tokio;
use serde::{Deserialize, Serialize};

// BAD — adds duplicate dep to crate Cargo.toml
```

When adding a new dependency, add it to `[workspace.dependencies]` in the root `Cargo.toml`, then reference it with `.workspace = true` in your crate.

## 6. Testing

### Unit Tests

Every crate has inline tests with `#[cfg(test)] mod tests`. These test individual functions and types:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_extracts_urls() {
        let input = "Visit https://example.com for info";
        let urls = extract_urls(input);
        assert_eq!(urls, vec!["https://example.com"]);
    }

    #[tokio::test]
    async fn test_tool_execute() {
        let tool = MyTool;
        let ctx = make_test_ctx();
        let result = tool.execute(serde_json::json!({"target": "x"}), &ctx).await.unwrap();
        assert!(result.success);
    }
}
```

### Integration Tests

Cross-crate tests live in `tests/integration/`. They exercise the full stack (registry, coordinator, persistence):

```rust
// tests/integration/export_notify.rs
use std::sync::Arc;
use pr_tools::ToolRegistry;
use pr_persistence::Persistence;

#[tokio::test]
async fn test_export_after_session() {
    let tmp = tempfile::tempdir().unwrap();
    let db = Arc::new(Persistence::open(&tmp.path().join("test.db")).unwrap());
    // ... setup, exercise, assert ...
}
```

### E2E Tests

End-to-end tests in `tests/e2e/` run the full coordinator pipeline with a **deterministic mock LLM** (`tests/support/mock_llm.rs`). No API key or network needed:

```rust
// tests/e2e/basic_research.rs
#[path = "../support/mock_llm.rs"]
mod mock_llm;

use mock_llm::MockLlm;
use pr_agent::Coordinator;

#[tokio::test]
async fn test_basic_research() {
    let tmp = tempfile::tempdir().unwrap();
    let coordinator = Coordinator::new(
        session_id,
        query,
        Arc::new(MockLlm::single_agent()),
        Arc::new(ToolRegistry::new()),
        event_tx,
        db.clone(),
        output_dir,
        config,
    );

    let output = coordinator.execute().await.unwrap();
    assert!(!output.synthesis.trim().is_empty());
}
```

Live-API E2E tests are marked `#[ignore]` and run manually:

```bash
cargo test --test e2e_basic_research -- --ignored
```

### Property Tests

Use `proptest` for data-heavy parsers and transformers. The workspace already depends on `proptest`:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_url_parsing_roundtrip(url in "https?://[a-z0-9.-]+\\.[a-z]{2,}/?") {
        let parsed = parse_url(&url);
        prop_assert!(parsed.is_ok());
    }
}
```

### Running Tests

```bash
# All tests in the workspace
cargo test --workspace

# Single crate
cargo test -p pr-tools

# Single test by name
cargo test test_basic_research

# With output displayed
cargo test -- --nocapture

# E2E (mock LLM)
cargo test --test e2e_basic_research
cargo test --test e2e_multi_agent
cargo test --test e2e_hub_messaging
cargo test --test e2e_real_tools

# Integration
cargo test --test integration_export_notify
cargo test --test integration_history
```

### Test Naming Convention

- `test_<what>` for unit tests: `test_parser_extracts_emails`, `test_output_ok_with_meta`
- `test_<scenario>` for integration: `test_export_after_session`, `test_registry_executes_builtin`
- `test_<full_feature_name>` for E2E: `test_basic_research`, `test_multi_agent`

## 7. Pull Request Process

### Branch Naming

```
feat/add-rss-search-backend      — new feature
fix/search-timeout-handling      — bug fix
refactor/extract-tool-traits     — refactor
docs/update-contributing-guide   — documentation
test/add-proptests-for-parser    — test additions
```

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(tools): add RSS feed search backend

- Implement SearchEngine::search_rss() with feed parsing
- Add RssConfig to SearchConfig
- Unit tests for feed URL validation
```

```
fix(mcp): reconnect on stdio server crash

MCPBridgeTool now attempts one reconnect before failing when the
stdio process exits unexpectedly.
```

### What to Include

- **Description**: What changed and why. Link to any related issues.
- **Test results**: Paste output of `cargo test --workspace` (or the specific tests that apply).
- **Breaking changes**: If the public API of any crate changed, document it. Update `Cargo.toml` version if needed.
- **Performance notes**: If the change affects hot paths (tool execution, LLM calls, search), note the before/after if measured.

### Review Checklist

Before submitting:

1. `cargo fmt --all` — formatting clean
2. `cargo clippy --workspace -- -D warnings` — no lint warnings
3. `cargo test --workspace` — all tests pass
4. New public APIs have doc comments (`///`)
5. New tools have a description that explains when to use *and* when NOT to use them
6. Config changes have defaults (so existing `config.toml` files don't break)
7. Error types use `thiserror` in library crates
8. No hardcoded API keys or credentials

## 8. Architecture Decisions

### DAG Dependency Graph

The workspace forms a **directed acyclic graph** (DAG). No crate may depend on another that depends back on it:

```
core → llm → agent → tools
core → persistence → agent
core → memory → agent
tools → mcp
agent → supervisor
agent → governance
core → server → tui
core → lsp
```

This means:
- `core` is the foundational crate — everything depends on it, it depends on nothing internal.
- `llm` knows about `core` but not `tools` or `agent`.
- `tools` knows about `core` and `llm` but not `agent` (tools are invoked *by* agents, not the other way around).
- `mcp` bridges external tools into `tools` — it depends on `tools` (not `agent`).

If you find yourself needing a circular dependency, extract the shared types into `core` instead.

### Why 12 Crates (Not Fewer)

Each crate enforces a **compile-time boundary** that prevents accidental coupling:

| Crate | Responsibility | Why Separate |
|-------|---------------|--------------|
| `core` | Shared types, config, error, events | Foundation — prevents circular deps |
| `llm` | LLM provider trait + clients | Swappable backends without touching agent logic |
| `agent` | Coordinator, runtime, spawning | Orchestration — depends on everything but is never depended on by tools |
| `tools` | Tool trait + all built-in tools | Tools are independently testable, hot-reloadable |
| `mcp` | MCP protocol client + bridge | External integration isolated from core tool logic |
| `persistence` | SQLite/Postgres session store | Storage layer swap without touching agent logic |
| `memory` | Long-term semantic memory | Optional subsystem, no-op when detached |
| `server` | HTTP API (REST + SSE) | API layer independent of TUI |
| `tui` | Terminal UI (ratatui) | UI layer independent of HTTP |
| `lsp` | Language Server Protocol | Language features for the editor integration |
| `governance` | Policy engine, approvals | Security boundary — can be audited in isolation |
| `supervisor` | Process lifecycle | Operational concerns separated from business logic |

Splitting further (e.g., separating each tool category into its own crate) would add build overhead with minimal benefit. 12 crates is the sweet spot for compile-time isolation without excessive dependency ceremony.

### Tool Dispatch Design

The tool dispatch pipeline:

1. **Registration**: `ToolRegistry::with_builtins()` creates a `HashMap<String, Arc<dyn Tool>>` and registers all built-in tools. MCP bridge tools are added after the server connections are established.

2. **Schema emission**: `ToolRegistry::list_schemas()` collects all `ToolSchema` objects. These are serialized into the system prompt so the LLM knows what tools are available.

3. **Execution**: `ToolRegistry::execute(name, args, ctx)` does a hashmap lookup, calls `tool.execute(args, ctx).await`, and returns `ToolOutput`.

4. **Context injection**: `ToolContext` is constructed once per session with shared resources (HTTP client, file caches, optional LLM, etc.) and passed to every tool call. Tools that need subsystems (e.g., `save_contacts` needs `contact_db`) access them through `ctx`.

5. **Guard layer**: Before execution, the agent runtime applies the `Guard` (`crates/tools/src/guard.rs`) for file-read-before-edit validation and destructive command blocking.

### Event Bus Pattern

Fathom uses **tokio's `broadcast` channel** as its event bus:

```rust
use tokio::sync::broadcast;

let (event_tx, _event_rx) = broadcast::channel::<AgentEvent>(1024);
```

`AgentEvent` (`crates/core/src/event.rs`) is a tagged enum with variants for every lifecycle event:

```rust
#[serde(tag = "type")]
pub enum AgentEvent {
    SessionStarted { id: SessionId, query: String },
    AgentSpawned { id: AgentId, parent: Option<AgentId>, role: String, ... },
    AgentStateChanged { id: AgentId, state: AgentState },
    ToolCallStarted { agent_id: AgentId, tool: String, args: Value },
    ToolCallCompleted { agent_id: AgentId, tool: String, duration_ms: u64 },
    Finding { agent_id: AgentId, finding: Finding },
    AgentCompleted { id: AgentId, summary: String, tokens_used: u64 },
    SessionCompleted { id: SessionId, ... },
    // ...
}
```

Events flow one direction: agents *emit*, the coordinator/server *consumes*. The TUI subscribes via `event_rx.resubscribe()` to update the terminal display. The HTTP server streams them as SSE to connected clients.

---

## Questions?

Open a [GitHub Discussion](https://github.com/yakushevhk/Fathom/discussions) or file an issue. For security-related concerns, see `docs/GOVERNANCE.md`.
