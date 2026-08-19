# CLI Usage

The `parallel-research` binary has 5 commands: `run`, `worker`, `tui`, `serve`, `config`.

```
parallel-research <COMMAND>

Commands:
  run     Run an investigation (headless)
  worker  Worker mode (internal, for multi-process)
  tui     Interactive TUI
  serve   HTTP API server
  config  Configuration management
```

---

## `run` — headless investigation

The primary mode. Launches a coordinator that decomposes the request into sub-agents.

```bash
parallel-research run <QUERY> [OPTIONS]

Arguments:
  <QUERY>  Research query

Options:
  -o, --output <DIR>  Directory for results (default: from config)
  --profile <NAME>    Persona: hunter | analyst | validator | file in
                      ~/.parallel-research/profiles/ | path to .toml
  --repeat <SECS>     Watch mode: repeat the request every N seconds
```

### Personas (`--profile`)

A preset adds a system prompt and can override model/fast_model,
temperature, max_depth, max_agents and deny_tools:

```bash
parallel-research profiles list
parallel-research profiles show hunter
parallel-research profiles new my-persona   # template in profiles/
parallel-research run --profile hunter "find DM at Acme"
```

### Watch mode (`--repeat`)

Each run is compared against the previous one against the contacts database: new emails /
phones / personas are printed as a diff and sent out as an alert via the
`[notifications]` channels (webhook / Telegram / email, event `watch.new_contacts`).
Session completion/crash notifications also work (`session.completed`,
`session.failed`).

```bash
parallel-research run "Acme team" --repeat 21600   # every 6 hours
```

### Examples

```bash
# Simple investigation
parallel-research run "What are quantum computers?"

# With output directory specified
parallel-research run "Compare Rust and Go" --output ./research/

# OSINT / LeadGen
parallel-research run \
  "Find contacts of CEOs of IT companies in Moscow. Extract emails and phone numbers." \
  --output ./leads/
```

### What happens

1. The coordinator plans: decomposes the request into 2-5 subtasks
2. Launches sub-agents (in parallel)
3. Agents use tools (web_search, extract_contacts, ...)
4. The coordinator synthesizes the results
5. Writes `index.md`, `summary.md`, `findings/`, `sources.md`
6. Exports (PDF/HTML/JSON/DOCX) and sends notifications

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
└── .research.db       # Session SQLite database
```

### Logging

```bash
# Verbose logs
RUST_LOG=debug parallel-research run "..."

# Errors only
RUST_LOG=error parallel-research run "..."
```

---

## `tui` — interactive mode

Terminal interface built on ratatui.

```bash
parallel-research tui [QUERY] [--profile <NAME>] [--replay <SESSION-ID>]

Arguments:
  [QUERY]  Optional initial query

Options:
  --profile <NAME>      Persona for sessions started from the TUI
  --replay <SESSION-ID> View a saved session (without a live run)
```

### Layout

```
┌───────────────────────────────────────────────────┐
│ Parallel Research | Session: xxx | 2:34 │ ███░ 67% │
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

---

## `serve` — HTTP API server

Launches an Axum server for programmatic control.

```bash
parallel-research serve [OPTIONS]

Options:
  --port <PORT>  Port (default: 8080)
  --host <HOST>  Bind address (default: 127.0.0.1)
```

```bash
parallel-research serve --port 8080
```

By default the server listens only on loopback. To bind to an external address
(`--host 0.0.0.0`) you **must** set `PARALLEL_RESEARCH_API_KEYS`,
otherwise startup will be rejected.

For API details — see [HTTP-API.md](HTTP-API.md).

---

## `mcp-serve` — MCP server

Exposes all agent tools to external MCP clients (Claude, ZCode, etc.)
over stdio. Actually executes `tools/call` via the shared `ToolRegistry`.

```bash
parallel-research mcp-serve
```

Connection in the MCP client config:
```json
{ "command": "parallel-research", "args": ["mcp-serve"] }
```

---

## `memory` — long-term memory

Manage the semantic knowledge base without running the agent.

```bash
# Hybrid search
parallel-research memory search "Acme CEO email" [--top-k 10] [--scope agent|user|run|all]

# List records
parallel-research memory list [--scope ...] [--status active|superseded|archived|all] [-n 20]

# Single record + version chain
parallel-research memory get <id> [--follow active|latest|full_history]

# Statistics (scopes, entity graph, DB size)
parallel-research memory stats

# Re-embedding after changing the model
parallel-research memory rebuild

# Distill session run-facts into durable knowledge
parallel-research memory distill [--session <key>] [--dry-run]

# GC: archiving expired/stale facts + N→1 compaction of groups
parallel-research memory gc [--ttl-days 30] [--dry-run]

# Full deletion of scope records (requires --yes)
parallel-research memory nuke --scope run --yes
```

Details — see [MEMORY-KB.md](MEMORY-KB.md).

---

## `sessions` — session history

```bash
# Recent sessions (search by query substring)
parallel-research sessions list [-n 20] [--search "kazan"]

# Details of a single session: agents + findings
parallel-research sessions show <id-or-prefix>
```

---

## `worker` — internal mode

Used by the coordinator in multi-process mode. **Do not run manually.**

```bash
parallel-research worker \
  --session-id <SID> \
  --agent-id <AID> \
  --task <TASK> \
  --socket <PATH> \
  --role <ROLE>
```

Activated automatically when `use_multiprocess = true`.

---

## `config` — configuration management

```bash
# Show config
parallel-research config show

# Set a value
parallel-research config set <KEY> <VALUE>
```

### Examples

```bash
parallel-research config show
parallel-research config set llm.api_key "sk-..."
parallel-research config set agent.max_agents 30
```

---

## Typical scenarios

### Academic research

```bash
parallel-research run \
  "Research the state of quantum computing in 2026: key players, technologies, outlook" \
  --output ./quantum/
```

### Lead generation (OSINT)

```bash
parallel-research run \
  "Find 20 SaaS companies in Berlin (Series A-B). Collect emails of founders and CTOs." \
  --output ./berlin-saas/
```

### Competitor analysis

```bash
parallel-research run \
  "Compare Notion's 5 main competitors: features, pricing, target audience" \
  --output ./competitors/
```

### Automation via the HTTP API

```bash
# Start the server
parallel-research serve --port 8080 &

# Create a session
curl -X POST http://localhost:8080/api/v1/sessions \
  -H "Content-Type: application/json" \
  -d '{"query": "Research the AI agent market"}'

# Check status
curl http://localhost:8080/api/v1/sessions/<SESSION_ID>

# Fetch results
curl http://localhost:8080/api/v1/sessions/<SESSION_ID>/results
```