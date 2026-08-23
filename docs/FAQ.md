# Frequently Asked Questions

Common questions about Fathom agent behavior, configuration, data management, computer use, and troubleshooting.

---

## Table of Contents

- [Agent Behavior](#agent-behavior)
- [Configuration](#configuration)
- [Data & Memory](#data--memory)
- [Computer Use](#computer-use)
- [Troubleshooting](#troubleshooting)

---

## Agent Behavior

### Why did my agent loop / get stuck?

Fathom detects "doom loops" -- when an agent retries the same tool call with identical arguments 3 or more times without progress. On the first detection the agent is nudged with a warning to try a different approach. On the second consecutive detection the agent is stopped to prevent exhausting the token budget.

Additionally, stall detection monitors whether an agent is making observable progress (new tool calls, messages, state changes):

- After `stall_warn_seconds` (default: 450) of no progress, a warning is emitted.
- After `stall_kill_seconds` (default: 1200), the agent is hard-cancelled.

To tune these thresholds:

```toml
[agent]
stall_warn_seconds = 600    # warn after 10 minutes of inactivity
stall_kill_seconds = 1800   # hard-cancel after 30 minutes
```

Set either to `0` to disable that gate. If the agent seems stuck on a specific tool error, check whether `[context] tool_output_max_bytes` (default: 50000) is truncating the error output so the model cannot diagnose the failure.

### How do I reduce token usage?

Several mechanisms control token consumption:

- **`fast_model`** -- set a cheap model (e.g. a flash variant) for auxiliary calls like entity extraction, memory classification, and reranking. The main `model` handles only planning and synthesis:

  ```toml
  [llm]
  model = "deepseek-chat"
  fast_model = "deepseek-chat"  # cheaper model for side-tasks
  ```

- **`compact_threshold`** -- triggers context compaction earlier. At `0.30`, compression starts when 30% of the context window is used (default: `0.50`):

  ```toml
  [context]
  compact_threshold = 0.30
  ```

- **`session_token_limit`** -- sets a hard cap on total tokens per session. Agents stop when the budget is exhausted:

  ```toml
  [agent]
  session_token_limit = 100000
  ```

- **`max_agents` and `max_depth`** -- limit the number of sub-agents spawned. Fewer agents means fewer LLM calls:

  ```toml
  [agent]
  max_agents = 10
  max_depth = 1
  ```

- **`temperature`** -- lower values (0.1--0.3) produce shorter, more focused outputs, reducing output token usage.

### Why was my tool call rejected?

Tools can be blocked at multiple levels:

1. **Approval gate** -- tools listed in `approval_tools` (default: `["save_contacts", "git_push"]`) require explicit operator approval before execution. In the TUI, press `y` or `n`. Via the HTTP API, call `POST /sessions/:id/approve`. If no operator responds within `approval_timeout_seconds` (default: 300), the `approval_fallback` verdict applies (`allow` or `deny`):

   ```toml
   [agent]
   approval_tools = ["save_contacts", "git_push"]
   approval_fallback = "deny"
   approval_timeout_seconds = 60
   ```

2. **Deny-list** -- `deny_tools` blocks tools by role:

   ```toml
   [agent]
   [agent.deny_tools]
   researcher = ["shell", "save_contacts"]
   ```

3. **Governance policy** -- when `FATHOM_GOVERNANCE_ENABLED=true`, the policy engine evaluates each tool invocation against the policy rules. An unmatched rule fails closed (deny). See [GOVERNANCE.md](GOVERNANCE.md).

4. **Lifecycle hooks** -- a `PreToolUse` hook can return `{"verdict": "deny"}` to block specific tool calls.

### How does Goal Mode work?

Goal Mode (`replan_rounds` in `[agent]`, default: 1) is a quality assurance loop that runs after the initial fan-out of sub-agents completes:

1. An LLM judge reviews every collected finding against the original goal.
2. If concrete gaps remain (e.g. missing data on a subtopic, incomplete contact details), the judge proposes up to 3 gap-filling subtasks.
3. A replan round launches new sub-agents to cover the gaps.
4. This repeats up to `replan_rounds` times.

Set `replan_rounds = 0` to disable Goal Mode entirely. Goal Mode is especially valuable for lead-generation queries where partial results are the norm.

---

## Configuration

### Can I use multiple LLM providers?

Yes. Use `role_models` to route different agent roles to different models:

```toml
[llm]
provider = "deepseek"
base_url = "https://api.deepseek.com"
api_key = "sk-..."
model = "deepseek-chat"

[agent]
[agent.role_models]
analyst = "deepseek-reasoner"
researcher = "deepseek-chat"
```

All models must be reachable from the same `base_url` provider endpoint. The `role_models` map overrides the main `[llm] model` on a per-role basis.

You can also use `fast_model` for a two-tier architecture: the main model handles planning and synthesis, while the fast model handles high-volume auxiliary calls (extraction, classification, reranking):

```toml
[llm]
model = "deepseek-reasoner"
fast_model = "deepseek-chat"
```

### How do I add a new search backend?

Add the provider's API key in a `[search.<provider>]` sub-section. Supported providers:

| Provider | Config Key | API Key Required |
|----------|-----------|-----------------|
| LinkUp | `[search.linkup]` | Yes |
| Exa | `[search.exa]` | Yes |
| Tavily | `[search.tavily]` | Yes |
| Serper | `[search.serper]` | Yes |
| Brave | `[search.brave]` | Yes |
| Parallel | `[search.parallel]` | Yes |
| DuckDuckGo | *(built-in)* | No |

Example:

```toml
[search]
backend = "hybrid"

[search.exa]
api_key = "your-exa-key"

[search.tavily]
api_key = "your-tavily-key"
```

### What is the difference between hybrid and smart mode?

- **`hybrid`** -- iterates through configured backends in order (LinkUp, Exa, Tavily, Serper, Brave, Parallel, DuckDuckGo) and returns results from the first one that returns non-empty results. This is a fallback chain: if your primary provider is rate-limited or down, the system falls through to the next configured provider.

- **`smart`** -- runs all configured backends simultaneously, deduplicates results by URL, and merges rankings via Reciprocal Rank Fusion (RRF). This yields the broadest result set but consumes quota from every provider on every query. Use `smart` for high-value research where completeness matters more than cost.

### Where is my config file?

The default location is `~/.fathom/config.toml`. Override it with the `PR_CONFIG` environment variable:

```bash
PR_CONFIG=/path/to/other/config.toml fathom run "query"
```

When `PR_CONFIG` is set, the default `~/.fathom/config.toml` is ignored entirely.

Individual values can be modified from the CLI:

```bash
fathom config set llm.api_key "sk-new-key"
fathom config set search.backend smart
fathom config set agent.max_depth 3
```

---

## Data & Memory

### Where does Fathom store data?

| Path | Content | Override |
|------|---------|---------|
| `~/.fathom/config.toml` | Configuration | `PR_CONFIG` |
| `~/.fathom/memory.db` | Semantic memory database | `PR_MEMORY_DB` or `[memory] db_path` |
| `~/.fathom/jobs.db` | Durable jobs registry | `PR_JOBS_DB` |
| `~/.fathom/jobs/` | Per-job workspaces | `PR_JOBS_DIR` |
| `~/.fathom/sessions/` | Session history and replays | -- |
| `~/.fathom/profiles/` | Persona profile files | -- |
| `<output_dir>/` | Research output per session | `PR_OUTPUT_DIR` or `[output] dir` |
| `./contacts.db` | Contact database | `[contacts] db_path` |

### How does memory deduplication work?

When new facts are ingested, the `absorb` pipeline runs automatically:

1. **Deduplication** -- the pipeline checks if a semantically similar fact already exists using hybrid search (BM25 + vector similarity).
2. **Linking** -- the new fact is connected to related existing facts via supersession chains.
3. **Classification** -- if `[memory] llm_classify = true` (default), the fast LLM assigns categories and confidence scores.
4. **Storage** -- the fact is written to SQLite with its embedding vector and metadata.

Facts are append-only with versioning via `supersedes` edges. Older facts are not deleted but superseded by newer, more accurate versions.

### Can I export/import memory?

The memory database is a standard SQLite file. Export and import using standard SQLite tools:

```bash
# Export all facts
sqlite3 ~/.fathom/memory.db "SELECT * FROM facts;" > memory-export.csv

# Full database copy
cp ~/.fathom/memory.db /backup/memory-backup.db

# Import via direct SQLite manipulation
sqlite3 new-memory.db < schema.sql
sqlite3 new-memory.db ".import data.csv facts"
```

There is no dedicated export/import CLI command. The database schema is documented in [MEMORY-KB.md](MEMORY-KB.md).

### How do I clear session history?

Session history is stored in `~/.fathom/sessions/`. To clear it:

```bash
# Remove all session history
rm -rf ~/.fathom/sessions/

# Or archive it first
tar czf /backup/sessions-$(date +%Y%m%d).tar.gz ~/.fathom/sessions/
rm -rf ~/.fathom/sessions/
```

To clear a specific session via the API:

```bash
curl -X DELETE http://localhost:8080/api/v1/sessions/<SESSION_ID>
```

This cancels a running session. Completed session records persist in the database.

---

## Computer Use

### How do I set up computer use?

Computer use requires the Playwright computer service running separately:

```bash
cd apps/computer
npm install
npm start
```

This starts the Playwright service on `http://127.0.0.1:8765`. Fathom connects to it automatically when computer-use tools are invoked.

For Docker-based per-agent isolation, set the Docker supervisor environment variables:

```bash
export COMPUTER_TOKEN=your-shared-secret
export COMPUTER_IMAGE=fathom/computer:latest
export COMPUTER_NETWORK=fathom-computer
export COMPUTER_BASE_PORT=19000
```

See [COMPUTER-USE.md](COMPUTER-USE.md) for the full protocol and container lifecycle details.

### Why cannot Fathom access localhost?

Fathom's browser and computer-use tools enforce SSRF (Server-Side Request Forgery) protection by default. Requests to these targets are blocked:

- `localhost`, `127.0.0.1`, `::1`
- Private IP ranges (`10.x`, `172.16-31.x`, `192.168.x`)
- Link-local addresses (`169.254.x.x`)
- Cloud metadata endpoints (`169.254.169.254`)
- Multicast addresses

This prevents agents from accessing internal services, cloud metadata, or local databases through browser automation.

### How do I test with loopback?

For development and testing only:

```bash
export COMPUTER_ALLOW_PRIVATE_HOSTS=true
```

**Never set this in production.** It disables a critical security boundary. The hard deny for cloud-metadata endpoints (169.254.169.254) and multicast addresses is never bypassed, even with this flag set.

---

## Troubleshooting

### Fathom will not build

**Symptom:**

```
error[E0658]: use of unstable library feature
```

or compilation fails with missing dependencies.

**Causes and fixes:**

- **Rust version too old.** Fathom requires Rust 1.97+. Update with `rustup update`.
- **Missing system libraries.** Install build essentials:

  ```bash
  # Debian/Ubuntu
  sudo apt-get install build-essential pkg-config libssl-dev

  # macOS
  xcode-select --install
  ```

- **Out of memory.** `cargo build --release` can consume several GB. Use `cargo build` (debug) for faster iteration, or add swap space.

### LLM returns errors

**Symptom:**

```
LLM error: timeout after 300s
API error 429: Too Many Requests
```

**Fixes:**

1. Verify `api_key` is set and valid in `~/.fathom/config.toml` under `[llm]`.
2. Verify `base_url` points to the correct provider endpoint.
3. Verify the `model` name is available on the configured provider.
4. For 429 errors, wait for the cooldown period (Fathom retries automatically up to 3 times with exponential backoff).
5. Use `fast_model` to route high-volume auxiliary calls to a cheaper model with higher rate limits.

### Search returns no results

**Symptom:**

Agents report empty search results or skip search entirely.

**Fixes:**

1. Check that `[search] backend` is set to a valid value (`hybrid`, `smart`, or a specific provider name).
2. Verify API keys are set for the configured providers in `[search.<provider>]` sub-sections.
3. For `hybrid` mode, only the first configured provider with results is used. If your primary provider's key is invalid, results fall through to the next one. Check which provider is actually being hit.
4. For `duckduckgo`, no API key is required.
5. Set `RUST_LOG=debug` to see which search backend is being called and what responses are returned.

### TUI is blank or crashes

**Symptom:**

The terminal interface shows a blank screen, garbled output, or crashes on startup.

**Fixes:**

1. **Terminal compatibility.** The TUI requires a terminal with at least 80x24 characters. Resize your terminal window.
2. **Terminal emulator.** Some terminal emulators have incomplete support for the rendering library. Try a different emulator (e.g., kitty, alacritty, iTerm2, Windows Terminal).
3. **SSH sessions.** Ensure `TERM` is set correctly (e.g., `xterm-256color`). Fallback rendering may be needed for limited terminals.
4. **GPU rendering.** Some terminals disable GPU acceleration by default. Enable it if the TUI feels laggy.
5. **Use the HTTP API instead.** If the TUI is incompatible with your environment, use `fathom serve` and interact via the HTTP API or the embedded dashboard at `http://localhost:8080/dashboard`.

---

## Related

- [INSTALLATION.md](INSTALLATION.md) -- build instructions and requirements
- [CONFIGURATION.md](CONFIGURATION.md) -- full configuration reference
- [TROUBLESHOOTING.md](TROUBLESHOOTING.md) -- extended troubleshooting guide
- [USAGE.md](USAGE.md) -- CLI commands and options
- [COMPUTER-USE.md](COMPUTER-USE.md) -- browser automation setup
- [MEMORY-KB.md](MEMORY-KB.md) -- semantic memory design
- [GOVERNANCE.md](GOVERNANCE.md) -- policy engine and access control
