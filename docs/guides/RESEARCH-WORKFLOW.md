# Research workflow

A practical guide to running effective research sessions with Fathom — from writing a query to interpreting results, handling interruptions, and setting up recurring monitoring.

## Overview

Fathom runs a **multi-agent research pipeline**:

1. **Plan** — the coordinator decomposes your query into 2–5 subtasks
2. **Execute** — sub-agents run in parallel, each using tools (web search, extraction, verification, etc.)
3. **Goal Mode** — an LLM judge reviews findings against the original goal and fills gaps in up to `replan_rounds` iterations
4. **Synthesize** — all findings are combined into a final report
5. **Export & notify** — results are written to disk, optionally exported (PDF/HTML/JSON/DOCX) and pushed to notification channels

```
output/
├── index.md           # table of contents + metadata
├── summary.md         # final synthesis
├── sources.md         # list of sources
├── findings/          # findings per subtask
│   ├── finding-1.md
│   ├── finding-2.md
│   └── finding-3.md
├── report.html        # export (if configured)
├── contacts.csv       # contacts exported from this run (if any)
└── .research.db       # session SQLite database
```

---

## 1. Structuring queries

The quality of your research output depends heavily on how you formulate the query. The coordinator's planner reads the query and decomposes it into parallel subtasks, so specificity and structure matter.

### Principles

| Principle | Bad | Good |
|-----------|-----|------|
| **Be specific** | "Research AI companies" | "Analyze the top 10 AI startups in Berlin that raised Series A in 2025–2026: their market focus, team size, and funding" |
| **Name roles** | "Find contacts at Acme" | "Find the CEO, CTO, and Head of Sales at Acme — extract emails and LinkedIn profiles" |
| **State scope** | "Tell me about Rust" | "Compare Rust, Go, and C++ for building CLI tools: performance, ecosystem, and developer experience" |
| **Include output expectations** | "Research competitors" | "Find 15 competitors in the SaaS analytics space. For each: name, website, funding stage, and employee count. Output as a table." |

### Target-count hints

When the query includes a numeric target (e.g. "10 companies", "5 emails", "20 contacts"), the coordinator detects it as a **lead-gen** task type and runs a post-execution gap analysis: if the session didn't harvest enough contacts, an extra reflection round spawns targeted agents to fill the gap.

```bash
fathom run "Find 10 emails of CTOs at fintech startups in London" --output ./london-fintech/
```

The planner sees `10` and `emails` → sets `TaskType::LeadGen` with `target_count: Some(10)`. After the main run, if only 4 contacts were saved, a reflection agent reviews the shortfall and proposes gap-filling subtasks.

### Long tasks from files

For complex instructions with multiple paragraphs, quotes, or newlines, use `--task-file` instead of a positional argument:

```bash
cat > ./tasks/competitor-analysis.txt << 'EOF'
Research the top 8 competitors in the AI code review space.

For each competitor:
1. Company name, founding year, HQ location
2. Core product description and pricing model
3. Key integrations (CI platforms, VCS)
4. Team size and funding history
5. Recent news or product launches (last 6 months)

Output format: markdown table with competitor name as anchor heading.
Include a competitive landscape summary at the end.
EOF

fathom run --task-file ./tasks/competitor-analysis.txt --output ./research/code-review/
```

The file content fully replaces the positional query argument.

---

## 2. Choosing profiles

A **profile** (also called a persona) is a TOML preset that injects a system prompt and can override model, temperature, `max_depth`, `max_agents`, `replan_rounds`, `timeout_seconds`, and `deny_tools`. Profiles let you tune the agent's behaviour for different kinds of research without passing flags every time.

### Built-in profiles

| Profile | Best for |
|---------|----------|
| `hunter` | OSINT, lead generation, contact harvesting — aggressive search, broad tool access |
| `analyst` | Market research, competitive analysis, structured reports — balanced, analytical |
| `validator` | Fact-checking, verification, quality assurance — conservative, high-rigour |

