# parallel-research: article documentation

A complete breakdown of a multi-agent research tool in Rust: architecture,
tool-calling mechanics, parallel execution, benchmarks, and real-world case studies with actual LLMs.

The documentation is written as a source for the article: each file is self-contained,
contains concrete numbers and reproducible commands.

## Contents

| File | About |
|---|---|
| [01-architecture.md](./01-architecture.md) | Architecture: 8 crates, research pipeline, agent model, guardrails |
| [02-tool-calling.md](./02-tool-calling.md) | Tool-calling mechanics: registry, batches, partitioning, join_all vs spawn parallelism |
| [03-benchmarks.md](./03-benchmarks.md) | Benchmarks: dispatch, parallel batches, parsing, live session statistics |
| [04-parsing.md](./04-parsing.md) | Parsing guide: `parse_html`, `extract_json`, SSRF protection, cache |
| [05-case-studies.md](./05-case-studies.md) | Six real-world case studies with actual LLM: coding, parsing, OSINT, research; bugs found |

## What is this project

`parallel-research` is a CLI/TUI/HTTP tool that takes a natural language
query and executes it with a **team of parallel LLM agents**:

- planning and decomposition of the query into subtasks;
- parallel execution of research agents;
- **up to 38 built-in tools** (web search, HTML/JSON parsing, files, shell, git,
  Python/Node REPL, OSINT verification of emails/phones/social media, CRM sync);
- batched tool calling with true multi-threaded parallelism (`tokio::spawn`);
- protection: SSRF guard, prompt-injection filters, file locks, cancellation on shell failure;
- everything is written to SQLite: every tool call with duration — later analyzed
  via the `parallel-research stats` command.

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

## How to reproduce

```bash
# build
cargo build --release

# micro-benchmarks of the tool layer (without LLM and network)
./target/release/parallel-research bench
./target/release/parallel-research bench -s parallel-cpu -n 8

# statistics for any session
./target/release/parallel-research stats -o <session-output-dir>

# live run
./target/release/parallel-research run "your research query" --output ./out
```

Measurement environment: macOS, 10 cores (Apple Silicon), release build `--release`.