# Benchmarks: tool execution layer

The LLM decides *what* to do — tools do the work. Between decision and execution
lies the `ToolExecutor` with its classification (parallel-safe vs sequential),
path conflict detector, cascade cancellation on shell errors, and `ToolCall`
serde machinery. This layer runs on every step of every agent — its
characteristics directly determine research time.

The benchmark is built into the product: `parallel-research bench`. All numbers
in this article are from a single run on **macOS, 10 cores, release build**
(2026-08-08); reproducible with a single command, fixtures are created
automatically in a temporary directory and cleaned up afterwards.

## Methodology

Each scenario repeats the run several times (5–10 iterations, warm-up call
before measurement) and computes the median/mean in microseconds. To avoid
measuring OS cache artifacts, fixtures are created uniquely for each run:

- 16 files of ~2 MB random hex data each (I/O batch),
- HTML document ~1 MB with a 3000-row table (CPU batch),
- JSON document ~4 MB with 20,000 objects (extract_json),
- RSS feed ~5 MB with 20,000 entries (web_feed),
- source tree: 240 Rust files ~14 KB each, each with 40 functions,
  40 structs and impl-blocks (code_symbols / repo_map).

Timing is `Instant::now()` around the call; for batches wall time is measured.

## 1. Dispatch overhead

How much machinery lies between a tool call and the actual work:

| measurement | iterations | per-call |
|---|---:|---:|
| `registry.execute` (raw dispatch) | 300 | 3066 µs |
| `execute_batch`, 1 call | 300 | 3103 µs |
| `execute_batch`, 8 calls (amortized) | 320 | 2995 µs |
| serde round-trip of `ToolCall` arguments | 100000 | 319 ns |

Overhead of the executor on top of raw dispatch — **36 µs** for a batch of one
call; amortized to zero with 8 calls.

The 3 ms figure includes `file_read` of an actual 2 MB file; the routing itself
takes microseconds (319 ns for argument serialization).

## 2. Parallel vs sequential — I/O batch

16 × `file_read` of different files, ~2 MB each:

| mode | wall time | per file | speedup |
|---|---:|---:|---:|
| sequential (one by one) | 48.7 ms | 3.05 ms | 1.00× |
| parallel (`execute_batch`, join_all) | 50.9 ms | 3.18 ms | 0.96× |
| parallel (`execute_batch_spawn`, tokio tasks) | 13.4 ms | 0.84 ms | **3.64×** |

`join_all` polls futures on a single thread — for cheap local reads this
provides no benefit. `execute_batch_spawn` distributes calls across runtime
workers — hence the 3.64×.

## 3. Parallel vs sequential — CPU batch

8 × `parse_html` of the same document ~1 MB (3000-row table):

| mode | wall time | per parse | speedup |
|---|---:|---:|---:|
| sequential | 46.8 ms | 5.84 ms | 1.00× |
| parallel (`execute_batch`) | 46.5 ms | 5.81 ms | 1.01× |
| parallel (`execute_batch_spawn`, tokio tasks) | 15.2 ms | 1.89 ms | **3.08×** |

CPU-bound work shows the difference most clearly: parallelism without gain for
`join_all` (futures still share one thread) and genuine distribution across
cores for the spawn variant.

## 4. Mixed batch — automatic partitioning

A realistic agent step: reads (parallel-safe) and writes (sequential) in one
batch:

| tool | phase | success | duration |
|---|---|---|---:|
| file_read ×4 | parallel | ✅ | 3–12 ms |
| file_write ×3 | sequential | ✅ | 0 ms |
| grep | parallel | ✅ | 13 ms |

Batch of 8 calls: 5 parallel, 3 sequential; wall time **14.3 ms**.
Result order matches call order (verified by id).

## 5. parse_html at different document sizes

| document | rows | avg parse | throughput |
|---|---:|---:|---:|
| 6 KB | 100 | 0.15 ms | 680k rows/s |
| 63 KB | 1000 | 1.13 ms | 887k rows/s |
| 191 KB | 3000 | 3.22 ms | 931k rows/s |
| 773 KB | 12000 | 13.10 ms | 916k rows/s |

Nearly linear growth, ~900k rows/s on large documents.

## 6. extract_json — throughput

JSON ~4 MB, 20,000 objects:

| query | iterations | average |
|---|---:|---:|
| wildcard `items[*].value` (limit 500) | 10 | 27.6 ms |
| deep key `items.12345.meta.score` | 10 | 21.1 ms |
| top-level key `total` | 10 | 14.4 ms |

The document is re-parsed on every call — a deliberate choice: the tool remains
stateless and parallel-safe; caching would create races between agents.

## 7. web_feed — feed parsing and parallelism

Local RSS fixture, `web_feed` (quick-xml). Scaling by feed size:

| entries in feed | avg parse | entries/s |
|---|---:|---:|
| 1000 | 0.57 ms | 1.75M/s |
| 5000 | 2.39 ms | 2.09M/s |
| 12000 | 5.62 ms | 2.13M/s |
| 20000 | 9.27 ms | 2.16M/s |

Linear growth, ~2M entries/s. Parallelism — 8 × `web_feed` of the same feed
~5 MB:

| mode | wall time | per call | speedup |
|---|---:|---:|---:|
| sequential | 113.7 ms | 14.22 ms | 1.00× |
| `execute_batch` (join_all) | 112.0 ms | 14.00 ms | 1.02× |
| `execute_batch_spawn` (tokio tasks) | 36.5 ms | 4.56 ms | **3.12×** |

## 8. code_symbols / repo_map — symbol extraction

Synthetic tree: 240 Rust files ~14 KB each (40 functions + 40 structs +
impl-blocks in each):