```bash
# List available profiles
fathom profiles list

# Inspect a profile's system prompt and settings
fathom profiles show hunter
```

### Creating custom profiles

```bash
# Generate a template in ~/.fathom/profiles/
fathom profiles new my-profile
```

A profile is a TOML file:

```toml
# ~/.fathom/profiles/deep-research.toml
system_prompt = """
You are a thorough research analyst. Always verify claims across multiple sources.
Cite sources explicitly. Flag uncertainty. Produce structured markdown reports.
"""
model = "gpt-4o"
temperature = 0.3
max_depth = 3
max_agents = 8
replan_rounds = 2
timeout_seconds = 300
deny_tools = ["save_contacts"]
```

### Using profiles

```bash
# Run with a built-in profile
fathom run --profile hunter "Find DM at Acme Corp"

# Run with a custom profile
fathom run --profile deep-research "Analyze the RAG market"

# Run with a profile from a specific path
fathom run --profile ./my-profiles/vendor-analysis.toml "..."

# Use a profile in the TUI
fathom tui --profile analyst
```

### Profile selection strategy

| Query type | Recommended profile | Reason |
|------------|-------------------|--------|
| Contact harvesting / OSINT | `hunter` | Broad tool access, aggressive search behaviour, higher replan rounds |
| Market analysis / reports | `analyst` | Balanced temperature, structured output, good for synthesis |
| Fact-checking / verification | `validator` | Lower temperature, conservative, multiple verification passes |
| Deep multi-topic research | Custom (high `max_agents`, `replan_rounds: 2–3`) | Wide exploration, multiple gap-filling rounds |
| Quick single-fact lookup | `--profile analyst` or default | Minimal overhead, one or two agents |

---

## 3. Interpreting results

### Output structure

After a run completes, the output directory contains:

#### `summary.md`
The final synthesis — a complete report written by the coordinator's LLM, combining all findings. This is the primary deliverable.

#### `index.md`
Table of contents with session metadata: query, profile used, number of agents, total tokens, timestamps.

#### `findings/`
One file per subtask agent. Each finding contains the agent's raw output — tool call results, extracted data, and reasoning. Use these to understand *how* the conclusion was reached.

```
findings/
├── finding-1.md   # Companies research
├── finding-2.md   # Contact extraction
├── finding-3.md   # Verification results
└── finding-4.md   # Market analysis
```

#### `sources.md`
Aggregated list of all URLs and sources referenced during the run.

#### `contacts.csv` (OSINT runs)
Exported contacts from the session, one per row, with confidence scores.

### Reading the session log

Use `RUST_LOG=debug` to see the coordinator's reasoning during planning, Goal Mode judging, and synthesis:

```bash
RUST_LOG=debug fathom run "..." --output ./out/
```

Key log lines to look for:

```
# Planning phase
INFO  coordinator: decomposing query into N subtasks
INFO  coordinator: task type = LeadGen, target count = 10

# Goal Mode
INFO  coordinator: goal mode: replan round 1 — 2 gap-filling task(s)
INFO  coordinator: goal mode: goal satisfied before round 2

# Synthesis
INFO  coordinator: synthesizing 5 findings into final report
```

### Session stats

```bash
# View session history
fathom sessions list -n 10

# Inspect a specific session (agents, findings, token usage)
fathom sessions show <session-id-prefix>

# Detailed tool-call statistics
fathom stats <session-id-prefix>
```

`fathom stats` shows per-tool call counts, average duration, success rates, and token consumption — useful for identifying bottlenecks (e.g. a tool that consistently times out or an agent that consumed disproportionate tokens).

---

## 4. Using Goal Mode

Goal Mode is the quality assurance loop that runs after the initial agent fan-out completes. It dramatically improves result completeness for complex queries.

### How it works

```
Initial plan → agents execute → Goal Mode judge reviews findings
    ├── goal satisfied → proceed to synthesis
    └── gaps remain → propose up to 3 gap-filling subtasks
                        → spawn new agents → re-evaluate
                                └── repeat up to replan_rounds times
```

