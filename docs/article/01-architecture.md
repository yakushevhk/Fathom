# 01. Architecture

## Workspace: 8 crates

```
fathom (binary, src/main.rs + src/bench.rs)
├── pr-core          — domain types with no logic dependencies:
│                      Session, AgentId, Message, ToolCall, ToolOutput,
│                      Finding, Contact, AppConfig, errors
├── pr-llm           — LLM providers: native DeepSeek + OpenAI-compatible
│                      (via factory), retries, streaming, token accounting
├── pr-agent         — the brain of the system: Coordinator, Runtime (agent loop),
│                      ToolExecutor (batching and parallelism), prompts,
│                      context budget, compaction, doom-loop protection
├── pr-tools         — 38 built-in tools + registry + SSRF guard +
│                      anti-injection + file locks + fetch cache
├── pr-mcp           — MCP client: dynamic connection of external tools
├── pr-persistence   — SQLite: session database (.research.db), contact
│                      storage (contacts.db), finding history
├── pr-server        — axum HTTP API + SSE events
└── pr-tui           — terminal interface on ratatui
```

Dependencies point strictly downward: `pr-tools` does not know about `pr-agent`,
`pr-core` does not know about anyone. This allows benchmarking the tool layer in
isolation (the `bench` command uses only `pr-tools` + `pr-agent::ToolExecutor`).

## Request lifecycle

```
CLI: fathom run "query"
        │
        ▼
┌─────────────────────────────────────────────────────────────┐
│ Coordinator (pr-agent/coordinator.rs)                       │
│  1. Planning: LLM decomposes the query into 3–5 subtasks    │
│  2. Parallel spawn of researcher agents                     │
│  3. Collect findings from all agents                        │
│  4. Synthesis: LLM assembles summary.md from facts          │
│  5. Write: summary.md, findings/, sources.md, .research.db  │
└─────────────────────────────────────────────────────────────┘
        │ each agent is a background tokio task
        ▼
┌─────────────────────────────────────────────────────────────┐
│ AgentRuntime (pr-agent/runtime.rs)                          │
│  loop: LLM → tool calls → execution → results → LLM …      │
│                                                             │
│  Phase 1: intercept instantaneous results                   │
│  Phase 2: execute batch (spawn parallel + sequential)       │
│  Phase 3: write results and harvest findings                │
└─────────────────────────────────────────────────────────────┘
```

### Three phases of tool batch execution

Each LLM response may contain multiple tool calls — they are executed as a
**batch**, not one by one:

1. **Phase 1 — planning/interception.** Deduplication of calls that overlap on
   file paths; cache of "instantaneous" results (e.g., repeated
   `scratchpad` with the same arguments is not re-executed).
2. **Phase 2 — execution.** `ToolExecutor` splits the batch into parallel-safe
   calls (sent to `tokio::spawn` and genuinely distributed across cores) and
   sequential calls (executed strictly one at a time). Details —
   in [02-tool-calling.md](./02-tool-calling.md).
3. **Phase 3 — recording.** Each result with its duration is written to SQLite
   (`tool_results`), successful search/parse calls are automatically
   turned into `Finding` with sources (`harvest_finding`).

## Agent model

- **Coordinator** — one per session; plans, spawns, synthesizes.
- **Researcher** — worker agents; each receives a subtask, token budget,
  and the full set of tools. Agents do not communicate directly — only through
  the shared scratchpad and findings.
- **Hierarchical spawn** — an agent can spawn a sub-agent via the
  `spawn_agent` tool (depth is controlled by the runtime).
- **Roles in prompts**: base system prompt (`prompts/default.txt`) and a
  compact version for reasoning models (`prompts/deepseek.txt`, shorter —
  this is a test-invariant).

## Guardrails

| Mechanism | Where | What it does |
|---|---|---|
| SSRF guard | `pr-tools/guard.rs` | Blocks requests to internal addresses; checked on **every** redirect hop |
| Anti-injection | `pr-tools/injection.rs` | Filters web page content before feeding it into context |
| File locks | `pr-tools/file_lock.rs` | Parallel agents do not write to the same file simultaneously |
| Shell cascade | `ToolExecutor` | A failed `shell` cancels sibling batch calls |
| Doom-loop detector | `pr-agent/doom_loop.rs` | Detects repeating meaningless calls and stops the agent |
| Context compaction | `pr-agent/compaction.rs` | Compresses history when the context blows up |
| Token budget | `pr-agent/budget.rs` | An agent cannot burn more than allocated |
| Recovery/Resume | `pr-agent/resume.rs`, `resume.rs` | Session can be restarted after a crash |
| Truncation retry | `runtime.rs` | If a reasoning model spent the entire budget on thinking and returned empty content — retry with an increased budget (up to 2 times) and a deterministic fallback |

## Reasoning model support

A separate story discovered through live testing: DeepSeek family models
return a chain of thought in the `reasoning_content` field, and these tokens
**are subtracted from `max_tokens`**. If the budget is small, the model returns
empty `content` with `finish_reason: "length"` — and a naive client gets empty
reports.

The solution spans three places:

1. `pr-llm/deepseek.rs` — parsing `reasoning_content` + diagnostic
   warning on empty content;
2. `coordinator.rs::synthesize()` — synthesis budget no lower than 16,384, retry
   with doubling (up to 32,768), deterministic fallback report from findings;
3. `runtime.rs` — up to 2 retries of the main loop with a hint to the model
   "budget exhausted, write the answer directly".

## Persistence

Each session leaves behind:

```
<output-dir>/
├── .research.db      — SQLite: sessions, agents, messages, tool_results,
│                       findings, subtasks (full execution trace)
├── summary.md        — synthesized report
├── findings/         — discovered facts, one per file
├── sources.md        — all sources (URLs, titles, snippets)
└── index.md          — session manifest
```

OSINT task contacts go into a separate `contacts.db` (schema with migrations,
phone normalization, deduplication, optional CRM push).

It is `.research.db` that is the data source for the `stats` command and all
figures in [03-benchmarks.md](./03-benchmarks.md).

## What to read next

- Batch mechanics and parallelism: [02-tool-calling.md](./02-tool-calling.md)
- Benchmarks: [03-benchmarks.md](./03-benchmarks.md)
- Parsing: [04-parsing.md](./04-parsing.md)
- Real-world cases: [05-case-studies.md](./05-case-studies.md)