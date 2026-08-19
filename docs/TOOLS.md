# Tool Reference

**44 tools** available to agents (+5 browser tools when Chrome is running with CDP). Each implements the `Tool` trait and is automatically registered in `ToolRegistry`.

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
Downloads a page and converts HTML to readable text.

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
Executes a bash command.

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
Checks email: syntax, MX records, disposable, role-based.

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
Spawns a sub-agent for a subtask.

| Parameter | Type | Description |
|----------|------|-------------|
| `task` | string | Task for the sub-agent |
| `role` | string? | Role (default: researcher) |

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

## Control plane

### `question`
Ask a question to the operator and wait for an answer (TUI input or
`POST /sessions/:id/answer` in API). In headless mode without an operator
the agent receives a "continue on your own" response and does not block. Intercepted by the
runtime (like `spawn_agent`) — the tool itself only validates the question.

| Parameter | Type | Description |
|----------|------|-------------|
| `question` | string | One specific question (up to 500 characters) |

### `skill`
Loads full skill instructions by name.

| Parameter | Type | Description |
|----------|------|-------------|
| `name` | string | Skill name (from `~/.parallel-research/skills`) |

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