The judge is an LLM call that receives the original query and all collected findings, then produces a JSON verdict:

```json
{"complete": false, "new_subtasks": ["Find pricing details for Acme", "Verify CTO email for 3 companies"]}
```

or:

```json
{"complete": true, "new_subtasks": []}
```

### Configuring Goal Mode

```toml
# config.toml
[agent]
replan_rounds = 2   # default: 1, set to 0 to disable
max_agents = 10     # cap on total agents (including gap-fillers)
```

Or per-profile:

```toml
# ~/.fathom/profiles/deep-research.toml
replan_rounds = 3
max_agents = 12
```

### When to use Goal Mode

| Scenario | replan_rounds | Why |
|----------|--------------|-----|
| Quick fact lookup | 0 | No need for gap-filling; one pass is enough |
| Standard research | 1 (default) | One quality check catches obvious gaps |
| Lead generation | 2–3 | Partial results are the norm; each round fills missing contacts |
| Competitive analysis | 2 | Multiple dimensions (product, funding, team, news) — easy to miss one |
| Deep investigation | 3 | Maximum coverage, tolerates longer runtime |

### Monitoring Goal Mode

In the TUI, Goal Mode rounds appear as new agent spawns after the initial batch completes. The event log shows:

```
• Goal Mode judge: evaluating 4 findings
• Goal Mode: 2 gaps found — spawning 2 gap-filling agents
• Goal Mode round 1: agents running
• Goal Mode judge: re-evaluating...
• Goal Mode: goal satisfied after 1 round
```

### Budget gates

Goal Mode respects two hard limits and will not spawn gap-filling agents when either is exhausted:

1. **Token budget** — the session's configured token window is consumed by each agent; once exhausted, replanning stops
2. **Agent budget** — `max_agents` caps the total number of agents per session, including gap-fillers

Both limits are logged at the `info` level when hit:

```
INFO  coordinator: replan skipped: max_agents reached
INFO  coordinator: replan skipped: session token budget exhausted
```

---

## 5. Session resume

If a session is interrupted (Ctrl+C, crash, timeout), it can be resumed without redoing completed work.

### How resume works

The coordinator persists subtasks and their completion status to the session's SQLite database (`.research.db`). On resume:

1. The database is loaded from the output directory
2. Completed sub-agents' findings are preserved
3. The coordinator identifies which sub-agents did **not** complete
4. Only unfinished sub-tasks are re-run
5. The session is finalized as usual (synthesis, export, notifications, CRM sync)

### Usage

```bash
# Resume the most recent interrupted session
fathom resume

# Resume a specific session
fathom resume --session-id abc123

# Resume with a custom output directory
fathom resume --output ./research/ --session-id abc123
```

### Example workflow

```bash
# Start a long research session
fathom run "Deep research: 50 biggest AI startups in Europe" --output ./ai-europe/

# Session is interrupted (Ctrl+C, power loss, timeout)
# Later, resume it:
fathom resume --output ./ai-europe/

# Output: the session picks up where it left off.
# Completed findings remain, unfinished ones re-run.
```

### When resume is useful

- **Long-running sessions** that hit timeouts or LLM rate limits
- **Network interruptions** during web-scraping-heavy runs
- **Iterative refinement** — run a quick first pass, inspect partial results, then resume to continue
- **CI/CD pipelines** where a research job is killed by a deployment and needs to pick up

### Limitations

- Resume works only if the output directory's `.research.db` is intact
- The session's tool results are **not** cached — gap-filling agents will re-execute their tools (but the coordinator's persisted findings from completed agents are preserved)
- The model configuration must be the same as the original run (model changes may cause subtle differences in gap-filling behaviour)

---

## 6. Memory digest

The long-term semantic memory system stores facts from past sessions and makes them available to future runs. At session start, the top-level coordinator agent receives a **memory digest** — a deterministic summary of relevant memories, open TODOs, and recent records.