| tool | wall time | result |
|---|---:|---:|
| code_symbols (whole tree, limit 1000) | 1.7 ms | 7007 symbols |
| repo_map (whole tree, 3 symbols per file) | 6.6 ms | 4330 map lines |

Parallelism — 8 × `code_symbols` on different subdirectories:

| mode | wall time | per call | speedup |
|---|---:|---:|---:|
| sequential | 17.3 ms | 2.17 ms | 1.00× |
| `execute_batch` (join_all) | 10.2 ms | 1.27 ms | 1.70× |
| `execute_batch_spawn` (tokio tasks) | 3.4 ms | 0.42 ms | **5.16×** |

## 9. Cases: what the benchmark found and fixed

The benchmark is not a showcase — it's a tool. Two concrete cases from its life.

### 9.1. repo_map: 31× from regex caching

The first version of `extract_symbols` compiled two regular expressions
(`regex::Regex::new`) **per file**. For `code_symbols` with its `limit`
this is tolerable — the loop stops after the first ~25 files. For `repo_map`,
which honestly traverses the entire tree, it's a disaster: DFA compilation
dominated.

Fix: regexes moved to `OnceLock` statics (compiled once per process),
file reads in `repo_map` parallelized via `tokio::spawn` + `join_all`
while preserving order. A/B on a debug build (240 files):

| version | wall time |
|---|---:|
| before (regex per file + sequential reads) | 902.0 ms |
| after (OnceLock cache + spawn reads) | 29.5 ms |

**31× on a debug build**; on release the same tree processes in 6.6 ms.
A real-world data point: before optimization, `repo_map` on this project's
actual repository (103 files) took 106 ms in release.

### 9.2. Silent serialization: tools forgotten in classification

`ToolExecutor` divides tools into parallel-safe and sequential; an unknown tool
defaults to sequential "just in case." When `web_crawl`, `web_feed`,
`code_symbols`, and `repo_map` were introduced, they weren't added to the
classification — so any batch of several such calls **silently executed one
at a time**, without errors or warnings.

The benchmark showed this in numbers: a spawn-batch of 8 × `web_feed` gave
1.00× — the same as sequential execution, even though `parse_html` under the
same conditions gave 3×. After adding the tools to `parallel_safe` (and a
classification test):

| batch | spawn before fix | spawn after fix |
|---|---:|---:|
| 8 × web_feed | 1.00× | **3.12×** |
| 8 × code_symbols | ~1.0× | **5.16×** |

Moral: every new read-only tool must get a classification test — otherwise
parallelism disappears without a trace.

## Session statistics

`parallel-research stats -o <dir>` reads SQLite tracing from a real session
(tables tool_calls / agents / sessions) and computes what synthetic benchmarks
cannot show:

- **Average call duration per tool** (avg / p50 / p95) — how long
  `web_search` actually takes vs `file_read`;
- **Batching ratio** — fraction of calls executed in the parallel phase;
- **Breakdown by agent** — who made how many calls and how much time they
  spent.

Example from a debug session (153 calls):

| tool | calls | success | avg | p50 | p95 |
|---|---:|---:|---:|---:|---:|
| web_search | 8 | 8/8 | 587 ms | 571 ms | 780 ms |
| web_fetch | 76 | 76/76 | 320 ms | 240 ms | 1131 ms |
| spawn_agent | 10 | 10/10 | 15 ms | 6 ms | 64 ms |
| file_write | 19 | 19/19 | 1 ms | 0 ms | 8 ms |

Network tools — 85% of calls and nearly all the time; batching 73%: coordination
(15 ms per agent) vs 10 minutes of network I/O.

### Live run of jobs mode (new tools)

A job running four new tools on real data (release, 2026-08-08):

| tool | calls | success | avg |
|---|---:|---:|---:|
| web_crawl (3 pages, depth 1) | 1 | 1/1 | 4789 ms |
| web_feed (hnrss.org, 5 entries) | 1 | 1/1 | 1193 ms |
| repo_map (103 files of the repository) | 1 | 1/1 | 106 ms |
| code_symbols (single file) | 1 | 1/1 | 3 ms |

Network tools are bound by the network (4.8 s for a crawl with politeness
pauses, 1.2 s for loading a feed), local tools by the parser: 106 ms for a map
of 103 files before the optimization from §9.1 and milliseconds after.

## How to reproduce

```bash
parallel-research bench                # all 8 scenarios
parallel-research bench -s dispatch    # only overhead
parallel-research bench -s feed-parse  # web_feed
parallel-research bench -s code-map    # code_symbols / repo_map
parallel-research bench --save report.md
parallel-research stats -o <dir>       # real session statistics
```

Scenarios: `dispatch`, `parallel-io`, `parallel-cpu`, `mixed`, `parse-scale`,
`extract-json`, `feed-parse`, `code-map`. No network, no LLM, fixtures are
created and cleaned up automatically.

## Summary

- **319 ns** for argument serialization, **36 µs** executor overhead on a
  single-call batch — the execution layer vanishes against tool work.
- **3.1–5.2×** real speedup for CPU-bound batches via
  `execute_batch_spawn` (spawn across tokio workers); `join_all` is honest for
  network waiting but not for CPU.
- **~2M entries/s** for the feed parser (quick-xml), **~900k rows/s** for
  HTML selectors, milliseconds for a repository map.
- Mixed batch (5 parallel + 3 sequential operations) — 14.3 ms with
  preserved result order.
- The benchmark already paid for itself: found the 31× `repo_map` degradation
  (§9.1) and the silent serialization of four new tools (§9.2). Both issues
  closed with fixes and tests.