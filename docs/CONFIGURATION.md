# Configuration

The config is stored in `~/.fathom/config.toml` (TOML). All sections are optional — missing fields take default values. Old configs (without new sections) load without errors.

The configuration file is the single source of truth for the entire system. It is loaded once at startup by `AppConfig::load()`, which reads the path resolved from the `PR_CONFIG` environment variable (if set) or `~/.fathom/config.toml` by default. Every config struct uses `#[serde(default)]` on all fields, so new sections and keys are always backward-compatible — older configs missing newer sections (e.g. `[memory]`, `[[mcp.servers]]`, `[[hooks]]`) load without errors and use Rust-level defaults.

The config is broadly divided into: LLM provider routing, agent orchestration parameters, search backends, context-management budgets, output & export, multi-channel notifications, contact database, CRM sync, governance policy engine, credentials vault, long-term semantic memory, MCP tool servers, lifecycle hooks, computer use (browser/Playwright), personality profiles, and HTTP server settings. This reflects Fathom's positioning as a **universal autonomous AI worker** — a virtual AI employee capable of research, outreach, code, and computer use.

---

## Full Example

```toml
# ~/.fathom/config.toml

# ─────────────────────────────────────────────
# LLM Provider
# ─────────────────────────────────────────────
[llm]
provider = "deepseek"
base_url = "https://api.deepseek.com"
api_key = "sk-your-key"
model = "deepseek-v4-flash"
max_tokens = 8192
temperature = 0.7

# ─────────────────────────────────────────────
# Agents
# ─────────────────────────────────────────────
[agent]
max_depth = 2              # Sub-agent nesting depth
max_agents = 20            # Max agents per session
max_iterations = 50        # Max iterations per agent
timeout_seconds = 600      # Timeout
use_multiprocess = false   # Multi-process isolation

# Goal Mode: LLM judge compares result against goal and launches
# gap-filling rounds (0 = disable)
replan_rounds = 1

# Control plane: side-effect tools wait for operator approval
# (TUI: y/n, HTTP: POST /sessions/:id/approve)
approval_tools = ["save_contacts", "git_push"]
approval_fallback = "allow"       # verdict without operator: allow|deny
approval_timeout_seconds = 300    # wait for decision before fallback

# ─────────────────────────────────────────────
# Search
# ─────────────────────────────────────────────
[search]
backend = "hybrid"         # linkup|exa|tavily|serper|brave|parallel|duckduckgo|hybrid|smart

[search.linkup]
api_key = "..."
[search.exa]
api_key = "..."
[search.tavily]
api_key = "..."
[search.serper]
api_key = "..."
[search.brave]
api_key = "..."
[search.parallel]
api_key = "..."

# ─────────────────────────────────────────────
# Context Management
# ─────────────────────────────────────────────
[context]
context_window = 128000        # Context window (tokens)
compact_threshold = 0.50       # Compression trigger (fraction of window)
tool_output_max_bytes = 50000  # Tool output limit (bytes)
tool_output_max_lines = 2000   # Tool output limit (lines)
turn_budget_bytes = 200000     # Per-turn budget (bytes)

# ─────────────────────────────────────────────
# Output
# ─────────────────────────────────────────────
[output]
dir = "./research-output"

# ─────────────────────────────────────────────
# Export Results
# ─────────────────────────────────────────────
[export]
format = "html"            # pdf|html|json|docx (pdf/docx require pandoc)

# ─────────────────────────────────────────────
# Notifications
# ─────────────────────────────────────────────
[notifications]
webhook_url = ""           # JSON POST on completion
email_to = ""
email_from = ""
smtp_host = ""
smtp_port = 587
smtp_username = ""
smtp_password = ""
telegram_bot_token = ""
telegram_chat_id = ""

# ─────────────────────────────────────────────
# Contacts Database
# ─────────────────────────────────────────────
[contacts]
db_path = "./contacts.db"  # SQLite path
pg_url = ""                # PostgreSQL URL (optional)

# ─────────────────────────────────────────────
# CRM Sync
# ─────────────────────────────────────────────
[crm]
provider = ""              # amocrm|bitrix24|hubspot (empty = disabled)
domain = ""                # Domain (amoCRM/Bitrix24)
api_key = ""

# ─────────────────────────────────────────────
# Long-term Semantic Memory
# ─────────────────────────────────────────────
[memory]
enabled = true             # master switch
db_path = ""               # empty = ~/.fathom/memory.db
embeddings = "auto"        # auto|openai|tfidf
embedding_base_url = ""    # empty = llm.base_url
embedding_api_key = ""     # empty = llm.api_key
embedding_model = "text-embedding-3-small"
semantic_weight = 0.7      # w in hybrid score (0=BM25, 1=vectors)
top_k = 5                  # search/digest results count
min_score = 0.25           # relevance threshold for results
temporal_decay = 0.01      # freshness decay per day
auto_digest = true         # inject digest into top-agent prompts
llm_classify = true        # LLM classification of facts in absorb
rerank = false             # LLM-reranking of search results as second pass

# ─────────────────────────────────────────────
# MCP Servers
# ─────────────────────────────────────────────
[[mcp.servers]]
name = "web-search"
transport = "stdio"        # stdio|http
command = "npx"
args = ["-y", "@modelcontextprotocol/server-web-search"]

[[mcp.servers]]
name = "remote-tools"
transport = "http"
url = "https://mcp.example.com"
```

