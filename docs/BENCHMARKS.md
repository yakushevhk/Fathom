# parallel-research — tool layer benchmarks

host: macos · cores: 10 · files: 16 × ~2047 KB · release build


## 1. Tool dispatch overhead

How much machinery sits between the LLM's tool call and the actual work.

| measurement | iterations | per-call |
|---|---:|---:|
| registry.execute (raw dispatch) | 300 | 5140 µs |
| execute_batch, 1 call | 300 | 7614 µs |
| execute_batch, 8 calls (amortized) | 320 | 5893 µs |
| ToolCall args serde round-trip | 100000 | 752 ns |

Executor overhead over raw dispatch: **2473 µs** per single-call batch; amortized overhead drops to **753 µs** per call in an 8-call batch.

## 2. Parallel vs sequential — I/O-bound batch (file_read)

16 × file_read of distinct ~2047 KB files.

| mode | wall time | per file | speedup |
|---|---:|---:|---:|
| sequential (one at a time) | 79.0 ms | 4.94 ms | 1.00× |
| parallel (execute_batch, join_all) | 83.7 ms | 5.23 ms | **0.94×** |
| parallel (execute_batch_spawn, tokio tasks) | 25.8 ms | 1.61 ms | **3.06×** |

16/16 calls succeeded (join_all), 16/16 (spawn); all classified parallel-safe: true.

## 3. Parallel vs sequential — CPU-bound batch (parse_html)

8 × parse_html of a ~1 MB table (3000 rows), selector `tr.item`, texts mode.

| mode | wall time | per parse | speedup |
|---|---:|---:|---:|
| sequential | 130.3 ms | 16.29 ms | 1.00× |
| parallel (execute_batch) | 72.4 ms | 9.05 ms | **1.80×** |
| parallel (execute_batch_spawn, tokio tasks) | 34.5 ms | 4.31 ms | **3.78×** |

8/8 parses succeeded (join_all), 8/8 (spawn). join_all shares one thread — spawn spreads CPU work across cores.

## 4. Mixed batch — automatic partitioning

A realistic agent turn: reads (parallel-safe) + writes (sequential) in one batch.

| tool | phase | success | duration |
|---|---|---|---:|
| file_read (r0) | parallel | ✅ | 16 ms |
| file_read (r1) | parallel | ✅ | 20 ms |
| file_read (r2) | parallel | ✅ | 25 ms |
| file_read (r3) | parallel | ✅ | 29 ms |
| file_write (w0) | sequential | ✅ | 2 ms |
| file_write (w1) | sequential | ✅ | 0 ms |
| file_write (w2) | sequential | ✅ | 0 ms |
| grep (g0) | parallel | ✅ | 65 ms |

Batch of 8 calls: 5 ran concurrently, 3 serialized; total wall time 70.9 ms, 8 succeeded.
Order in the result vector matches the original call order (verified by id).

## 5. parse_html scaling with document size

Same selector (`tr.item`, texts mode), documents of increasing size.

| document | rows | avg parse | throughput |
|---|---:|---:|---:|
| 6 KB | 100 | 0.19 ms | 531915 rows/s |
| 63 KB | 1000 | 4.08 ms | 245339 rows/s |
| 191 KB | 3000 | 9.55 ms | 314268 rows/s |
| 773 KB | 12000 | 34.22 ms | 350723 rows/s |

## 6. extract_json throughput

~4 MB JSON document with 20 000 objects.

| query | iterations | avg |
|---|---:|---:|
| wildcard scan `items[*].value` (limit 500) | 10 | 71.24 ms |
| deep single key `items.12345.meta.score` | 10 | 67.78 ms |
| top-level key `total` | 10 | 43.47 ms |

Source parsing dominates: the JSON document is re-parsed per call (no cross-call cache), which keeps the tool stateless and parallel-safe.

## 7. web_feed (quick-xml) scaling and parallelism

Local RSS fixture, `web_feed`: tolerance to feed size and CPU-bound speed under parallelism.