### How the digest works

1. The coordinator's query is used to hybrid-search the memory store (cosine similarity + BM25)
2. Top relevant facts are bundled under a `## Long-term memory digest` header
3. The digest is injected into the coordinator's system prompt (volatile tier)
4. The coordinator can use `memory_search` to pull more detail or `memory_absorb` to save new facts

```
## Long-term memory digest (topic: "AI startups in Europe")

Relevant memories:
- Acme Corp raised $50M Series B in March 2026 (memory: a1b2c3d4)
- Beacon AI is headquartered in Berlin, ~200 employees (memory: e5f6g7h8)
- CTO of Acme Corp is Jane Smith (memory: i9j0k1l2)

Open TODOs:
- Verify email for Acme CTO

Recent records (last 24h):
- Phone number extraction for 3 London startups (memory: m3n4o5p6)
```

### Memory tools available to agents

| Tool | Purpose |
|------|---------|
| `memory_absorb` | Save new facts (scope: agent/user/run) |
| `memory_search` | Hybrid search or read by ID |
| `memory_digest` | Get a digest for any topic mid-session |
| `memory_boost` | Raise importance of a useful record |
| `memory_link` | Create a typed edge between two records |
| `memory_graph` | Entity graph operations (add/query entities and relations) |

### CLI memory management

```bash
# Search stored knowledge
fathom memory search "Acme CEO email" --top-k 10

# List recent memories
fathom memory list --scope run -n 20

# Get a specific record with version history
fathom memory get a1b2c3d4 --follow latest

# Distill session facts into durable knowledge
fathom memory distill --session abc123

# View memory statistics
fathom memory stats

# Rebuild embeddings after changing the model
fathom memory rebuild
```

### Distillation workflow

After a research session, run-scoped facts can be distilled into durable `agent`-scope knowledge:

```bash
# Dry-run to see what would be distilled
fathom memory distill --dry-run

# Actually distill
fathom memory distill
```

This promotes valuable findings (verified contacts, company facts, market insights) from ephemeral session context into the permanent knowledge base, so future runs benefit from past discoveries without re-searching.

### Example: knowledge compounding across sessions

```bash
# Session 1: initial research
fathom run "Find 10 AI startups in Berlin" --output ./berlin-ai/

# Distill findings into memory
fathom memory distill

# Session 2: related research — coordinator sees existing knowledge
fathom run "Find CTO contacts at AI startups in Berlin" --output ./berlin-ctos/
# The digest includes: "Startup X raised €5M" (from session 1)
# Agent doesn't re-research the company — it goes straight to finding the CTO
```

---

## 7. Watch mode

Watch mode (`--repeat`) runs the same query on a schedule and reports only what changed since the last run.

### How it works

```bash
fathom run "Find contacts at Acme Corp" --repeat 21600   # every 6 hours
```

1. The first run executes normally — full research, full output
2. After `--repeat` seconds, the query runs again
3. Contact results are compared against the previous run's contact database
4. **Only new** contacts (emails, phones, personas not seen before) trigger alerts
5. Alerts are sent via configured notification channels (webhook, Telegram, email)
6. The loop continues until interrupted (Ctrl+C)

### Notification events

| Event | Trigger |
|-------|---------|
| `watch.new_contacts` | New contacts found in a watch-mode run |
| `session.completed` | Any session finishes successfully |
| `session.failed` | Any session crashes |

### Notification configuration

```toml
# config.toml
[notifications]
webhook_url = "https://hooks.slack.com/..."
telegram_bot_token = "..."
telegram_chat_id = "..."
```

### Watch mode use cases

| Use case | Repeat interval | Rationale |
|----------|----------------|-----------|
| Competitor monitoring | 86400 (24h) | Daily check for new team members, executive changes |
| Lead generation | 21600 (6h) | Catch new business listings, updated contact pages |
| Job board monitoring | 3600 (1h) | Track new openings for market intelligence |
| News monitoring | 43200 (12h) | Twice-daily check for new mentions |
| CRM enrichment | 604800 (7d) | Weekly pass to find missing contacts for existing accounts |