---

## Sections

### `[llm]`

The LLM section controls which provider and model drive the agent fleet. All agents in a session share the same LLM endpoint unless `role_models` (see `[agent]`) overrides per role. The provider is resolved by name at startup; supported providers include DeepSeek, OpenAI-compatible endpoints, and any provider that exposes a standard chat-completions API.

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `provider` | string | `"deepseek"` | Provider name |
| `base_url` | string | `"https://api.deepseek.com"` | API endpoint |
| `api_key` | string | `""` | **Required** for operation |
| `model` | string | `"deepseek-chat"` | Model |
| `fast_model` | string | `""` | Cheap model for auxiliary calls (extract, memory classification, rerank). Empty = uses `model` |
| `max_tokens` | u32 | `8192` | Max response tokens |
| `temperature` | f32 | `0.7` | Generation temperature |

**Multi-model routing.** The `fast_model` field enables a two-tier architecture. The main `model` (e.g. a strong reasoning model) is used for planning, report writing, and complex multi-step tasks. The `fast_model` (e.g. a cheap flash model) handles high-volume auxiliary calls: entity extraction from search results, memory fact classification during `absorb`, LLM-based reranking of search results, and other side-tasks that don't require deep reasoning. This separation dramatically reduces costs and latency without sacrificing quality on the critical path. When `fast_model` is empty, the main `model` is used for everything.

**`temperature`** controls the randomness of generation. Lower values (0.1–0.3) are recommended for extraction, classification, and structured output tasks. Higher values (0.7–0.9) are better for creative synthesis, report writing, and exploration. The `model` and `fast_model` can each be overridden on a per-role basis via `role_models` in `[agent]`.

### `[agent]`

The agent section governs the orchestration topology: how many agents can be spawned, how deeply they nest, how long they run, and what safety gates are in place.

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `max_depth` | u32 | `2` | Max agent nesting depth |
| `max_agents` | u32 | `20` | Max agents per session |
| `max_iterations` | u32 | `50` | Max LLM iterations per agent |
| `timeout_seconds` | u64 | `600` | Session timeout |
| `use_multiprocess` | bool | `false` | Isolate agents in separate processes |
| `max_concurrent_children` | u32 | `4` | Concurrent children of one parent |
| `stall_warn_seconds` | u64 | `450` | Stall warning when no progress (0 = off) |
| `stall_kill_seconds` | u64 | `1200` | Cancel agent when no progress (0 = off) |
| `session_token_limit` | u64 | `0` | Session token budget (0 = unlimited) |
| `replan_rounds` | u32 | `1` | Goal Mode: max gap-filling rounds after LLM judge (0 = off) |
| `approval_tools` | string[] | `["save_contacts", "git_push"]` | Tools requiring operator approval |
| `approval_fallback` | string | `"allow"` | Verdict without operator: `allow` \| `deny` |
| `approval_timeout_seconds` | u64 | `300` | Wait for operator decision before fallback (0 = immediate fallback) |
| `deny_tools` | map | `{}` | Tool deny-lists by role: `researcher = ["shell"]` |
| `role_models` | map | `{}` | Model per role: `analyst = "deepseek-reasoner"` |

**Nesting and concurrency.** `max_depth` controls how many levels of sub-agent spawning are allowed (root = 0, child = 1, grandchild = 2, …). `max_concurrent_children` limits how many children a single parent can spawn simultaneously — this prevents runaway parallelism from overwhelming the LLM backend or the search providers. `use_multiprocess` runs each agent in a separate OS process (via `std::process::Command`), providing stronger isolation and separate memory-space garbage collection at the cost of higher startup latency and IPC overhead.

