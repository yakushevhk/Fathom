# CLI Usage

The `fathom` binary is a **universal autonomous AI worker** — it accepts natural-language tasks, decomposes them into sub-tasks with hierarchical sub-agents, and executes them autonomously. Commands span headless investigation, interactive TUI, HTTP API serving, MCP tool exposure, memory management, background jobs, session history, contact/CRM operations, and browser-based computer use.

```
fathom <COMMAND>

Commands:
  run       Run an investigation (headless)
  worker    Worker mode (internal, for multi-process)
  tui       Interactive TUI
  serve     HTTP API server
  mcp-serve MCP server (stdio)
  memory    Long-term semantic memory operations
  sessions  Browse past session history
  resume    Resume an interrupted session
  contacts  Contact database operations (OSINT / lead generation)
  jobs      Background jobs with self-healing retries
  profiles  Manage personas/profiles
  config    Configuration management
  bench     Benchmark the tool-execution layer
  stats     Show tool-call statistics for a recorded session
```

---

## `run` — headless investigation

The primary mode. Launches a coordinator that decomposes the request into sub-agents, runs them in parallel, and synthesises the results.

```bash
fathom run <QUERY> [OPTIONS]

Arguments:
  <QUERY>  Research query

Options:
  -o, --output <DIR>      Directory for results (default: from config)
  --profile <NAME>        Persona: hunter | analyst | validator | file in
                          ~/.fathom/profiles/ | path to .toml
  --repeat <SECS>         Watch mode: repeat the request every N seconds
  --task-file <PATH>      Read the task from a file instead of the positional
                          argument (useful for long instructions with
                          quotes/newlines, e.g. Terminal-Bench). The file
                          content wins over `query` when both are given.
```

### Personas (`--profile`)

A preset adds a system prompt and can override `model`/`fast_model`, `temperature`, `max_depth`, `max_agents`, `replan_rounds`, and `deny_tools`. Profiles are loaded from built-in presets (`hunter`, `analyst`, `validator`), user-created files in `~/.fathom/profiles/`, or an explicit path to a `.toml` file.

```bash
fathom profiles list
fathom profiles show hunter
fathom profiles new my-persona   # template in profiles/
fathom run --profile hunter "find DM at Acme"
```

Each profile is a TOML file that can define a `system_prompt` block, model overrides, temperature, `max_depth`, `max_agents`, `replan_rounds` (Goal Mode), `timeout_seconds`, and `deny_tools`. The `--profile` flag is also available on the `tui` command.

### Goal Mode (`replan_rounds`)

After the initial fan-out of sub-agents completes, the coordinator runs **Goal Mode** — a quality assurance loop. An LLM judge reviews every collected finding against the original goal and decides whether the goal is fully satisfied. If concrete gaps remain (e.g. insufficient data on a specific subtopic, missing contact details for a subset of targets), the judge proposes up to 3 gap-filling subtasks and a **replan round** launches to cover them. This repeats up to `replan_rounds` times (configured in `[agent] replan_rounds`; default 1). Set `replan_rounds = 0` to disable.

Goal Mode is especially valuable for lead-gen queries where partial results are the norm — the judge catches gaps like "CTO contacts missing for 5 companies" and sends targeted agents to fill them.

### Watch mode (`--repeat`)

Each run is compared against the previous one against the contacts database: new emails / phones / personas are printed as a diff and sent out as an alert via the `[notifications]` channels (webhook / Telegram / email, event `watch.new_contacts`). Session completion/crash notifications also work (`session.completed`, `session.failed`).

```bash
fathom run "Acme team" --repeat 21600   # every 6 hours
```

The watch loop runs forever until interrupted. It maintains a set of known contact identity keys between runs, so only genuinely new contacts trigger alerts.

### Examples

```bash
# Simple investigation
fathom run "What are quantum computers?"

# With output directory specified
fathom run "Compare Rust and Go" --output ./research/

# OSINT / LeadGen
fathom run \
  "Find contacts of CEOs of IT companies in Moscow. Extract emails and phone numbers." \
  --output ./leads/

# Long task from a file (handles quotes, newlines, complex instructions)
fathom run --task-file ./tasks/competitor-analysis.txt

# With a persona profile
fathom run --profile analyst "Analyze Q3 market trends for electric vehicles"
```

### What happens