### Example: competitor people tracking

```bash
fathom run \
  "Find new team members, executives, and engineering leads at Acme Corp, Beacon AI, and CloudSync. \
   Extract emails and LinkedIn profiles." \
  --output ./watch/competitors/ \
  --repeat 86400
```

First run: full extraction of all known contacts. Subsequent runs: only **new** hires or newly discovered contacts trigger alerts. The diff is printed to stdout and pushed through notification channels.

### Watch mode with Goal Mode

Watch mode and Goal Mode compose naturally:

```bash
fathom run --profile hunter \
  "Find contacts of 5 new CTOs at European fintech companies (not already in my database)" \
  --output ./watch/fintech-ctos/ \
  --repeat 43200
```

The `hunter` profile with its default `replan_rounds` ensures each watch cycle does thorough gap-filling, so you don't miss a contact just because the first pass was incomplete.

### Stopping watch mode

Watch mode runs until interrupted. Use `Ctrl+C` to stop. The contact identity set is persisted between runs, so resuming later with the same `--output` directory picks up where it left off without re-alerting on already-known contacts.

---

## 8. End-to-end workflow examples

### Quick investigation

```bash
# One-shot fact-finding
fathom run "What is the RAG retrieval-augmented generation market size in 2026?"
```

### Multi-stage research with distillation

```bash
# Stage 1: broad market research
fathom run --profile analyst \
  "Research the top 20 AI infrastructure companies: products, funding, team size" \
  --output ./research/ai-infra/

# Stage 2: distill findings into long-term memory
fathom memory distill

# Stage 3: targeted follow-up on one segment
fathom run --profile hunter \
  "Find contacts (CEO, CTO, VP Eng) at the top 5 data-labeling companies from previous research" \
  --output ./research/data-labeling-leads/
```

### Competitive intelligence (watch + memory)

```bash
# Initial setup
mkdir -p ./watch/competitors

# First run: full research
fathom run --profile analyst \
  "Monitor 10 competitors: Acme, Beacon, CloudSync, ... — track hiring, funding, product launches" \
  --output ./watch/competitors/

# Distill findings into knowledge base
fathom memory distill

# Set up recurring watch (every 12 hours)
fathom run --profile hunter \
  "Find new hires, executives, and contact changes at the 10 competitors" \
  --output ./watch/competitors/ \
  --repeat 43200
```

### Recovering from interruption

```bash
# Long session gets interrupted
fathom run "Deep research on 50 European AI startups" --output ./research/ai-europe/
# Ctrl+C, or timeout

# Resume later
fathom resume --output ./research/ai-europe/

# If resume also interrupted, resume again — it's idempotent
fathom resume --output ./research/ai-europe/
```

### Batch lead generation with Goal Mode

```bash
fathom run --profile hunter \
  "Find 20 IT companies in London with 50-200 employees. \
   Extract CEO and CTO emails, phone numbers, LinkedIn profiles. \
   Focus on fintech and SaaS companies." \
  --output ./leads/london-it/ \
  --task-file ./tasks/lead-gen-instructions.txt
```

With `replan_rounds = 2` (default for `hunter`), if the first pass only finds 12 contacts, Goal Mode detects the gap and sends targeted agents to find the remaining 8.

---

## 9. Interactive TUI workflow

The TUI provides real-time visibility into the research pipeline.

### Starting a session

```bash
# Start TUI with a query
fathom tui "Research the AI agent market"

# Start TUI with a profile
fathom tui --profile analyst

# Replay a past session (no live run)
fathom tui --replay abc123
```

### During a run

