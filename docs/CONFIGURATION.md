# Configuration

The config is stored in `~/.parallel-research/config.toml` (TOML). All sections are optional — missing fields take default values. Old configs (without new sections) load without errors.

---

## Full Example

```toml
# ~/.parallel-research/config.toml

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
db_path = ""               # empty = ~/.parallel-research/memory.db
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

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `provider` | string | `"deepseek"` | Provider name |
| `base_url` | string | `"https://api.deepseek.com"` | API endpoint |
| `api_key` | string | `""` | **Required** for operation |
| `model` | string | `"deepseek-chat"` | Model |
| `fast_model` | string | `""` | Cheap model for auxiliary calls (extract, memory classification, rerank). Empty = uses `model` |
| `max_tokens` | u32 | `8192` | Max response tokens |
| `temperature` | f32 | `0.7` | Generation temperature |

### `[agent]`

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
`PR_CONFIG=/path/to/config.toml parallel-research run ...` — convenient for
budget/one-off runs and tests without touching the main
`~/.parallel-research/config.toml`. Similarly, `PR_MEMORY_DB` specifies
the memory database file.

### `[search]`

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `backend` | string | `"hybrid"` | Search backend |

**`backend` values:**
- `linkup`, `exa`, `tavily`, `serper`, `brave`, `parallel`, `duckduckgo` — a single specific backend
- `hybrid` — first configured backend with results (order: linkup → exa → tavily → serper → brave → parallel → duckduckgo)
- `smart` — all configured backends in parallel, deduplication by URL, ranking (reciprocal rank fusion)

Sub-sections `[search.*]` contain `api_key` for each provider. DuckDuckGo does not require a key.

### `[context]`

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `context_window` | u32 | `128000` | Context window size |
| `compact_threshold` | f32 | `0.50` | Window fraction for compression trigger |
| `tool_output_max_bytes` | u32 | `50000` | Tool output limit (bytes) |
| `tool_output_max_lines` | u32 | `2000` | Tool output limit (lines) |
| `turn_budget_bytes` | u32 | `200000` | Aggregate per-turn budget |

### `[output]`

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `dir` | string | `"./research-output"` | Output directory |

### `[export]`

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `format` | string | `"html"` | `pdf` \| `html` \| `json` \| `docx`. Unknown format → html. PDF/DOCX require pandoc. |

### `[notifications]`

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `webhook_url` | string | `""` | URL for JSON POST on completion |
| `email_to` | string | `""` | Email recipient |
| `email_from` | string | `""` | Sender (default: parallel-research@localhost) |
| `smtp_host` | string | `""` | SMTP server (default: localhost) |
| `smtp_port` | u16 | `587` | SMTP port |
| `smtp_username` | string | `""` | SMTP username |
| `smtp_password` | string | `""` | SMTP password |
| `telegram_bot_token` | string | `""` | Telegram bot token |
| `telegram_chat_id` | string | `""` | Chat ID for notifications |

Notifications are sent only if the corresponding field is non-empty.

### `[contacts]`

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `db_path` | string | `"./contacts.db"` | Path to SQLite contacts database |
| `pg_url` | string | `""` | PostgreSQL URL (non-empty → uses PG) |

### `[crm]`

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `provider` | string | `""` | `amocrm` \| `bitrix24` \| `hubspot` \| empty |
| `domain` | string | `""` | Domain/subdomain (amoCRM, Bitrix24) |
| `api_key` | string | `""` | API key/token |

### `[memory]`

Long-term semantic memory (see [MEMORY-KB.md](MEMORY-KB.md)).

| Field | Type | Default | Description |
|------|-----|---------|----------|
| `enabled` | bool | `true` | Master switch for the subsystem |
| `db_path` | string | `""` | Path to SQLite; empty = `~/.parallel-research/memory.db` |
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

### `[[mcp.servers]]`

Array of MCP servers:

| Field | Type | Description |
|------|-----|----------|
| `name` | string | Server name |
| `transport` | string | `stdio` \| `http` |
| `command` | string? | Command (for stdio) |
| `args` | string[] | Arguments (for stdio) |
| `url` | string? | URL (for http) |

---

## CLI Config Management
…

…
All new sections have `#[serde(default)]`. Configs from older versions (only `[llm]`, `[agent]`, `[search]`) load correctly — new fields get default values.