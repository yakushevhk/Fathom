# fathom: article documentation

A complete breakdown of a multi-agent research tool in Rust: architecture,
tool-calling mechanics, parallel execution, benchmarks, and real-world case studies with actual LLMs.

The documentation is written as a source for the article: each file is self-contained,
contains concrete numbers and reproducible commands.

## Why this series exists

`fathom` grew out of a simple observation: an agent is only as good
as the machinery between the LLM's intent and the world. Most write-ups about
AI agents stop at prompts and tool lists; the interesting engineering — how a
tool call becomes threads, how a batch of calls is partitioned, how a 0-byte
report is diagnosed from a SQLite trace — stays invisible. This series is
written to a different standard: every claim is backed by a reproducible
command, a measured number, or a live session artifact, and each file is
self-contained so it can stand alone as an article or be read in sequence as a
thesis. The running theme is that these are not toy demos: the figures come
from a real, working binary that ran real LLM workloads against a real
10-core machine.

The articles track the natural order in which you would build and understand
such a system: **what the system is** (architecture), **how the hot path works**
(tool calling), **how fast it is and why** (benchmarks), **how tricky data
extraction is made safe** (parsing), and finally **what breaks in the wild**
(case studies). Read together, they form the full argument of the project: a
research agent's value is decided by its execution layer, and that layer can —
and should — be measured, guarded, and fixed like any other performance-critical
code.

## Contents

| File | About |
|---|---|
| [01-architecture.md](./01-architecture.md) | Architecture: 8 crates, research pipeline, agent model, guardrails |
| [02-tool-calling.md](./02-tool-calling.md) | Tool-calling mechanics: registry, batches, partitioning, join_all vs spawn parallelism |
| [03-benchmarks.md](./03-benchmarks.md) | Benchmarks: dispatch, parallel batches, parsing, live session statistics |
| [04-parsing.md](./04-parsing.md) | Parsing guide: `parse_html`, `extract_json`, SSRF protection, cache |
| [05-case-studies.md](./05-case-studies.md) | Six real-world case studies with actual LLM: coding, parsing, OSINT, research; bugs found |

### How to read the series

Each file answers one question, and the answers build on each other:

- **[01-architecture.md](./01-architecture.md)** answers *what is this system?*
  It lays out the 8-crate workspace with its strict downward dependency rule,
  the request lifecycle (plan → spawn → research → synthesize → persist), the
  agent model (coordinator + parallel researchers + hierarchical sub-agents),
  and the full guardrail stack. It is the map for every other article: the
  `ToolExecutor` it mentions is dissected in 02, its numbers are measured in 03,
  the parser tools it lists are detailed in 04, and the guardrails are exactly
  what the live runs in 05 stress-tested. Start here if you want the big picture
  before the details.

- **[02-tool-calling.md](./02-tool-calling.md)** answers *how does a tool call
  actually travel from JSON in the LLM response to execution on cores?* It is a
  deep dive into the single hottest path in the system, from the `Tool` trait
  and `schemars`-generated schemas through the three-phase batch executor to the
  `tokio::spawn` parallelism. Its central insight — that `futures::join_all`
  gives concurrency but not parallelism, because CPU-bound tool futures share
  one thread — is the intellectual heart of the whole project and the reason
  `execute_batch_spawn` exists. It also explains the safety mechanics (shell
  cascade, path dedup, result-order guarantees, `{e:#}` error chains) that make
  parallel execution safe enough for real agents.

- **[03-benchmarks.md](./03-benchmarks.md)** answers *is it fast, and why?*
  It documents the built-in `fathom bench` command and its
  methodology (fresh fixtures per run, warm-up calls, median/mean over several
  iterations), then presents the dispatch overhead, the I/O and CPU batch
  speedups, the parser throughput tables, and — most valuably — the two real
  performance bugs the benchmark itself found and fixed (the 31× `repo_map`
  regex regression and the silent serialization of unclassified tools). The
  moral is that the benchmark is a development tool, not a showcase: it exists
  because the numbers in 02's argument ("spawn over join_all") needed to be
  proven, and it pays for itself by catching regressions that would otherwise
  vanish without a trace.

- **[04-parsing.md](./04-parsing.md)** answers *how is web data turned into
  structured findings?* It explains why the two tools `parse_html` and
  `extract_json` were added at all (flat-text `web_fetch` could not answer
  "top-10 posts" or "extract the price table"), documents their parameters and
  the two selector semantics (target vs region modes), and shows the shared
  secured fetch infrastructure: session cache, SSRF guard on every redirect
  hop, a 2 MB body limit, and typed error codes the agent can act on. It ties
  back to 03 for measured throughput and to 01 for how every successful parse is
  harvested into a `Finding`.