| Panel | Shows |
|-------|-------|
| **Agents** | Tree of running agents with status indicators (○ pending, ◐ running, ✓ done, ✗ failed) |
| **Output / Thinking** | Streaming LLM responses, thinking blocks, and final findings |
| **Tools** | Tool calls (→ running, ✓ done, ✗ failed) with elapsed time |
| **Event Log** | Session lifecycle events: spawns, completions, Goal Mode rounds |
| **Memory** | Recent facts from the knowledge base (if `[memory] enabled = true`) |
| **Jobs** | Background jobs status (if `fathom jobs submit` was used) |

### Key controls

| Key | Action |
|-----|--------|
| `i` | Enter input mode (type a new query) |
| `Enter` | Send the query |
| `Shift+Enter` | New line in input |
| `Esc` | Exit input mode |
| `Tab` | Switch between panels |
| `↑` / `↓` | Scroll / navigate agent tree |
| `←` / `→` | Collapse / expand agent sub-tree |
| `t` | Toggle thinking panel (show/hide model reasoning) |
| `y` / `n` | Approve / reject a pending side-effect tool |
| `c` | Clear output panel |
| `?` | Show key help |
| `q` | Quit |

### Approval flow

When an agent wants to run a side-effect tool (e.g. `save_contacts`, `git_push`), the TUI pauses and shows a prompt. Press `y` to approve or `n` to reject. Configure which tools require approval in `[agent] approval_tools`. If you step away, `approval_fallback` (default `allow` or `deny`) kicks in after `approval_timeout_seconds`.

### Agent questions

Agents can ask you questions via the built-in `question` tool — for example, when an ambiguous goal needs clarification, or when a choice between two materially different directions arises. The TUI surfaces the question with a reply field, and your answer is delivered back to the agent.

---

## 10. Troubleshooting common issues

| Issue | Likely cause | Fix |
|-------|-------------|-----|
| "No findings produced" | Query too vague or model refused to decompose | Add structure and specificity. Use `--task-file` for complex instructions |
| Goal Mode not running | `replan_rounds = 0` or token budget exhausted | Check config: `[agent] replan_rounds` should be ≥ 1 |
| Resume finds no session | Wrong output directory or `.research.db` missing | Pass `--output` pointing to the original session directory |
| Watch mode not alerting | No notification channels configured | Set `[notifications]` in config |
| Memory digest empty | Memory subsystem not enabled or no relevant facts | Enable `[memory] enabled = true` and run `fathom memory distill` |
| Agents stuck on tool | Tool timeout or network issue | Check `RUST_LOG=debug` output. Increase `timeout_seconds` in profile |
| "replan skipped: max_agents reached" | Too many agents in initial plan, leaving no room for gap-fillers | Increase `max_agents` in config or profile |
| Contact dedup not working | Contacts from different sessions have slightly different names | Use `fathom contacts dedup --merge` to merge duplicates automatically |

---

## Reference: CLI commands used in this guide

| Command | Purpose |
|---------|---------|
| `fathom run <query>` | Run a headless investigation |
| `fathom run --profile <name>` | Run with a persona profile |
| `fathom run --task-file <path>` | Run with a task from a file |
| `fathom run --repeat <secs>` | Watch mode — repeat every N seconds |
| `fathom run --output <dir>` | Specify output directory |
| `fathom tui [query]` | Interactive TUI |
| `fathom tui --replay <id>` | Replay a past session in the TUI |
| `fathom resume` | Resume the most recent interrupted session |
| `fathom resume --session-id <id>` | Resume a specific session |
| `fathom sessions list` | Browse past sessions |
| `fathom sessions show <id>` | Inspect a session's agents and findings |
| `fathom stats <id>` | Tool-call statistics for a session |
| `fathom profiles list` | List available profiles |
| `fathom profiles show <name>` | Inspect a profile's configuration |
| `fathom profiles new <name>` | Create a custom profile template |
| `fathom memory search <query>` | Search the semantic memory store |
| `fathom memory list` | List memories |
| `fathom memory distill` | Distill session facts into durable knowledge |
| `fathom memory stats` | Memory statistics |
| `RUST_LOG=debug fathom run ...` | Verbose logging for debugging |