**Stall detection.** The runtime tracks whether an agent is making observable progress (new tool calls, new messages, state changes). After `stall_warn_seconds` of no progress, a warning is emitted via the notifier. After `stall_kill_seconds`, the agent is hard-cancelled. Setting either to `0` disables the corresponding gate. This is especially useful for catching agents stuck in infinite loops or waiting on a non-responsive external API.

**Approval flow.** Tools listed in `approval_tools` are gated — they do not execute until an operator explicitly approves. In the TUI the operator is prompted with `y/n`; in the HTTP API the operator calls `POST /sessions/:id/approve`. If the operator does not respond within `approval_timeout_seconds`, the `approval_fallback` verdict applies (`allow` or `deny`). Setting `approval_timeout_seconds` to `0` applies the fallback immediately.

**Role-based tool deny-lists.** `deny_tools` is a map from role name (e.g. `coordinator`, `researcher`, `analyst`, `verifier`, `writer`) to a list of tool names that are blocked for that role. For example, an `analyst` role might be denied `save_contacts` and `git_push` to prevent accidental side effects during research. This is enforced at the tool-invocation boundary in the agent runtime.

**Role-based model overrides.** `role_models` maps role names to model identifiers. When set, agents assigned to that role use the specified model instead of the main `[llm] model`. This allows routing e.g. the `analyst` role to a powerful reasoning model while the `researcher` role uses a cheaper flash model, all from the same LLM provider endpoint.

**How `session_token_limit` works.** The budget is split equally among
tasks in each fan-out round (`per-agent cap`), and each agent
stops at a turn boundary when its cap is exhausted **or** the next
turn (estimated from current context) would exceed the cap. Children inherit
the remaining parent cap; when the cap is exhausted, `spawn_agent` refuses.
The check happens between turns, so one "expensive" turn with a large context
may overshoot the cap by its own size — with realistic budgets the overshoot
converges to zero. New tasks and replanning do not start when the total
budget is exhausted. With no limit (`0`) behavior is unchanged.

**The `PR_CONFIG` variable.** The config path can be overridden:
`PR_CONFIG=/path/to/config.toml fathom run ...` — convenient for
budget/one-off runs and tests without touching the main
`~/.fathom/config.toml`. Similarly, `PR_MEMORY_DB` specifies
the memory database file.

### `[search]`

The search section configures which web-search backends are available and how they are combined. The system supports multiple backends to provide redundancy, breadth, and cost flexibility.

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `backend` | string | `"hybrid"` | Search backend |

**`backend` values:**
- `linkup`, `exa`, `tavily`, `serper`, `brave`, `parallel`, `duckduckgo` — a single specific backend
- `hybrid` — first configured backend with results (order: linkup → exa → tavily → serper → brave → parallel → duckduckgo)
- `smart` — all configured backends in parallel, deduplication by URL, ranking (reciprocal rank fusion)

Sub-sections `[search.*]` contain `api_key` for each provider. DuckDuckGo does not require a key.

**Backend selection strategy.** A single-backend mode (`linkup`, `exa`, etc.) pins all searches to one provider. Use this when you have a preferred provider with a strong track record for your domain, or when you want to strictly control costs. 

**Hybrid mode** iterates through the configured backends in the defined order and returns results from the first one that returns non-empty results. This provides a simple fallback chain — if your primary provider (e.g. LinkUp) is rate-limited or down, the system automatically falls through to the next configured provider (e.g. Exa, then Tavily, etc.).

**Smart mode** runs all configured backends _simultaneously_ for every query, then merges the results via Reciprocal Rank Fusion (RRF). Duplicate URLs are collapsed, and the combined ranking reflects the consensus across all providers. This yields the broadest and most robust result set but consumes quota from every configured provider on every query. Smart mode is recommended for high-value research tasks where completeness matters more than cost.

### `[context]`

Context management controls how the system budgets tokens, truncates tool output, and triggers compaction to stay within the LLM's context window.

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `context_window` | u32 | `128000` | Context window size |
| `context_window_profile` | string | `"low"` | Window profile: `low` (conservative) \| `max` (optimistic) |
| `compact_threshold` | f32 | `0.50` | Window fraction for compression trigger |
| `tool_output_max_bytes` | u32 | `50000` | Tool output limit (bytes) |
| `tool_output_max_lines` | u32 | `2000` | Tool output limit (lines) |
| `turn_budget_bytes` | u32 | `200000` | Aggregate per-turn budget |