- **[05-case-studies.md](./05-case-studies.md)** answers *what actually breaks
  in production?* Six live sessions against a real reasoning LLM — including
  the failures — show the empirical loop the whole project runs on: live run →
  anomaly in the SQLite trace → diagnosis → fix with regression test → rerun.
  This is where 01's guardrails and 02's mechanics get proven or broken:
  the empty-report bug is a `max_tokens` interaction with `reasoning_content`,
  the contacts bug is a migration-order mistake, and the parallelism numbers
  confirm 03's claims at session scale (peak concurrency 15, 912 seconds
  saved). Read it last — it assumes all the vocabulary from the previous files
  and pays off the series' central argument: **the SQLite trace of every tool
  call is the main debugging tool.**

## What is this project

`fathom` is a CLI/TUI/HTTP tool that takes a natural language
query and executes it with a **team of parallel LLM agents**:

- planning and decomposition of the query into subtasks;
- parallel execution of research agents;
- **up to 38 built-in tools** (web search, HTML/JSON parsing, files, shell, git,
  Python/Node REPL, OSINT verification of emails/phones/social media, CRM sync);
- batched tool calling with true multi-threaded parallelism (`tokio::spawn`);
- protection: SSRF guard, prompt-injection filters, file locks, cancellation on shell failure;
- everything is written to SQLite: every tool call with duration — later analyzed
  via the `fathom stats` command.

The architectural principle behind all of it is strict separation with
downward-only dependencies: `pr-core` holds domain types, `pr-tools` knows
nothing about the agent, and the `bench` command can therefore exercise the
tool layer in isolation — with no LLM and no network — which is what makes the
numbers in 03 trustworthy and reproducible. The design decisions documented in
each article all serve one measurable goal: research wall-clock time. Planning
turns one query into parallelizable subtasks; batching turns N sequential
network calls into one overlapped batch; `execute_batch_spawn` turns CPU-bound
parsing across cores; guardrails make that parallelism safe instead of chaotic;
and SQLite tracing makes every session debuggable after the fact.

## Key figures (summary for the article)

| Metric | Value |
|---|---|
| Built-in tools | 38 (+5 browser CDP conditional, + MCP tools dynamic) |
| Dispatch overhead for a 1-call batch | ~141 µs over raw call |
| Per-call overhead in an 8-call batch | ≈0 µs (amortized) |
| serde tool-argument speed | ~316 ns per round-trip |
| Parallel I/O batch speedup (16 file_read) | **4.04×** (spawn) |
| Parallel CPU batch speedup (8 × parse_html ~1 MB) | **3.18×** (spawn) |
| parse_html throughput | ~900 000 lines/s on a ~1 MB document |
| Peak parallelism in a live OSINT session | 13 concurrent calls |
| Time saved by parallelism (OSINT, 285 calls) | **912 seconds (67% of busy time)** |
| Tests in workspace | 900+ |

These figures appear throughout the series and are worth reading with their
context: the dispatch overhead (~141 µs) is the argument from 02 that batch
orchestration is practically free once batches reach 8 calls; the 4.04×/3.18×
speedups are the measured proof of the `join_all` vs `spawn` insight from 02,
run as the dedicated scenarios in 03; the live parallelism and time-saved
numbers come from the OSINT session dissected in 05 (Case 3); and the test
count is the project's answer to every bug the case studies uncovered —
each fix is locked in with at least one regression test.

## How to reproduce

```bash
# build
cargo build --release

# micro-benchmarks of the tool layer (without LLM and network)
./target/release/fathom bench
./target/release/fathom bench -s parallel-cpu -n 8

# statistics for any session
./target/release/fathom stats -o <session-output-dir>

# live run
./target/release/fathom run "your research query" --output ./out
```

Every number in this series can be regenerated with the commands above: `bench`
covers all eight scenarios from 03 with no network and no LLM (fixtures are
created in a temp directory and cleaned up automatically), `stats` reads the
SQLite trace that any `run` leaves behind — the same trace that powers the
session tables in 05 — and `run` is the entry point for reproducing the case
studies against a real model.

Measurement environment: macOS, 10 cores (Apple Silicon), release build `--release`.