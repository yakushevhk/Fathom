# Tool Reference

**51 always-registered tools (+ up to 5 CDP + up to 6 computer)** available to agents (+6 computer-use tools when the Playwright computer service is running). Each implements the `Tool` trait and is automatically registered in `ToolRegistry`.

**Tool categories:**

| Category | Tools |
|---|---|
| **Web search** | `web_search`, `web_fetch`, `web_crawl`, `web_feed` |
| **Browser (CDP)** | `browser_navigate`, `browser_click`, `browser_type`, `browser_extract`, `browser_screenshot` |
| **Computer use** | `computer_snapshot`, `computer_navigate`, `computer_click`, `computer_type`, `computer_key`, `computer_screenshot` |
| **File system** | `file_read`, `file_write`, `file_edit`, `glob`, `grep` |
| **Shell** | `shell` (sandboxed) |
| **Code analysis** | `code_symbols`, `repo_map` |
| **OSINT** | `verify_email`, `suggest_emails`, `verify_phone`, `verify_social_profile`, `search_social`, `search_business_directory`, `find_leads`, `enrich_company`, `enrich_person`, `extract_contacts`, `parse_corporate_site`, `search_news` |
| **Memory** | `memory_absorb`, `memory_search`, `memory_digest`, `memory_boost`, `memory_link`, `memory_graph`, `memory` (basic) |
| **Data** | `parse_html`, `extract_json` |
| **Vision** | `analyze_image` |
| **Git** | `git_status`, `git_diff`, `git_log`, `git_add`, `git_commit`, `git_push` |
| **PDF** | `pdf_extract` |
| **REPL** | `python_exec`, `node_exec` |
| **Contacts** | `save_contacts` |
| **Agent control** | `spawn_agent`, `question`, `skill`, `scratchpad`, `undo` |
| **Coordination** | `hub`, `daemon` |

---

## Tool Registration