**Window profiling.** The `context_window_profile` field acts as a safe fallback when the actual LLM window size is unknown or the explicit `context_window` is `0`. A `low` profile asserts a conservative floor (e.g. 64K tokens), while `max` asserts a more optimistic floor (e.g. 128K). This is resolved by `resolve_window()` in `capability.rs`: if `context_window` is explicitly set to a positive value, that value is used directly; otherwise the profile's floor is applied.

**Compaction.** When the active context exceeds `compact_threshold * context_window` tokens, the runtime compresses the conversation history: older messages are summarized, redundant tool outputs are pruned, and the compacted representation is injected back into the context. This prevents the agent from hitting the model's hard window limit while preserving essential information.

**Tool output truncation.** Tool outputs are truncated at `tool_output_max_bytes` **or** `tool_output_max_lines`, whichever is reached first. This prevents runaway tool outputs (e.g. a full HTML page from a scraper) from consuming the entire context budget. The truncation is applied per tool call, not per tool — each invocation gets its own slice of the budget.

**Per-turn budget.** `turn_budget_bytes` limits the aggregate size of a single LLM call (system prompt + history + tool outputs + user message). If the next turn would exceed this budget, the system attempts compaction first; if compaction still doesn't fit, the turn is refused with an actionable error.

### `[output]`

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `dir` | string | `"./research-output"` | Output directory |

The output directory holds per-session result files: the final report, intermediate artifacts, exported contact files, and logs. The directory is created automatically if it does not exist. Each session creates a timestamped subdirectory within this path.

### `[export]`

Controls how the final research deliverables are exported. The export format applies to the final report that is generated at the end of a session.

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `format` | string | `"html"` | `pdf` \| `html` \| `json` \| `docx`. Unknown format → html. PDF/DOCX require pandoc. |

**Export formats.** `html` produces a self-contained HTML report with embedded CSS and inline images. `json` exports the structured research data (contacts, findings, sources) as a JSON document suitable for downstream processing or ingestion. `pdf` and `docx` require `pandoc` to be installed on the system path — the report is first rendered as HTML, then converted via pandoc with a LaTeX/PDF or DOCX template. When pandoc is not available and `pdf` or `docx` is requested, the system falls back to `html` and emits a warning.

### `[notifications]`

Multi-channel notification system for session completion events. Notifications are sent only when the corresponding field is non-empty, so you can enable any subset of channels independently.

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `webhook_url` | string | `""` | URL for JSON POST on completion |
| `email_to` | string | `""` | Email recipient |
| `email_from` | string | `""` | Sender (default: fathom@localhost) |
| `smtp_host` | string | `""` | SMTP server (default: localhost) |
| `smtp_port` | u16 | `587` | SMTP port |
| `smtp_username` | string | `""` | SMTP username |
| `smtp_password` | string | `""` | SMTP password |
| `telegram_bot_token` | string | `""` | Telegram bot token |
| `telegram_chat_id` | string | `""` | Chat ID for notifications |

Notifications are sent only if the corresponding field is non-empty.

**Webhook.** When `webhook_url` is set, the system sends an HTTP POST with a JSON body containing the session ID, status (completed / failed / timeout), summary text, and a link to the output directory. The webhook is ideal for integration with CI/CD pipelines, Slack bots via Zapier/n8n, or custom dashboards.

**Email.** When `email_to` and `smtp_host` are set, the system sends an HTML email notification. The `email_from` field defaults to `fathom@localhost` if left empty. SMTP authentication is used when `smtp_username` is non-empty. The email includes the session summary and a link to the export directory.

