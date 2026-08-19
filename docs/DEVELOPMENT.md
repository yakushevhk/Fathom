# Development

Developer guide: project structure, building, testing, adding tools.

---

## Project Structure

```
parallel-research/
├── Cargo.toml              # Workspace root + binary
├── src/
│   └── main.rs             # CLI: run/worker/tui/serve/config
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
│   │       └── types.rs
│   ├── agent/              # Agent runtime
│   │   └── src/
│   │       ├── runtime.rs      # AgentRuntime
│   │       ├── coordinator.rs  # Coordinator
│   │       ├── compaction.rs   # Context compaction
│   │       ├── prompt.rs       # PromptBuilder
│   │       ├── tool_executor.rs
│   │       ├── budget.rs
│   │       ├── background.rs
│   │       ├── ipc.rs
│   │       ├── process_manager.rs
│   │       ├── doom_loop.rs
│   │       ├── resume.rs
│   │       └── recovery.rs
│   ├── tools/              # 35 tools
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
├── tests/
│   ├── e2e/                # End-to-end tests
│   │   ├── basic_research.rs
│   │   └── multi_agent.rs
│   ├── integration/        # Integration tests
│   │   ├── export_notify.rs
│   │   └── history.rs
│   └── support/
│       └── mock_llm.rs     # MockLlm for offline tests
├── docs/                   # Documentation
├── Dockerfile
├── docker-compose.yml
├── install.sh
└── parallel-research.service
```

---

## Building

```bash
# Debug (fast iteration)
cargo build

# Release (optimized)
cargo build --release

# Check without building
cargo check

# Clippy
cargo clippy --workspace

# Formatting
cargo fmt
```

---

## Testing

**624 tests** (all passing):

| Crate | Tests |
|-------|-------|
| pr-tools | 293 |
| pr-core | 108 |
| pr-agent | 107 |
| pr-tui | 34 |
| pr-server | 29 |
| pr-persistence | 21 |
| pr-mcp | 16 |
| pr-llm | 9 |
| E2E + Integration | 7 |

### Running tests

```bash
# All tests
cargo test --workspace

# Specific crate
cargo test -p pr-tools

# Specific test
cargo test -p pr-tools test_extract_emails_plain

# With output (for debugging)
cargo test -p pr-tools test_name -- --nocapture

# E2E tests (require network for live variant)
cargo test --test basic_research

# Ignored live tests (with real API)
PR_CDP_LIVE=1 cargo test -p pr-tools live_cdp -- --ignored
```

### MockLlm for offline tests

`tests/support/mock_llm.rs` provides a deterministic LLM mock for E2E tests without a real API:

```rust
let mock = MockLlm::new(vec![
    MockResponse::text("Response without tool calls"),
]);
```

---

## Adding a new tool

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

### 4. Classify for parallelism

`crates/agent/src/tool_executor.rs`:
- Read-only tools → `parallel_safe`
- Write/state tools → `sequential_only`

### 5. Tests

```bash
cargo test -p pr-tools my_tool
```

---

## Adding a new search backend

1. Add config in `crates/core/src/config.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MySearchConfig {
    pub api_key: String,
}
```

2. Implement search in `crates/tools/src/search.rs`:
```rust
async fn search_my_backend(&self, query: &str, limit: u32) -> Vec<SearchResult> {
    // HTTP request to API
}
```

3. Add to `search()` dispatch and hybrid/smart modes.

---

## Architectural principles

1. **`core` depends on nothing** — fundamental types
2. **Tools return `ToolOutput`** — do not propagate `Err` to the model
3. **Errors → model** — tool failures are returned as tool results, do not interrupt the loop
4. **Char-boundary safe** — all string slices check UTF-8 boundaries
5. **Graceful degradation** — network errors are logged, do not crash agents
6. **Config backward compatibility** — new sections with `#[serde(default)]`

---

## Debugging

```bash
# Verbose logs
RUST_LOG=debug ./target/release/parallel-research run "..."

# Specific module only
RUST_LOG=pr_agent=debug,pr_tools=trace ./target/release/parallel-research run "..."

# Backtrace on panic
RUST_BACKTRACE=1 ./target/release/parallel-research run "..."
```

---

## CI/CD

Recommended pipeline:

```yaml
# .github/workflows/ci.yml
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --workspace
      - run: cargo clippy --workspace -- -D warnings

  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo build --release
      - uses: actions/upload-artifact@v4
        with:
          name: parallel-research
          path: target/release/parallel-research
```

---

## Performance

- **Context compaction** prevents context overflow
- **Tool result truncation** limits large outputs
- **Smart parallelism** speeds up read-only operations
- **SQLite WAL mode** for concurrent access
- **Frozen memory snapshot** preserves LLM prefix cache

---

## Known limitations

- LinkedIn requires proxies/cookies (HTTP 999 anti-bot)
- 2GIS/Google Maps require API keys for stability
- PDF/DOCX export requires pandoc
- Vision model requires `PARALLEL_VISION_API_KEY`