1. The coordinator plans: decomposes the request into 2-5 subtasks
2. Persists planned subtasks to the session database (Goal Mode light — observable progress)
3. Launches sub-agents (in parallel)
4. Agents use tools (web_search, extract_contacts, ...)
5. The coordinator runs the Goal Mode judge (up to `replan_rounds` gap-filling rounds)
6. The coordinator synthesizes the results
7. Writes `index.md`, `summary.md`, `findings/`, `sources.md`
8. Exports (PDF/HTML/JSON/DOCX) and sends notifications
9. CRM sync (if configured) pushes new contacts automatically

### Output structure

```
output/
├── index.md           # Table of contents + metadata
├── summary.md         # Final synthesis
├── sources.md         # List of sources
├── findings/          # Findings per subtask
│   ├── finding-1.md
│   ├── finding-2.md
│   └── finding-3.md
├── report.html        # Export (if configured)
├── contacts.csv       # Contacts exported from this run (if any)
└── .research.db       # Session SQLite database
```

### Logging

```bash
# Verbose logs
RUST_LOG=debug fathom run "..."

# Errors only
RUST_LOG=error fathom run "..."
```

All logging goes to stderr, so stdout is clean for `mcp-serve` or structured output.

---

## `tui` — interactive mode

Terminal interface built on ratatui. Shows live agent progress, streaming LLM output, tool calls, and an event log.

```bash
fathom tui [QUERY] [--profile <NAME>] [--replay <SESSION-ID>]

Arguments:
  [QUERY]  Optional initial query

Options:
  --profile <NAME>      Persona for sessions started from the TUI
  --replay <SESSION-ID> View a saved session (without a live run)
```

### Layout

```
┌───────────────────────────────────────────────────┐
│ Fathom | Session: xxx | 2:34 │ ███░ 67% │
├────────────────┬──────────────────────────────────┤
│ Agents         │ Output / Thinking                 │
│ ○ coordinator  │ # Research Summary                │
│   ◐ researcher │ Text streams here...              │
│   ◐ researcher │                                   │
│   ✓ analyst    │ [thinking] dim cyan panel         │
├────────────────┼──────────────────────────────────┤
│ Tools          │ Event Log                        │
│ → web_search   │ • Agent spawned                  │
│ ✓ web_fetch    │ → web_search (2.1s)              │
├────────────────┴──────────────────────────────────┤
│ > Enter a query...                                │
└───────────────────────────────────────────────────┘
```

### Keys

| Key | Action |
|---------|----------|
| `i` | Enter input mode |
| `Enter` | Send the request |
| `Shift+Enter` | New line in input |
| `Esc` | Exit input mode |
| `Tab` | Switch panels |
| `↑` / `↓` | Input history / scroll (in the Agents panel — tree cursor) |
| `←` / `→` | Collapse / expand the agent's sub-tree (Agents panel) |
| `t` | Toggle thinking panel |
| `y` / `n` | Approve / reject a pending side-effect tool |
| `c` | Clear output |
| `?` | Key help |
| `q` | Quit |

The header shows a sparkline of token usage over the course of the session. In `--replay`
mode a saved session is loaded: the agent tree with final statuses and
findings in the log (the title is marked `[REPLAY]`).

### Streaming mode

When the TUI is running a live session, LLM responses are streamed in real time. Text deltas appear in the Output panel as they arrive from the provider (via `AgentEvent::LlmStreamChunk`). If the provider cannot stream, the system transparently falls back to a non-streaming completion — an agent never dies because a gateway lacks SSE.

### Approval flow

When an agent wants to run a side-effect tool (e.g. `save_contacts`, `git_push`), the TUI pauses and shows a `PendingApproval` prompt. The operator presses `y` to approve or `n` to reject. Which tools require approval is configured in `[agent] approval_tools`. If the operator is not present, the `approval_fallback` setting (`allow` or `deny`) kicks in after `approval_timeout_seconds`.

### Question tool

Agents can ask the operator a question directly via the built-in `question` tool. When an agent is genuinely blocked — ambiguous goals, missing credentials, a choice between materially different directions — it sends a question to the operator. The TUI surfaces the `PendingQuestion` with a reply channel, and the operator's answer is delivered back to the agent.

### Memory panel

If the memory subsystem is enabled (`[memory] enabled = true`), the TUI shows a Memory panel with recent facts, their scope, and status. The panel refreshes automatically every 2 seconds alongside the Jobs panel.

### Jobs panel

The TUI also shows a background jobs panel (when `fathom jobs submit` has been used), listing all jobs with their status, attempt count, and age. The panel refreshes every 2 seconds.

---

## `serve` — HTTP API server

