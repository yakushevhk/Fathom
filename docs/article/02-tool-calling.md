# 02. Tool calling: from JSON in LLM response to execution on cores

This file is a deep dive into the path of a single tool call through the system.
All measurements are taken by the `parallel-research bench` command; methodology is in
[03-benchmarks.md](./03-benchmarks.md).

## Tool anatomy

Each tool implements a trait (`pr-tools/registry.rs`):

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> ToolSchema;      // JSON Schema from schemars
    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext)
        -> anyhow::Result<ToolOutput>;
}
```

- **Send + Sync** — a mandatory requirement: tools are executed in spawned tasks
  on different threads. This is not a "nice to have" but an architectural contract.
- **ToolSchema** is generated from the argument type via `schemars` — the schema
  that the LLM sees never diverges from the actual argument parser.
- **ToolContext** — execution state: working directory, HTTP client
  with timeouts (30 s total / 10 s connect), fetch cache, file locks,
  contact database, LLM provider for LLM-assisted tools.

The registry (`ToolRegistry`) is a simple `HashMap<String, Arc<dyn Tool>>` with
`with_builtins()`, registering 38 tools. An unknown tool returns
`ToolOutput::err("Unknown tool: …")`, not a panic — the LLM can read the
error and correct itself.

## Wire format: where nanoseconds live

The LLM returns a tool call in the OpenAI-compatible object format:

```rust
struct ToolCall {
    id: String,
    call_type: String,          // "function"
    function: ToolCallFunction, // { name, arguments: String /* JSON */ }
}
```

Arguments arrive as a **JSON string** — this is the protocol standard, but each
call pays for serialization/deserialization. Measurement (100 000 iterations):

| Operation | Time |
|---|---:|
| `ToolCall::new` + `arguments()` round-trip | **~316 ns** |

Negligible: even 1000 calls in a batch is 0.3 ms total. The bottleneck of
tool calling is not serialization, but execution.

## Batch: why not one by one

Modern models return several tool calls in a single response. Executing
them sequentially means serializing network latencies. Example from a live
session: 4 `extract_json` calls at ~840 ms each.

- Sequential: 4 × 840 = 3 360 ms
- Parallel: ~850 ms (measured in a live parsing session, peak concurrency = 4)

`ToolExecutor::execute_batch` and its multi-threaded variant
`execute_batch_spawn` accept a vector of calls and return results
**in the original order**, each with a `parallel: bool` and `duration_ms` tag.

## Partitioning: which tools can be parallelized

Whitelist of parallel-safe tools:

```
web_search, web_fetch, parse_html, extract_json, search_news, search_social,
search_business_directory, parse_corporate_site, suggest_emails, verify_email,
verify_phone, verify_social_profile, enrich_company, enrich_person,
find_leads, extract_contacts, save_contacts, pdf, file_read, grep, glob,
scratchpad, memory, skill_load, vision
```

Everything else (`shell`, `file_write`, `file_edit`, `git_*`, `python_exec`,
`node_exec`, `browser_*`, `spawn_agent`, `undo`) is executed
**sequentially**: these tools have observable side effects, and their
reordering changes the result.

An additional safeguard within the parallel group is **path intersection
detection**: if two "parallel" calls read/write the same file, the second
is automatically moved to the sequential phase. So `file_read` stays
parallel in 99% of cases, but does not race with itself.

## Key insight: concurrency ≠ parallelism

The first version of the batch used `futures::join_all` on each call's future.
Measurements showed a sobering picture:

| Batch | sequential | join_all | speed-up |
|---|---:|---:|---:|
| 16 × file_read ~2 MB | 320 ms | 311 ms | **0.97×** |
| 8 × parse_html ~1 MB | 810 ms | 791 ms | **1.02×** |

Why? `join_all` polls all futures **on a single task**. A network call inside
`await` yields control — and other futures make progress (concurrency).
But the CPU work of the HTML parser **contains no await** and monopolizes
the thread: futures execute one after another, just interleaved.

For real research sessions this is not as painful — batches there are mostly
network-bound (searches, fetches), and join_all honestly overlapped waiting
(see peak concurrency of 13–15 in live sessions). But once the agent started
writing code and parsing local files, CPU-bound scenarios required true
multi-threading.

### Solution: execute_batch_spawn

Each parallel-safe call is wrapped in `tokio::spawn`:

```rust
let handle = tokio::spawn(async move {
    // shell-cascade check, then:
    let output = registry.execute(tc.name(), tc.arguments(), &ctx).await;
    ToolBatchResult { tool_call: tc, output, parallel: true, duration_ms }
});
```

The multi-threaded tokio runtime distributes tasks across worker threads —
and CPU-bound tools actually execute on different cores. Results are collected
via `join_all` on join handles; a join error is turned into
`ToolOutput::err`, not a runtime panic.

Results on a 10-core machine:

| Batch | sequential | join_all | spawn | speed-up (spawn) |
|---|---:|---:|---:|---:|
| 16 × file_read ~2 MB | 319.8 ms | 311.4 ms | **79.2 ms** | **4.04×** |
| 8 × parse_html ~1 MB | 809.8 ms | 791.5 ms | **254.7 ms** | **3.18×** |

The speed-up is not 10× (number of cores) because:

- spawn itself has a cost (task allocation, scheduling);
- file_read has a shared component (page cache reads, allocations);
- on short batches, fixed costs are more noticeable. On batches of 16 calls,
  the I/O scenario already scales nearly linearly.

### Dispatch overhead

How much does the machinery between "LLM returned a tool call" and "the tool
started executing" cost (a batch of one `file_read` on a 2 MB file):

| Layer | Time per call |
|---|---:|
| `registry.execute` (raw dispatch, dominated by file read) | 3 150 µs |
| `execute_batch` on top (1 call) | 3 291 µs |
| **batch overhead** | **~141 µs** |
| `execute_batch`, batch of 8 (amortized per call) | ~3 154 µs → overhead **≈0** |

Takeaway for the article: batch orchestration is practically free for batches
of ≥ 8 calls — and modern models return exactly such batches.

## Batch safety mechanics

### Shell cascade

If a `shell` call fails in a batch, all **not yet started** calls
(parallel and sequential) are cancelled with the tag
`Cancelled: sibling shell tool failed with: …`. This guards against the
scenario "build failed — agents keep writing code on top of a broken state."

### Instant result interception

Cheap idempotent calls (repeated scratchpad-read, etc.)
are returned from the cache at phase 1, never reaching execution.

### Result order

Results are returned strictly in the order of the original calls — the LLM
sees tool results in the same order it requested the calls. This is
verified by unit tests for both batch variants.

### Error format

Tool errors are formatted via `{e:#}` — the entire anyhow chain is preserved
down to the root cause. For the agent, the difference between "Tool execution
error: database error" and "…: contacts.db: table contacts has no column
phone_norm: SQL error" is the difference between "I'll try again" and a
targeted fix.

### Truncation

Large results are trimmed to budget values so that a single two-megabyte
HTML does not burn the agent's context. `parse_html` additionally trims
its own output (50 KB) and limits the number of elements (default 100).

## Final tool call path diagram

```
LLM response: tool_calls[]
   │  parse ToolCall (arguments: JSON string)      ~316 ns/ea
   ▼
Phase 1: instant interception, path dedup
   ▼
Phase 2a: parallel_safe → tokio::spawn each → join_all(handles)
Phase 2b: sequential → strictly one by one, shell cascade
   ▼
Phase 3: ToolOutput → truncation → ToolResult into agent messages
         + SQLite write (tool_results: name, success, duration_ms)
         + harvest_finding for search/parser tools
```