**Telegram.** When both `telegram_bot_token` and `telegram_chat_id` are set, the system sends a Telegram message via the Bot API. The bot token is the one obtained from [@BotFather](https://t.me/BotFather). The chat ID can be a user ID or a group chat ID (negative for groups). The message includes the session status and a brief summary.

### `[contacts]`

The contacts database stores all harvested or imported contact records. It supports two backends: SQLite (default) and PostgreSQL.

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `db_path` | string | `"./contacts.db"` | Path to SQLite contacts database |
| `pg_url` | string | `""` | PostgreSQL URL (non-empty → uses PG) |

**Backend selection.** When `pg_url` is non-empty, the system uses PostgreSQL. Otherwise it uses SQLite at `db_path`. The SQLite database is created automatically on first use. The PostgreSQL connection string follows the standard `postgresql://user:password@host:port/database` format.

**Contact schema.** Each contact record stores: name, title, company, email, phone, social profiles (LinkedIn, Twitter, GitHub, etc.), tags, notes, source (which research session produced it), and timestamps. The schema is managed by the `pr_persistence` crate and supports upsert-by-email deduplication.

### `[crm]`

CRM integration pushes collected contacts to external CRM platforms. This is optional and disabled by default.

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `provider` | string | `""` | `amocrm` \| `bitrix24` \| `hubspot` \| empty |
| `domain` | string | `""` | Domain/subdomain (amoCRM, Bitrix24) |
| `api_key` | string | `""` | API key/token |

**Provider notes.** For **amoCRM** the `domain` is your subdomain (e.g. `mycompany` in `mycompany.amocrm.ru`). The `api_key` is the integration token from Settings → API Access. For **Bitrix24** the `domain` is your portal URL (e.g. `mycompany.bitrix24.com`). The `api_key` is a webhook secret or OAuth token. For **HubSpot** the `domain` field is unused; the `api_key` is a private app access token. Contacts are pushed asynchronously — the CRM sync runs in the background after the session completes, and failures are logged but do not block the session.

### `[governance]`

Governance enforces a policy engine that evaluates every agent action against an allow/deny ruleset before execution. When enabled, the system checks each tool call against the configured policy and blocks violations. Requires `FATHOM_GOVERNANCE_ENABLED=true` to activate.

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `enabled` | bool | `false` | Master switch for the governance policy engine |
| `policy` | string | `""` | JSON string of allow/deny rules (see [GOVERNANCE.md](GOVERNANCE.md) for format) |

**Environment variables.** These fields can also be set via environment variables:

| Variable | Type | Description |
|----------|------|-------------|
| `FATHOM_GOVERNANCE_ENABLED` | bool | Enable governance policy engine (`true`/`false`) |
| `FATHOM_GOVERNANCE_POLICY` | JSON string | Inline JSON policy document with allow/deny rules |

**Policy format.** The `policy` field (or `FATHOM_GOVERNANCE_POLICY` env var) accepts a JSON document of the form:

```json
{"rules":[{"effect":"allow","tool":"browser.*","host":"example.com"},{"effect":"deny","tool":"browser.type","path":"/admin/*"}]}
```

When the policy is empty (default), governance still tracks actions but does not block them. See [GOVERNANCE.md](GOVERNANCE.md) for the full rule schema, audit log format, and best practices.

### `[credentials]`

The credentials vault provides encrypted storage for secrets (API keys, passwords, tokens) that agents can use during operation. All stored values are encrypted at rest using AES-256-GCM.

| Environment variable | Description |
|---------------------|-------------|
| `FATHOM_CREDENTIAL_KEY` | 32-byte AES-256-GCM encryption key for the credentials vault. Must be exactly 32 bytes when decoded, encoded as 64 hex characters or base64. |

**Key derivation.** The `FATHOM_CREDENTIAL_KEY` is used directly as the AES-256-GCM key (after hex/base64 decoding). A random 12-byte nonce is generated per encryption operation. Encrypted blobs are stored in SQLite alongside metadata (label, scope, timestamps). The vault is accessible via the HTTP API at `/api/v1/credentials` — list responses never include plaintext values.

**Security.** Never commit the key to version control. In production, inject it via a secrets manager or environment variable. The key is read once at server startup; changing it requires a server restart and re-encryption of all stored credentials.

### `[memory]`

Long-term semantic memory (see [MEMORY-KB.md](MEMORY-KB.md) for the full design). The memory subsystem stores self-contained facts in a SQLite database with hybrid (vector + BM25) search, append-only supersession chains, and an `absorb` pipeline that deduplicates and links new facts against existing ones. This is inspired by the mem0/Memora approach to persistent agent memory.

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `enabled` | bool | `true` | Master switch for the subsystem |
| `db_path` | string | `""` | Path to SQLite; empty = `~/.fathom/memory.db` |
| `embeddings` | string | `"auto"` | `auto` \| `openai` \| `tfidf` |
| `embedding_base_url` | string | `""` | Embedding endpoint; empty = `llm.base_url` |
| `embedding_api_key` | string | `""` | Embedding key; empty = `llm.api_key` |
| `embedding_model` | string | `"text-embedding-3-small"` | Embedding model (OpenAI backend) |
| `semantic_weight` | f32 | `0.7` | Vector weight in hybrid score (0–1) |
| `top_k` | u32 | `5` | Number of search/digest results |
| `min_score` | f32 | `0.25` | Minimum score for result inclusion |
| `temporal_decay` | f32 | `0.01` | Linear freshness decay per day (0 = off) |
| `auto_digest` | bool | `true` | Inject digest into top-agent prompts |
| `llm_classify` | bool | `true` | LLM classification of facts in absorb |
| `rerank` | bool | `false` | LLM-reranking of search results as second pass |
| `gc_ttl_days` | u32 | `30` | GC: age (days) after which untouched run facts are archived |
| `gc_compact_above` | u32 | `200` | GC: active-row threshold in scope group for N→1 compaction |
| `gc_confidence_decay_rate` | f64 | `0.02` | GC: daily confidence decay for unused facts |
| `gc_confidence_threshold` | f64 | `0.15` | GC: minimum confidence; below this, facts are archived |

**Embedding backend selection.** The `embeddings` field selects the embedding strategy:
- `auto` — attempts to use the LLM provider's embedding endpoint (OpenAI-compatible); falls back to TF-IDF if unavailable.
- `openai` — explicitly uses the OpenAI-compatible embedding API at `embedding_base_url` with `embedding_model`.
- `tfidf` — pure TF-IDF vectorization (no external API call, works offline, suitable for small-to-medium memory stores).

**Hybrid search formula.** Memory search combines BM25 keyword scoring with dense vector cosine similarity: `score = w * vector_sim + (1 - w) * bm25_score`, where `w = semantic_weight`. A weight of `0.7` means 70% of the score comes from semantic similarity and 30% from keyword matching. Set `semantic_weight = 1.0` for pure vector search, or `0.0` for pure BM25.

**Temporal decay.** `temporal_decay` applies a linear freshness penalty: each day a fact ages, its score is reduced by `decay * days_since_creation`. This ensures that newer, more relevant facts are preferred over older ones. Set to `0` to disable temporal decay entirely.

**The absorb pipeline.** When new facts are ingested, the `absorb` pipeline performs: (1) **deduplication** — checks if a semantically similar fact already exists; (2) **linking** — connects the new fact to related existing facts via supersession chains; (3) **classification** — if `llm_classify` is true, the fast LLM assigns categories and confidence scores; (4) **storage** — the fact is written to the SQLite store with its embedding vector and metadata.

**Auto-digest.** When `auto_digest` is true, the memory subsystem automatically injects a digest of the most relevant facts into the top-level agent's system prompt at the start of each session. This provides the agent with persistent context across sessions without manual retrieval.

**Garbage collection.** The GC subsystem runs periodically: `gc_ttl_days` controls how old a fact must be (without being accessed) before it is eligible for archival. `gc_compact_above` triggers N→1 compaction when a scope group has more than this many active rows — older, lower-confidence facts are merged into a single summary row. `gc_confidence_decay_rate` and `gc_confidence_threshold` govern the confidence-scoring model: each unused day reduces confidence by the decay rate, and facts below the threshold are archived.

### `[[mcp.servers]]`

Array of MCP (Model Context Protocol) servers. Each server extends the agent's toolset with external capabilities — databases, APIs, file systems, or custom logic. The servers are connected at startup and their tools are registered into the global tool registry alongside the built-in tools.

| Field | Type | Description |
|------|-----|----------|
| `name` | string | Server name |
| `transport` | string | `stdio` \| `http` |
| `command` | string? | Command (for stdio) |
| `args` | string[] | Arguments (for stdio) |
| `url` | string? | URL (for http) |

**Transport types.** `stdio` servers are spawned as subprocesses — the system runs `command args` and communicates with the server over its stdin/stdout using JSON-RPC. This is ideal for local tools (e.g. `npx @modelcontextprotocol/server-web-search`). `http` servers are reached over the network at the specified `url` using JSON-RPC over HTTP POST. This is suitable for remote or shared tool servers. Both transport types are connected concurrently at startup; a failed connection logs a warning but does not prevent the session from starting.

---

### `[[hooks]]`

Lifecycle hooks extend the system with custom subprocess callbacks at specific points in the agent execution lifecycle. This follows the fleet/E3 pattern: a hook is a short-lived command that receives a JSON payload on stdin and returns a JSON verdict on stdout.

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `event` | string | — | `PreToolUse` \| `PostToolUse` \| `Stop` |
| `command` | string | — | Command to run |
| `args` | string[] | `[]` | Command arguments |
| `tool` | string | `""` | Optional: only fire for this tool name (Pre/PostToolUse) |
| `timeout_ms` | u64 | `5000` | Hook timeout in milliseconds |

**Events.** `PreToolUse` fires before a tool is invoked — the hook can inspect the tool name and arguments and return `allow` or `deny` to gate the call. `PostToolUse` fires after a tool completes — the hook receives the tool output and can log, transform, or alert on it. `Stop` fires when an agent is being stopped (completion, cancellation, or error) — useful for cleanup, metrics emission, or logging final state.

**Tool filtering.** When `tool` is non-empty, the hook only fires for invocations of that specific tool (e.g. `tool = "save_contacts"`). When empty, the hook fires for every tool. This allows fine-grained control: a `PreToolUse` hook can deny `shell` for the `researcher` role while allowing it for `coordinator`.

**Timeout.** Every hook has a `timeout_ms` (default 5 seconds). If the hook does not respond within this window, it is killed and its verdict is treated as `allow` (for `PreToolUse`) or ignored (for `PostToolUse`/`Stop`). This prevents a misbehaving hook from blocking the agent indefinitely.

**JSON payload format.** The hook receives a JSON object on stdin with: `event` (string), `session_id` (string), `agent_id` (string), `role` (string), `tool` (string, only for tool events), `args` (object, only for tool events), `output` (string, only for PostToolUse/Stop). The hook replies on stdout with a JSON object: for `PreToolUse` the reply must include `verdict` (`"allow"` or `"deny"`); for other events any JSON is accepted but ignored.

**Example:**
```toml
[[hooks]]
event = "PreToolUse"
command = "/usr/local/bin/audit-tool"
tool = "shell"
timeout_ms = 3000

[[hooks]]
event = "Stop"
command = "/usr/local/bin/log-metrics"
args = ["--session-end"]
```

---

### `[computer]`

Computer use gives agents a browser (via Playwright) they can drive — navigate, click, type, screenshot, inspect accessibility trees. The computer service is an optional loopback Playwright service; with Docker, Fathom can provision one isolated computer per agent. Browser egress rejects localhost, private, link-local, multicast, and cloud-metadata targets by default.

| Environment variable | Default | Description |
|----------------------|---------|-------------|
| `FATHOM_COMPUTER_SERVICE_URL` | `http://127.0.0.1:8765` | URL of the Playwright computer service, used by the server relay |
| `COMPUTER_SERVICE_URL` | `http://127.0.0.1:8765` | Legacy alias for the same computer service URL |
| `COMPUTER_TOKEN` | *(auto-generated)* | Authentication token for computer service requests |
| `COMPUTER_IMAGE` | `ghcr.io/fathom/computer:latest` | Docker image for per-agent computer containers |
| `COMPUTER_NETWORK` | `fathom-computer` | Docker network for computer containers |
| `COMPUTER_BASE_PORT` | `9200` | Base port for per-agent loopback ports; agent `i` gets `base_port + i` |
| `COMPUTER_ALLOW_PRIVATE_HOSTS` | `false` | Allow localhost/private targets (development only; never bypasses metadata/multicast denies) |

**Service URL selection.** `FATHOM_COMPUTER_SERVICE_URL` is the canonical variable used by the server relay. `COMPUTER_SERVICE_URL` is a legacy alias — set both if you need compatibility with older tooling; the canonical one wins when both are set. When neither is set, the server defaults to `http://127.0.0.1:8765`.

**Authentication.** The `COMPUTER_TOKEN` shared secret authenticates relay requests to the computer service. When the Docker supervisor is configured, the same token is injected into provisioned per-agent containers. Without a token configured, the supervisor is unavailable and the server falls back to the single-service URL mode.

**Docker per-agent computers.** Set `COMPUTER_IMAGE`, `COMPUTER_NETWORK`, `COMPUTER_TOKEN`, and `COMPUTER_BASE_PORT` to let the supervisor provision one isolated container per agent, each with its own workspace/profile volumes, loopback ports (`base_port + agent_index`), restrictive capabilities, and health checks. Containers are stopped and removed on agent completion or cancellation.

**Egress policy.** `COMPUTER_ALLOW_PRIVATE_HOSTS=true` permits localhost and private ranges for development. It **never** bypasses hard denies for cloud-metadata endpoints (e.g. 169.254.169.254) or multicast addresses. Keep it `false` in production.

See [COMPUTER-USE.md](COMPUTER-USE.md) for the full protocol and container lifecycle details.

### `[server]`

HTTP API server settings for `fathom serve`. When bound to a non-loopback address, API keys are **required**.

| Environment variable | Default | Description |
|----------------------|---------|-------------|
| `FATHOM_API_KEYS` | *(unset = open access)* | Comma-separated API keys for non-loopback binds; all `/api/v1/*` requests require a key when set |
| `FATHOM_RATE_LIMIT` | `120` | Per-client rate limit, requests per minute |

**Authentication.** When `FATHOM_API_KEYS` (comma-separated) is set, every `/api/v1/*` request must present a valid key via `X-API-Key` header or Bearer token. Each configured key is registered with a human-readable name derived from a hash, which is used for rate-limiting and logging. Public endpoints (`/health`, `/metrics`, `/dashboard`) stay open, but the dashboard fetches data only through protected endpoints.

**Non-loopback binds.** `fathom serve --host 0.0.0.0` (or any non-loopback address) is **rejected at startup** unless `FATHOM_API_KEYS` is set. Loopback binds default to open access; set `FATHOM_API_KEYS` to enable auth locally too.

**Rate limiting.** Sliding-window limit per client identity (the authenticated principal name, or the client IP when auth is disabled). Each client's window is tracked independently. Exceeding the limit returns `429 Too Many Requests`. See [HTTP-API.md](HTTP-API.md) for details.

---

### Profiles

Profiles (personas) are named TOML presets that tune the fleet for a class of tasks. A profile is a small declarative overlay applied on top of `~/.fathom/config.toml`:

- an extra system-prompt block injected into every agent;
- optional overrides for the main/fast model, temperature and depth;
- extra tools denied for every role.

Profiles live in `~/.fathom/profiles/<name>.toml`; three presets (`hunter`, `analyst`, `validator`) are built in and available without any files.

```bash
fathom profiles list
fathom run --profile hunter "find decision makers at Acme"
```

**Profile fields:**

| Field | Type | Description |
|------|-----|----------|
| `name` | string | Profile name |
| `description` | string | Human-readable description |
| `prompt` | string | Extra system-prompt block injected into every agent |
| `model` | string? | Override `[llm] model` (strong model) |
| `fast_model` | string? | Override `[llm] fast_model` (cheap model) |
| `temperature` | f32? | Override `[llm] temperature` |
| `max_depth` | u32? | Override `[agent] max_depth` |
| `max_agents` | u32? | Override `[agent] max_agents` |
| `max_iterations` | u32? | Override `[agent] max_iterations` |
| `timeout_seconds` | u64? | Override `[agent] timeout_seconds` |
| `replan_rounds` | u32? | Override `[agent] replan_rounds` (0 = disable replanning) |
| `deny_tools` | string[] | Tools denied for every role (merged into `[agent] deny_tools`) |

**Built-in presets:**

- **`hunter`** — Aggressive lead harvesting: maximises verified contacts. Sets `max_agents = 6`, prioritises sources with people pages, and always runs `extract_contacts` with `enrich_entities` and `save_contacts`.
- **`analyst`** — Deep research & cross-checking, no side effects. Denies `save_contacts` and `git_push`. Every claim needs a source URL; conflicting sources get both cited. Focuses on companies, markets, numbers, and dates.
- **`validator`** — Verify and enrich already-collected contacts. Denies `spawn_agent` (no sub-agent creation). Works through the contact list one by one, using `verify_email` / `verify_phone` / `verify_social` tools.

**User-defined profiles** are stored as TOML files in `~/.fathom/profiles/`. User files override built-in presets with the same name. You can create a new profile template with:

```bash
fathom profiles new my-profile
```

Profiles are applied via `Profile::apply()` which overlays the profile's fields onto the loaded `AppConfig` in-place. The system prompt from the profile is injected into every agent's context at the start of each turn.

---

## CLI Config Management

The config file can be inspected and modified from the command line:

```bash
# Show the current configuration
fathom config show

# Set a single value by dotted key path
fathom config set llm.api_key "sk-new-key"
fathom config set agent.max_depth 3
fathom config set search.backend smart
```

The `config set` command parses the value as bool, integer, float, or string (in that order). The updated document is validated against the full `AppConfig` struct before being written, so unknown keys or values with the wrong type are rejected without touching the file. The command uses `set_config_value()` which reads the config, performs the nested key insertion, and writes the file atomically.

```bash
# Override the config path for a one-off run
PR_CONFIG=./scratch.toml fathom run "analyze competitor landscape"

# Override the memory database path
PR_MEMORY_DB=./scratch-memory.db fathom run "find contacts at Acme"
```

All new sections have `#[serde(default)]`. Configs from older versions (only `[llm]`, `[agent]`, `[search]`) load correctly — new fields get default values.