Launches an Axum server for programmatic control of research sessions.

```bash
fathom serve [OPTIONS]

Options:
  --port <PORT>  Port (default: 8080)
  --host <HOST>  Bind address (default: 127.0.0.1)
```

```bash
fathom serve --port 8080
```

By default the server listens only on loopback. To bind to an external address
(`--host 0.0.0.0`) you **must** set `FATHOM_API_KEYS`,
otherwise startup will be rejected.

The HTTP API supports creating sessions, polling status, and fetching results. It can be used for CI/CD pipelines, web dashboards, or integration with other tools.

For API details — see [HTTP-API.md](HTTP-API.md).

### HTTP API automation example

```bash
# Start the server
fathom serve --port 8080 &

# Create a session
curl -X POST http://localhost:8080/api/v1/sessions \
  -H "Content-Type: application/json" \
  -d '{"query": "Research the AI agent market"}'

# Check status
curl http://localhost:8080/api/v1/sessions/<SESSION_ID>

# Fetch results
curl http://localhost:8080/api/v1/sessions/<SESSION_ID>/results
```

---

## `mcp-serve` — MCP server

Exposes all **63 agent tools** (57 built-in + 6 computer-use) to external MCP clients (Claude Desktop, ZCode, Cursor, etc.) over stdio. The server uses the shared `ToolRegistry` with all tools registered. External MCP servers configured in `[mcp.servers]` are **not** re-exported (to avoid loops).

```bash
fathom mcp-serve
```

Connection in the MCP client config:
```json
{ "command": "fathom", "args": ["mcp-serve"] }
```

Nothing may print to stdout when the MCP server is running — the stdio protocol owns it. All logging goes to stderr.

---

## `computer` — browser-based computer use

Fathom can operate a real browser via a loopback Playwright service (`apps/computer`). The agent uses accessibility-tree snapshots (opaque refs) to "see" the page like a screen-reader and interact via refs — never brittle CSS selectors.

The computer service is started separately from the `apps/computer` directory:

```bash
cd apps/computer
npm start                    # starts the Playwright computer service
```

The computer service exposes a REST API that the coordinator and agents call through the built-in computer-use tools (`computer_snapshot`, `computer_navigate`, `computer_click`, `computer_type`, `computer_key`, `computer_screenshot`). Each agent-action returns a fresh snapshot so the agent always has current state.

### With Docker supervisor

When `COMPUTER_TOKEN` (and optionally `COMPUTER_IMAGE`, `COMPUTER_NETWORK`, `COMPUTER_BASE_PORT`) is set in the environment, Fathom provisions **one isolated computer per agent** — each with its own persistent workspace, profile volume, loopback port, health checks, and restrictive capabilities.

```bash
fathom serve --port 8080
# The HTTP API proxies computer actions to the right Docker container per agent
```

### Human takeover

The TUI, Tauri desktop app, and web dashboard all support **human takeover**: the operator can view the live screen, inspect the accessibility tree, type, click, navigate, and enter secrets — all without the agent losing context. Useful for multi-step auth flows, CAPTCHAs, or sensitive credential entry.

### Computer relay

The HTTP API exposes `/api/v1/computers/*` endpoints that proxy snapshot, click, type, key, screen, files, and control actions to the active computer service (or per-agent Docker container). This enables:

- **Live screen streaming** in the web dashboard and desktop app
- **Human-in-the-loop approval** for sensitive actions
- **Secret injection** without exposing the value in logs or to the agent

For full details — see [COMPUTER-USE.md](COMPUTER-USE.md).

---

## `memory` — long-term semantic memory

Manage the semantic knowledge base without running the agent. The memory subsystem stores facts with scopes (`agent`, `user`, `run`), supports versioning (active/superseded/archived), hybrid search (semantic + keyword), and automatic embedding.

```bash
# Hybrid search (semantic + keyword)
fathom memory search "Acme CEO email" [--top-k 10] [--scope agent|user|run|all]

# List records
fathom memory list [--scope ...] [--status active|superseded|archived|all] [-n 20]

# Single record + version chain
fathom memory get <id> [--follow active|latest|full_history]

# Statistics (scopes, entity graph, DB size)
fathom memory stats

# Re-embedding after changing the model
fathom memory rebuild

# Distill session run-facts into durable knowledge
fathom memory distill [--session <key>] [--dry-run]

# GC: archiving expired/stale facts + N→1 compaction of groups
fathom memory gc [--ttl-days 30] [--dry-run]

# Full deletion of scope records (requires --yes)
fathom memory nuke --scope run --yes
```