Every tool in the system implements the `Tool` trait, defined in `crates/tools/src/registry.rs`:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> ToolSchema;
    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput>;
}
```

The `ToolRegistry` is a `HashMap<String, Arc<dyn Tool>>` that stores all registered tools by name. It is constructed via `ToolRegistry::with_builtins()`, which registers every built-in tool in a single call. The registry is shared across the agent runtime (`Arc<ToolRegistry>`) and can be extended after construction:

- **`register(&mut self, tool: Arc<dyn Tool>)`** — adds a tool by name. Tools with the same name replace the previous entry.
- **`register_lsp(&mut self, project_root: PathBuf)`** — registers the LSP tool adapter, which lazily starts an LSP server on first use, auto-detected from the project's files.
- **`get(&self, name: &str) -> Option<&Arc<dyn Tool>>`** — looks up a tool by name.
- **`list_schemas(&self) -> Vec<ToolSchema>`** — returns the JSON schemas for all registered tools, used by the prompt builder to present available tools to the LLM and by the MCP bridge to expose tools to external MCP clients.
- **`execute(&self, name, args, ctx) -> ToolOutput`** — dispatches a call. Unknown tool names return `ToolOutput::err("Unknown tool: …")` rather than panicking, so the LLM can read the error and correct itself.

### MCP Bridge

External MCP (Model Context Protocol) servers can contribute tools to the registry through the MCP bridge (`crates/mcp/src/bridge.rs`). Tool names are namespaced as `mcp__{server}__{tool}` to avoid collisions with built-in tools and between servers. The Fathom server can also operate in reverse — exposing its own tool registry as an MCP server over stdio via `fathom mcp-serve`, so external MCP clients (Claude, ZCode, etc.) can call Fathom tools.

---

## Tool Schema Generation

Each tool's `schema()` method returns a `ToolSchema` struct:

```rust
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}
```

The `parameters` field is a JSON Schema object describing the tool's expected arguments — types, defaults, descriptions, and constraints. Schemas are generated at registration time by each tool's implementation and presented to the LLM in the system prompt via the `PromptBuilder` (which groups them into a volatile "tools" section that changes whenever tools are added or removed). The schema is also exposed through the HTTP API and MCP server for client-side tool discovery.

---

## ToolContext

Every tool execution receives a shared `ToolContext` that provides access to all subsystems:

| Field | Type | Purpose |
|-------|------|---------|
| `working_dir` | `PathBuf` | Working directory for file operations |
| `http_client` | `reqwest::Client` | HTTP client with 30s request / 10s connect timeouts |
| `search_config` | `SearchConfig` | Search backend configuration (Linkup, Exa, Tavily, Serper, etc.) |
| `file_history` | `FileHistory` | Checkpoint history for undo support |
| `file_locks` | `FileLockManager` | Per-path locking for concurrent file writes |
| `read_tracker` | `ReadTracker` | Tracks file reads for read-before-write validation and staleness detection |
| `vision_api_base` / `vision_api_key` | `String` | Vision API configuration (env `PARALLEL_VISION_API_*`) |
| `llm` / `fast_llm` | `Option<Arc<dyn LlmProvider>>` | LLM providers for tools that need auxiliary AI calls (entity extraction, memory classification, search reranking). `aux_llm()` prefers the fast model. |
| `contact_db` | `Option<Arc<dyn ContactStore>>` | Contact persistence database (SQLite or PostgreSQL) |
| `crm` | `Option<Arc<CrmSync>>` | CRM sync (amoCRM / Bitrix24 / HubSpot) |
| `fetch_cache` | `FetchCache` | Session-scoped fetch cache (10 min TTL, 64 entries) |
| `mx_cache` | `MxCache` | Session-scoped MX lookup cache (10 min TTL, 256 entries) |
| `memory` | `Option<Arc<Memory>>` | Long-term semantic memory store (mem0/Memora-inspired) |
| `session_id` | `Option<String>` | Session id for scoping / tagging run-level memories |
| `receipt_ledger` | `Option<ReceiptLedger>` | Durable verification-receipt ledger for contact verification |

The context is constructed via builder methods (`with_llm`, `with_memory`, `with_contact_db`, etc.) and shared across all agents in a session via `Arc<ToolContext>`.

---

## Agent Loop Integration

Tools are executed within the agent runtime's main loop (`crates/agent/src/runtime.rs`), which proceeds through three phases each turn:

### Phase 1: Pre-processing

The LLM's response is parsed into `ToolCall` structs. Each call goes through:

1. **Doom loop detection** — if the same tool+args has been called 3+ times consecutively, the model is nudged (first offense) or stopped (second offense).
2. **Denied-tools check** — per-role deny lists (configurable in `[agent] deny_tools`).
3. **Pre-tool hooks** — user-defined hooks (`eval` / `approve` / `deny`) via `[hooks]` config.
4. **Budget enforcement** — `TurnBudget` tracks per-turn token usage and truncates oversized tool outputs.

### Phase 2: Execution

The `ToolExecutor` partitions calls into parallel-safe and sequential groups (see below), executes them, and returns results tagged with durations. Parallel-safe calls are spawned as independent tasks so CPU-bound tools overlap across worker threads.

### Phase 3: Post-processing

Results are processed in original call order:

- **Cascading cancellation** — a failed shell tool cancels all sibling calls that follow it in the batch.
- **Sub-agent delegation** — `spawn_agent` calls are collected and run concurrently after the post-processing pass.
- **Operator questions** — `question` tools block until the human answers (or a timeout tells the agent to proceed alone).
- **Post-tool hooks** — may append additional context to results.
- **Autosave** — harvested contacts from any tool's metadata are automatically persisted to the contact database.
- **Findings** — structured findings from tool metadata are harvested and accumulated.
- **Persistence** — the turn's messages and tool results are saved to the database.

---

## Parallel Execution

Tools are classified for smart parallelism in `ToolExecutor`:

| Class | Behavior |
|-------|----------|
| **Parallel-safe** (read-only) | Executed concurrently via `futures::future::join_all` (network I/O overlap) or `tokio::spawn` (CPU + I/O overlap across worker threads) |
| **Sequential** (write/state) | Executed one at a time, in order |

### Parallel-safe tools

Web searches, fetches, crawls, parsers, file reads, glob, grep, PDF extraction, image analysis, OSINT verification/enrichment/extraction, memory reads (`memory_search`, `memory_digest`). These are read-only: they read from the network, filesystem, or database without mutating state.

### Sequential tools

File writes, file edits, shell commands, browser automation, git operations, code REPLs, `spawn_agent`, `question`, `memory_absorb`, `memory_boost`, `memory_link`, `memory_graph`, `save_contacts`, `skill`, `scratchpad`, `undo`. These mutate state and require exclusive access.

### Execution strategies

The `ToolExecutor` provides two execution strategies:

- **`execute_batch`** — polls all parallel-safe futures on the calling task via `join_all`. Overlaps network I/O but not CPU-bound work (futures share one thread).
- **`execute_batch_spawn`** — spawns each parallel-safe call as its own tokio task. The multi-threaded runtime spreads them across worker threads, so CPU-heavy tools (parse_html, extract_json, grep) genuinely overlap. This is the default strategy used by the runtime.

### Path-overlap detection

If two parallel-safe file tools target the same file path, the second one is moved to the sequential group to avoid read-write races. The `extract_file_path` function checks common argument keys (`path`, `file`, `file_path`, `filename`, `pattern`).

### Cascading cancellation

When a shell tool fails, all sibling tool calls in the batch are cancelled with a clear error message. This prevents the agent from continuing after a destructive command failure and avoids wasted work.

---

## Caching

The system maintains two session-scoped caches shared across all agents via `ToolContext`:

### FetchCache

Memoizes successful `web_fetch` responses (body + content type) keyed by URL. **64 entries** maximum, **10-minute TTL**. Eviction is oldest-by-insertion when the cap is exceeded. This means agents that re-read the same URL within the TTL skip the download entirely — particularly useful when multiple sub-agents are researching the same sources.

### MxCache

Memoizes DNS-over-HTTPS MX lookup results per domain. **256 entries** maximum, **10-minute TTL**. The cached value is the full MX record list (including empty lists for domains without mail) so cache hits reproduce byte-identical tool output. Repeated `verify_email` calls for addresses at the same domain skip the DNS round trip.

Both caches are tiny, bounded, cheap to clone (everything lives behind `Arc`), and strictly transparent — they only skip network/DNS work, never changing what a tool returns.

---

## Stall Detection

The coordinator runs a stall monitor (`crates/agent/src/coordinator.rs`) for every session to detect agents that have stopped making progress.

### How it works

1. Every `AgentEvent` updates a per-agent "last progress" timestamp.
2. A background task ticks every 30 seconds and checks idle time.
3. Default thresholds: **warn at 450 seconds** (7.5 minutes), **kill at 1200 seconds** (20 minutes). Both are configurable via `[agent] stall_warn_seconds` and `[agent] stall_kill_seconds`; setting either to 0 disables that stage.
4. On warn: a `tracing::warn!` log is emitted (visible in TUI).
5. On kill: the agent's `CancellationToken` is cancelled, which the runtime checks at the start of each main loop iteration. The agent stops with an `AgentFailed` event.

The stall monitor only watches agents registered in the session's token map. It auto-terminates when the session completes or fails.

### Doom loop detection

Separate from the stall monitor, the `DoomLoopDetector` (`crates/agent/src/doom_loop.rs`) catches agents stuck retrying the same failing operation. It tracks a bounded history of `(tool_name, args_hash)` signatures — the `hash_args` function uses `serde_json::Value`'s sorted-key serialization so semantically identical arguments produce the same hash regardless of insertion order. After 3 consecutive identical calls (default threshold), the agent is nudged to change strategy. On a second offense, the agent is stopped entirely. The detector resets when the agent recovers.

---

## Web

### `web_search`
Internet search via the configured backend (Linkup/Exa/Tavily/Serper/Brave/Parallel/DuckDuckGo).

| Parameter | Type | Description |
|----------|------|-------------|
| `query` | string | Search query |
| `limit` | u32 | Max results (default 10) |

**Returns**: list of results (title, URL, snippet).

### `web_fetch`
Downloads a page and converts HTML to readable text. Results are cached in the session-scoped FetchCache (10 min TTL) so repeated fetches of the same URL within the same session skip the download entirely.

| Parameter | Type | Description |
|----------|------|-------------|
| `url` | string | Page URL |
| `extract_text` | bool | Extract only text (default true) |

**Returns**: page text (truncated to 50K characters).

### `web_crawl`
BFS crawl of a site from a seed URL with deduplication and a politeness delay between requests.

| Parameter | Type | Description |
|----------|------|-------------|
| `url` | string | Starting http(s) URL |
| `max_pages` | usize | Max pages (default 10, cap 50) |
| `max_depth` | usize | Crawl depth (default 1, cap 3) |
| `same_domain` | bool | Stay within the same domain (default true) |
| `delay_ms` | u64 | Delay between requests (default 500, cap 5000) |
| `selector` | string? | CSS selector for text extraction (default: entire `<body>`) |
| `chars_per_page` | usize | Text per page (default 1500, 200–8000) |

**Returns**: `pages_fetched`, `links_discovered`, array of pages (url, depth, title, text, outgoing links or error).

### `web_feed`
Parses RSS/Atom/sitemap from a URL or local file (quick-xml).

| Parameter | Type | Description |
|----------|------|-------------|
| `source` | string | Feed URL or local file |
| `limit` | usize | Max items (default 50, 1–200) |
| `include_summaries` | bool | Include descriptions (default true, truncated to 500 characters) |

**Returns**: `kind` (`rss` | `atom` | `sitemap`), `count`, items (title, link, date?, summary?).

---

## Parsing

### `parse_html`
Structured data extraction from HTML via CSS selectors (URL with SSRF protection and session cache, or local file).

| Parameter | Type | Description |
|----------|------|-------------|
| `source` | string | URL or file path |
| `selector` | string? | CSS selector (default `body`) |
| `mode` | string? | `texts` (default) \| `html` \| `attr` \| `links` \| `tables` |
| `attribute` | string? | Attribute for `attr` mode (e.g. `href`) |
| `limit` | usize | Max elements (default 100, cap 500) |

**Returns**: JSON — element texts, inner HTML, attributes, links `{text, href}`, or tables as arrays of strings.

### `extract_json`
Queries a JSON document (API, file, or inline string) via dot-path.

| Parameter | Type | Description |
|----------|------|-------------|
| `source` | string | URL, file path, or JSON starting with `{`/`[` |
| `path` | string? | Path like `data.items.0.name`, iteration `items[*].email` |
| `limit` | usize | Max results (default 100, cap 500) |

**Returns**: selected values as a JSON array.

---

## Code

### `code_symbols`
Searches for definitions (functions, classes, structs, traits, methods) in a file or directory — line-based heuristics without tree-sitter.

| Parameter | Type | Description |
|----------|------|-------------|
| `path` | string | File or directory |
| `query` | string? | Case-insensitive substring for name filtering |
| `limit` | usize | Max characters (default 200, cap 1000) |

**Returns**: `files_scanned`, `symbols_found`, list of (file, name, kind, line, signature). Supports: Rust, Python, JS/TS, Go, Ruby, Java/Kotlin, C/C++/C#, PHP.

### `repo_map`
Compact codebase map: files by language + top symbols per file. Parallel file reading, cached regex.

| Parameter | Type | Description |
|----------|------|-------------|
| `path` | string? | Directory (default: working directory) |
| `max_files` | usize | Max files in map (default 300, cap 2000) |
| `symbols_per_file` | usize | Top symbols per file (default 3, cap 10; 0 = file list only) |

**Returns**: `files_mapped`, `files_total`, `by_language`, entries (file, lang, top_symbols). Skips `.git`, `target`, `node_modules`, etc.

---

## Files

### `file_read`
Reads a file (with partial read support).

| Parameter | Type | Description |
|----------|------|-------------|
| `path` | string | File path |
| `offset` | u32? | Starting line |
| `limit` | u32? | Number of lines |

**Feature**: tracks read files for read-before-write validation.

### `file_write`
Writes a file (creates directories).

| Parameter | Type | Description |
|----------|------|-------------|
| `path` | string | File path |
| `content` | string | Content |

**Feature**: per-path locking, updates file history.

### `file_edit`
Targeted find/replace.

| Parameter | Type | Description |
|----------|------|-------------|
| `path` | string | File path |
| `old_string` | string | What to replace |
| `new_string` | string | What to replace with |

**Validation**: read-before-write, staleness check, size guard.

### `glob`
File search by glob pattern.

| Parameter | Type | Description |
|----------|------|-------------|
| `pattern` | string | Glob pattern (e.g. `**/*.rs`) |

### `grep`
Search file contents (regex).

| Parameter | Type | Description |
|----------|------|-------------|
| `pattern` | string | Regex pattern |
| `path` | string? | Search directory |
| `file_pattern` | string? | File filter |

Uses ripgrep if available.

---

## Execution

### `shell`
Executes a bash command. A shell failure triggers cascading cancellation of all sibling tool calls in the batch — no subsequent tool in the same turn runs.

| Parameter | Type | Description |
|----------|------|-------------|
| `command` | string | Command |
| `timeout` | u64 | Timeout in seconds (default 120) |

**Protection**: blocks destructive commands (`rm -rf /`, `mkfs`, fork bombs).

### `python_exec`
Executes Python code.

| Parameter | Type | Description |
|----------|------|-------------|
| `code` | string | Python code |
| `timeout` | u64? | Timeout |

### `node_exec`
Executes Node.js code.

| Parameter | Type | Description |
|----------|------|-------------|
| `code` | string | JavaScript code |
| `timeout` | u64? | Timeout |

---

## Browser (Chrome DevTools Protocol)

Requires Chrome running with `--remote-debugging-port=9222`. Automatically detects CDP availability.

### `browser_navigate`
Opens a URL in the browser.

| Parameter | Type | Description |
|----------|------|-------------|
| `url` | string | URL to open |

### `browser_screenshot`
Takes a screenshot of the current page.

**Returns**: base64 image in `metadata.base64`.

### `browser_click`
Clicks an element (CSS selector).

| Parameter | Type | Description |
|----------|------|-------------|
| `selector` | string | CSS selector |

### `browser_type`
Types text into an element.

| Parameter | Type | Description |
|----------|------|-------------|
| `selector` | string | CSS selector |
| `text` | string | Text to type |

### `browser_extract`
Extracts the text content of the page (with JS rendering).

**Returns**: page text (truncated to 50K).

---

## Computer Use (Playwright)

Enabled when the loopback Playwright computer service (`apps/computer`) is running. The agent operates a **real browser** with a persistent Chromium profile/workspace through accessibility-tree snapshots with opaque refs — it never depends on brittle CSS selectors. The service also exposes `/screen` screenshot streaming, `/control/ws` input forwarding (with human bot/human control leases), and a confined `/files` workspace.

Requires the computer service and configuration: `FATHOM_COMPUTER_SERVICE_URL` / `COMPUTER_SERVICE_URL` + `COMPUTER_TOKEN` (or `COMPUTER_IMAGE`/`COMPUTER_NETWORK`/`COMPUTER_BASE_PORT` for the Docker supervisor). See `docs/COMPUTER-USE.md`.

Captures the current accessibility-tree snapshot of the active tab. Returns structured UI elements with opaque refs for subsequent actions.

**Parameters**: none (empty params).

### `computer_navigate`
Navigates the active tab to a URL. Egress guard rejects localhost/private/link-local/multicast/metadata targets by default.

| Parameter | Type | Description |
|----------|------|-------------|
| `url` | string | URL to open |

### `computer_click`
Clicks a UI element by its opaque snapshot ref.

| Parameter | Type | Description |
|----------|------|-------------|
| `ref` | string | Element ref from a snapshot |

### `computer_type`
Types text into a focused element (or one addressed by ref).

| Parameter | Type | Description |
|----------|------|-------------|
| `text` | string | Text to type |
| `ref` | string? | Element ref from a snapshot |

### `computer_key`
Sends a keyboard key / chord (Enter, Tab, Ctrl+C…).

| Parameter | Type | Description |
|----------|------|-------------|
| `key` | string | Key name or chord |

### `computer_screenshot`
Takes a screenshot of the current page.

**Returns**: base64 image in `metadata.base64`.

---

## Vision

### `analyze_image`
Analyzes an image via a vision model (Qwen 3.8).

| Parameter | Type | Description |
|----------|------|-------------|
| `image` | string | File path or image URL |
| `prompt` | string? | What to analyze |

**Config**: `PARALLEL_VISION_API_BASE`, `PARALLEL_VISION_API_KEY`, `PARALLEL_VISION_MODEL`.

---

## Git

### `git_status`
Shows the working tree status.

### `git_diff`
Shows changes.

| Parameter | Type | Description |
|----------|------|-------------|
| `staged` | bool? | Staged changes only |
| `path` | string? | Specific file |

### `git_log`
Commit history.

| Parameter | Type | Description |
|----------|------|-------------|
| `limit` | u32? | Max commits |

### `git_add`
Adds files to staging.

| Parameter | Type | Description |
|----------|------|-------------|
| `paths` | string[] | Files to add |

### `git_commit`
Creates a commit.

| Parameter | Type | Description |
|----------|------|-------------|
| `message` | string | Commit message |

### `git_push`
Pushes to remote.

| Parameter | Type | Description |
|----------|------|-------------|
| `remote` | string? | Remote (default: origin) |
| `branch` | string? | Branch |

---

## PDF

### `pdf_extract`
Extracts text from a PDF.

| Parameter | Type | Description |
|----------|------|-------------|
| `path` | string | Path to PDF file |

**Returns**: text + page count. Supports ToUnicode CMaps, encryption (empty password).

---

## OSINT / Lead Generation

### `extract_contacts`
Extracts contacts from text/HTML/URL.

| Parameter | Type | Description |
|----------|------|-------------|
| `text` | string? | Text to analyze |
| `html` | string? | HTML to analyze |
| `url` | string? | URL to download and analyze |

**Returns**: emails, phones (E.164), social profiles, persons, companies — each with confidence and source.

### `save_contacts`
Saves collected contacts to a persistent database with deduplication and optional push to CRM (amoCRM/Bitrix24/HubSpot).

| Parameter | Type | Description |
|----------|------|-------------|
| `contacts` | array | Array of contacts: `email`, `phone`, `name`, `title`, `company`, `socials`, `tags`, `notes`, `source`; at least one of email/phone/name required |
| `push_crm` | bool | Push to CRM (default true) |

**Feature**: when a normalized email/phone matches, data is MERGED into the existing record instead of creating a duplicate. **Returns**: how many added/merged and CRM push results.

### `find_leads`
High-level lead search.

| Parameter | Type | Description |
|----------|------|-------------|
| `industry` | string? | Industry |
| `location` | string? | Location |
| `company_size` | string? | Company size |
| `role_titles` | string[]? | Job titles (CEO, CTO...) |
| `limit` | u32? | Max leads |

**Process**: business directories → corporate websites → social networks → contact extraction → deduplication.

### `search_business_directory`
Search business directories (2GIS, Google Maps, Yandex Maps, Yellow Pages).

| Parameter | Type | Description |
|----------|------|-------------|
| `query` | string | What to search |
| `city` | string? | City |

**Returns**: name, category, address, phone, website, email, rating.

### `search_social`
Search for people/companies on social networks (Twitter, Telegram, LinkedIn).

| Parameter | Type | Description |
|----------|------|-------------|
| `query` | string | Search query |
| `platform` | string? | Specific platform |

### `parse_corporate_site`
Parses a corporate website: company info, team, contacts.

| Parameter | Type | Description |
|----------|------|-------------|
| `url` | string | Site URL |

**Returns**: company_name, description, industry, size, contacts, team_page_url, social_profiles.

### `search_news`
Search news/mentions (Serper News or Google News RSS).

| Parameter | Type | Description |
|----------|------|-------------|
| `query` | string | Who/what to search |
| `limit` | u32? | Max articles (default 10) |

**Returns**: title, URL, source, date, snippet + extracted persons/companies.

---

## Verification

### `verify_email`
Checks email: syntax, MX records, disposable, role-based. MX lookups are cached in the session-scoped MxCache (10 min TTL, 256 entries) so repeated checks for the same domain skip the DNS round trip.

| Parameter | Type | Description |
|----------|------|-------------|
| `email` | string | Email to check |

**Returns**: is_valid_syntax, domain_exists, mx_records, is_disposable, is_role_based, confidence.

### `suggest_emails`
Generates probable corporate emails for a person from name permutations on the domain, determines the company pattern from known colleagues' addresses, and verifies each candidate (syntax + MX, optionally SMTP).

| Parameter | Type | Description |
|----------|------|-------------|
| `name` | string | Name in Latin script (e.g. `Ivan Petrov`) |
| `domain` | string | Corporate domain |
| `known_emails` | string[]? | Known addresses of other people on the domain — to determine the pattern |
| `smtp_check` | bool | SMTP mailbox check (default false, slow) |

**Returns**: up to 9 candidates with confidence (0–1) and pattern match flag.

### `verify_phone`
Validates and normalizes a phone number (E.164).

| Parameter | Type | Description |
|----------|------|-------------|
| `phone` | string | Phone number |

**Returns**: normalized (E.164), country_code, country_name, is_valid, is_mobile.

### `verify_social_profile`
Checks that a social media profile exists.

| Parameter | Type | Description |
|----------|------|-------------|
| `url` | string | Profile URL |

**Returns**: exists, platform, username, name, followers.

---

## Enrichment

### `enrich_company`
Enriches company data.

| Parameter | Type | Description |
|----------|------|-------------|
| `company_name` | string | Company name |
| `website` | string? | Website |

**Returns**: industry, size, revenue, founded, headquarters, description, technologies.

### `enrich_person`
Enriches person data.

| Parameter | Type | Description |
|----------|------|-------------|
| `name` | string | Person's name |
| `company` | string? | Company |

**Returns**: title, company, linkedin, twitter, email, phone, location, bio.

---

## Meta

### `spawn_agent`
Spawns a sub-agent for a subtask. The runtime collects all spawn requests in a turn, verifies that the nesting depth is not exceeded, and executes the children concurrently after the post-processing pass. Depth enforcement is handled by the agent runtime, not the tool itself.

| Parameter | Type | Description |
|----------|------|-------------|
| `task` | string? | Task for the sub-agent (single spawn) |
| `role` | string | Agent role (default: `researcher`) |
| `context` | string[]? | Context for the child |
| `background` | bool | Run in background, default false |
| `tasks` | object[]? | Batch spawn (up to 8 tasks) |
| `output_schema` | object? | JSON schema for the output |
| `isolated` | bool | Isolated mode, default false |
| `handoff_to` | string? | Handoff the session to another agent |

**Limitation**: nesting depth (`max_depth`).

### `memory`
Agent memory management (MEMORY.md/USER.md).

| Parameter | Type | Description |
|----------|------|-------------|
| `action` | string | `add` \| `replace` \| `remove` \| `batch` |
| `target` | string | `memory` \| `user` |
| `content` | string? | Content |
| `old_text` | string? | For replace/remove |
| `operations` | array? | For batch |

See [MEMORY-SKILLS.md](MEMORY-SKILLS.md) for details.

## Long-term semantic memory

Knowledge base in SQLite (mem0/Memora model): hybrid search, append-only
fact versioning, entity graph. Available when `[memory] enabled = true`.
See [MEMORY-KB.md](MEMORY-KB.md) for details.

### `memory_absorb`
Save facts through the full absorb pipeline (secrets → consolidation →
dedup → classification `duplicate/supersede/contradict/related/new`).

| Parameter | Type | Description |
|----------|------|-------------|
| `facts` | array | `[{content, tags?, confidence?, metadata?}]` |
| `source` | string? | Origin (default: current session) |
| `scope` | string? | `agent` \| `user` \| `run` (default `agent`) |
| `context` | string? | Hint for the classifier (not saved) |
| `dry_run` | bool | Plan without writing |

### `memory_search`
Hybrid search (vectors + BM25 + freshness decay), optionally LLM-rerank.

| Parameter | Type | Description |
|----------|------|-------------|
| `query` | string | Query (if no `id`) |
| `id` | string? | Read a single record by id/prefix |
| `follow` | string? | `active` \| `latest` \| `full_history` (with `id`) |
| `top_k` | usize? | Result set size |

### `memory_digest`
Deterministic digest on a topic: relevant facts + open TODOs + recent activity.

| Parameter | Type | Description |
|----------|------|-------------|
| `topic` | string | Topic/task |

### `memory_boost`
Increase/decrease a record's importance (affects ranking).

| Parameter | Type | Description |
|----------|------|-------------|
| `id` | string | Record id/prefix |
| `amount` | f64 | Delta (default +0.5; negative to decrease) |

### `memory_link`
Typed edge between two records.

| Parameter | Type | Description |
|----------|------|-------------|
| `from` / `to` | string | Record ids/prefixes |
| `edge_type` | string | `related_to` \| `supersedes` \| `contradicts` \| `implements` \| `extends` \| `references` |
| `reason` | string? | Explanation |

### `memory_graph`
Entity graph person↔company↔location: node dedup by (name+type), multi-hop traversal.

| Parameter | Type | Description |
|----------|------|-------------|
| `action` | string | `add` \| `query` \| `list` |
| `entities` | array? | `[{name, type}]` for add |
| `relations` | array? | `[{from, to, relation, confidence?}]` for add |
| `name` | string? | Starting entity for query |
| `entity_type` | string? | Type filter |
| `depth` | usize? | Traversal depth (1–4, default 2) |

> **Headless mode:** When no operator is connected (headless/automated run), the question tool returns a "proceed on your own" response immediately, allowing agents to continue without human input. In operator-connected mode (TUI or HTTP API), the question blocks until answered or timed out.

---

> **Steer mode:** The `steer` parameter on `send` delivers messages as steering directives rather than regular inbox messages. Steering messages are injected mid-run as operator instructions, allowing an operator to redirect an agent's focus during execution without restarting the session. This is distinct from `await_reply` — steer messages are fire-and-forget operator directives, not conversational turns.

---

> **Log following:** The `ready_pattern` parameter accepts a regex that is matched against stdout/stderr. When set, the tool blocks until the pattern matches or the timeout expires, enabling wait-for-startup-message semantics beyond simple port binding. The daemon registry stores stdout/stderr history for later retrieval via the `status` command.

### `skill`
Loads full skill instructions by name.

| Parameter | Type | Description |
|----------|------|-------------|
| `name` | string | Skill name (from `~/.fathom/skills`) |

### `scratchpad`
Shared session ledger for coordination between agents (`.pr-context/ledger.md`).

| Parameter | Type | Description |
|----------|------|-------------|
| `action` | string | `read` \| `append` |
| `text` | string? | String for `append` |

**Why**: parallel agents can see what others have already covered and avoid duplicating work; the ledger survives compaction.

### `undo`
Revert file changes via file history checkpoints (OpenCode-style).

| Parameter | Type | Description |
|----------|------|-------------|
| `steps` | usize | How many checkpoints to revert (default 1) |

---

## Parallelism safety

Tools are classified for smart parallelism:

| Class | Tools | Behavior |
|-------|-------|----------|
| **Parallel-safe** (read-only) | web_search, web_fetch, web_crawl, web_feed, parse_html, extract_json, code_symbols, repo_map, file_read, glob, grep, pdf_extract, analyze_image, verify_email, verify_phone, verify_social_profile, suggest_emails, enrich_company, enrich_person, extract_contacts, search_business_directory, search_social, parse_corporate_site, search_news, find_leads, memory_search, memory_digest | Can execute in parallel |
| **Sequential** (write/state) | file_write, file_edit, shell, python_exec, node_exec, browser_*, git_*, spawn_agent, memory, memory_absorb, memory_boost, memory_link, memory_graph, question, save_contacts, skill, scratchpad, undo | Execute sequentially (exclusive access) |

Path-overlap detection: two file tools on the same path are serialized.