| feed items | avg parse | items/s |
|---|---:|---:|
| 1000 | 0.93 ms | 1077586 items/s |
| 5000 | 4.78 ms | 1045588 items/s |
| 12000 | 10.81 ms | 1110391 items/s |
| 20000 | 27.10 ms | 738089 items/s |

Parallel vs sequential — 8 × web_feed of the ~5 MB feed.

| mode | wall time | per call | speedup |
|---|---:|---:|---:|
| sequential | 236.2 ms | 29.53 ms | 1.00× |
| parallel (execute_batch) | 235.0 ms | 29.37 ms | **1.01×** |
| parallel (execute_batch_spawn, tokio tasks) | 188.6 ms | 23.57 ms | **1.25×** |

8/8 feed parses succeeded (join_all), 8/8 (spawn). join_all shares one thread — spawn spreads CPU work across cores.

## 8. code_symbols / repo_map — symbol extraction throughput

240 Rust files (~14 KB each, 40 fns + 40 structs + impls per file).

| tool | mode | wall time | per tree | items |
|---|---:|---:|---:|---:|
| code_symbols | single | 7.2 ms | — | 7007 lines |
| repo_map | single | 34.2 ms | — | 4330 lines |

Parallel vs sequential — 8 × code_symbols on distinct file subsets.

| mode | wall time | per call | speedup |
|---|---:|---:|---:|
| sequential | 61.9 ms | 7.73 ms | 1.00× |
| parallel (execute_batch) | 32.6 ms | 4.08 ms | **1.90×** |
| parallel (execute_batch_spawn, tokio tasks) | 20.4 ms | 2.54 ms | **3.04×** |

8/8 symbol scans succeeded (join_all), 8/8 (spawn). join_all shares one thread — spawn spreads CPU work across cores.

## 9. Semantic memory (pr-memory) — absorb / search / digest

Offline TF-IDF embedder (no network, no LLM); in-memory SQLite.

| batch size | absorbed | wall time | per fact |
|---:|---:|---:|---:|
| 10 | 10 | 10.2 ms | 1020 µs |
| 100 | 89 | 9.4 ms | 94 µs |
| 500 | 335 | 82.6 ms | 165 µs |

Re-absorbing 100 already-known facts: 93 skipped, 0 created in 5.1 ms (dedup fast path).

Hybrid search (vector + BM25) over the stored facts:

| query | matches | median latency |
|---|---:|---:|
| Acme revenue units filing | 5 | 2.30 ms |
| Globex churn rate report | 5 | 1.62 ms |
| headcount in Kazan during 2022 | 5 | 2.23 ms |
| Initech margin units | 5 | 1.95 ms |
| support load Dubai filing | 5 | 1.66 ms |

Search latency vs store size (brute-force cosine scan):

| memories | fill time | search (median) |
|---:|---:|---:|
| 1000 | 53 ms | 5.28 ms |
| 5000 | 926 ms | 23.52 ms |
| 10000 | 828 ms | 47.47 ms |

Digest build (relevant + TODOs + recent): 4 relevant memories in 4.76 ms.

---

## How to reproduce

```bash
cargo build --release
./target/release/parallel-research bench --scenario all > bench.md
# individual scenarios: dispatch | parallel-io | parallel-cpu | mixed |
#                       parse-scale | extract-json | feed-parse | code-map | memory
./target/release/parallel-research stats -o <output-dir>   # p50/p95 per tool from real sessions
```

The benchmarks do not use the network or an LLM (offline TF-IDF embedder for the
`memory` scenario) — they can be run in CI.

## Notes

- **dispatch** — executor-layer overhead: ~0.75 ms/call in an 8-call batch on
  top of raw registry dispatch.
- **spawn vs join_all** — CPU-bound tools (parse_html, code_symbols, web_feed)
  speed up in `execute_batch_spawn` (tokio tasks across cores): up to 3.8×;
  `join_all` shares a single thread and is only useful for I/O.
- **memory** — hybrid search latency grows linearly with store size
  (brute-force cosine scan): ~5 ms @1K, ~47 ms @10K records —
  acceptable for an offline digest; beyond >50K records an ANN index makes sense.
- The figures above are a snapshot on macOS arm64 (10 cores), release build; rebuild
  before comparing branches.