The `distill` command extracts run-scoped facts (from a specific session or all sessions) into persistent `agent`-scope knowledge, so agents in future runs benefit from past findings. The `gc` command archives expired facts (based on `expires_at` metadata) and compacts oversized scope groups. `rebuild` re-embeds all facts with the current embedding model — useful after changing the embedding provider.

Details — see [MEMORY-KB.md](MEMORY-KB.md).

---

## `sessions` — session history

Browse past research sessions stored in the SQLite database.

```bash
# Recent sessions (search by query substring)
fathom sessions list [-n 20] [--search "kazan"]

# Details of a single session: agents + findings
fathom sessions show <id-or-prefix>
```

The `show` command accepts a unique prefix (first few characters of the session ID). It displays the query, status, creation time, agent count, token usage, and all findings produced. The session database lives in the configured output directory.

---

## `worker` — internal mode

Used by the coordinator in multi-process mode. **Do not run manually.**

```bash
fathom worker \
  --session-id <SID> \
  --agent-id <AID> \
  --task <TASK> \
  --socket <PATH> \
  --role <ROLE>
```

Activated automatically when `use_multiprocess = true`. Each worker connects to the coordinator over a Unix socket, receives its task, runs the agent loop, and streams results back. Workers enforce the same wall-clock timeout as in-process agents.

---

## `resume` — resume an interrupted session

Re-runs unfinished sub-tasks of a previously interrupted session. Useful when a session was killed mid-flight (e.g. Ctrl+C, crash, timeout).

```bash
fathom resume [--output <DIR>] [--session-id <ID>]

Options:
  -o, --output <DIR>      Session output directory (contains .research.db).
                          Defaults to the configured output dir.
  -s, --session-id <ID>   Session id to resume. Defaults to the most recent
                          interrupted one.
```

The resumer loads the session's persisted subtasks from the database, identifies which sub-agents completed successfully and which did not, and re-runs only the unfinished ones. Completed findings are preserved. After the resumed run, the session is finalized as usual (export, notifications, CRM sync).

---

## `jobs` — durable background jobs

Submit research tasks that run detached from the terminal, check their status any time, read logs, and cancel or re-run them. Failed attempts are retried automatically with a self-healing task that carries the previous error context.

```bash
# Submit a task to run in the background
fathom jobs submit "<task>" [--attempts 3]

# List all jobs
fathom jobs list

# Show detailed status of one job
fathom jobs status <id-or-prefix> [--watch 5]

# Show the job's log (stdout+stderr of all attempts)
fathom jobs logs <id-or-prefix> [-n 50]

# Cancel a queued or running job
fathom jobs cancel <id-or-prefix>

# Re-run a failed/cancelled/completed (or stale) job from scratch
fathom jobs rerun <id-or-prefix>
```

### Job lifecycle

1. **Submit**: `jobs submit` creates a job record, spawns a detached process, and returns immediately.
2. **Running**: the detached process executes the research task. Output is written to `<job-dir>/job.log`.
3. **Retry**: on failure, the job waits `5 × attempt` seconds (backoff), then retries with the original task **augmented with the previous error** — so the agent can diagnose the partial workspace and fix its own mistake.
4. **Terminal states**: `completed`, `failed`, `cancelled`.

```bash
fathom jobs submit "Find 20 SaaS companies in Berlin" --attempts 3
# Output:
# Submitted job a1b2c
#   Task:     Find 20 SaaS companies in Berlin
#   Attempts: 3
#   Dir:      ~/.fathom/jobs/a1b2c3d4-...
#   Log:      ~/.fathom/jobs/a1b2c3d4-.../job.log
#
#   Watch it live:  fathom jobs status a1b2c --watch 5
#   Tail the log:   fathom jobs logs a1b2c
```

---

## `contacts` — contact database operations

Manage the OSINT/lead-generation contact database independently of research runs.

```bash
# List stored contacts
fathom contacts list [--limit 50]

# Export contacts to a file
fathom contacts export --format csv|vcf|json|xlsx [--output <DIR>]

# Find and optionally merge duplicate contacts
fathom contacts dedup [--merge]

# Push all stored contacts to the configured CRM
fathom contacts push-crm
```

### Export formats

Contacts can be exported in four formats:
- **csv** — comma-separated values (spreadsheet-friendly)
- **vcf** — vCard format (address book import)
- **json** — structured JSON array
- **xlsx** — Excel spreadsheet

Export-time dedup is applied automatically: contacts with the same normalized email or phone are merged into their most complete row.

### CRM sync

When `[crm]` is configured in the config file, the `push-crm` command pushes all contacts to the external CRM. Contacts that were already synced earlier (carrying a `crm_id`) are skipped to avoid duplicates. The sync also happens automatically at the end of every research run.

---

## `config` — configuration management

View and modify the application configuration.

```bash
# Show config
fathom config show

# Set a value
fathom config set <KEY> <VALUE>
```

### Examples

```bash
fathom config show
fathom config set llm.api_key "sk-..."
fathom config set agent.max_agents 30
fathom config set output.dir "./research-output"
```

Configuration is stored in TOML format. See [CONFIGURATION.md](CONFIGURATION.md) for the full schema.

---

## `profiles` — persona/profile management

List, view, or create task presets that alter agent behaviour.

```bash
# List available profiles (built-ins + user files)
fathom profiles list

# Show one profile's definition
fathom profiles show <name>

# Create a template profile file
fathom profiles new <name>
```

### Built-in profiles

| Profile | Use case |
|---------|----------|
| `hunter` | Aggressive OSINT / lead generation — broad search, low threshold |
| `analyst` | Deep analysis — thorough, structured, citation-heavy |
| `validator` | Fact-checking and verification — cautious, cross-references sources |

User-created profiles live in `~/.fathom/profiles/` as `.toml` files.

### Examples

```bash
# List all available profiles
fathom profiles list

# Inspect the hunter profile definition
fathom profiles show hunter

# Create a new profile template
fathom profiles new my-persona

# Use a profile with a research run
fathom run --profile analyst "Analyze Q3 market trends for electric vehicles"

# Use a profile in the interactive TUI
fathom tui --profile hunter

# Custom user-created profile
fathom run --profile my-persona "Research top AI startups in Europe"

# Profile from an explicit path
fathom run --profile ~/.fathom/profiles/custom-brief.toml "Brief on quantum computing"
```

---

## `bench` — tool-execution benchmarks

Benchmark the tool-execution layer without LLM or network calls. Useful for performance tuning and regression testing.

```bash
fathom bench [--scenario all] [--n 16] [--save <PATH>]

Options:
  -s, --scenario <SCENARIO>  Scenario: all | dispatch | parallel-io |
                              parallel-cpu | mixed | parse-scale |
                              extract-json | feed-parse | code-map
  -n <N>                     Number of parallel calls / data files
  --save <PATH>              Write the markdown report to a file
```

---

## `stats` — tool-call statistics

Show tool-call statistics for a recorded session (e.g. how many times each tool was called, average duration, success rate).

```bash
fathom stats [--output <DIR>]
```

---

## Typical scenarios

### Academic research

```bash
fathom run \
  "Research the state of quantum computing in 2026: key players, technologies, outlook" \
  --output ./quantum/
```

### Lead generation (OSINT)

```bash
fathom run \
  "Find 20 SaaS companies in Berlin (Series A-B). Collect emails of founders and CTOs." \
  --output ./berlin-saas/
```

### Competitor analysis

```bash
fathom run \
  "Compare Notion's 5 main competitors: features, pricing, target audience" \
  --output ./competitors/
```

### Scheduled monitoring (watch mode)

```bash
# Check for new job postings at target companies every 4 hours
fathom run \
  "Find new job postings at Acme Corp, TechCo, and Startup Inc" \
  --repeat 14400
```

### Background job with auto-retry

```bash
# Submit a heavy research task that runs detached
fathom jobs submit \
  "Analyze the top 50 AI startups from Crunchbase: funding, team size, tech stack" \
  --attempts 3

# Watch it live from another terminal
fathom jobs status a1b2c --watch 5

# Tail the log
fathom jobs logs a1b2c -n 100
```

### Resume interrupted session

```bash
# If a run was killed mid-flight, resume it:
fathom resume

# Or resume a specific session:
fathom resume --session-id abc123
```

### Automation via the HTTP API

```bash
# Start the server
fathom serve --port 8080 &

# Create a session
curl -X POST http://localhost:8080/api/v1/sessions \
  -H "Content-Type: application/json" \
  -d '{"query": "Research the AI agent market"}'

# Check status
curl http://localhost:8080/api/v1/sessions/<SESSION_ID>

# Fetch results
curl http://localhost:8080/api/v1/sessions/<SESSION_ID>/results
```