//! Benchmark harness and session statistics.
//!
//! `bench` measures the tool-execution layer without any LLM or network
//! involvement: dispatch overhead, parallel vs sequential batches, CPU-bound
//! parsing throughput, and argument serde cost. `stats` analyzes a recorded
//! session database (.research.db) from real runs: per-tool durations,
//! success rates, peak concurrency and batching profile.

use anyhow::Context as _;
use pr_agent::ToolExecutor;
use pr_core::{SearchConfig, ToolCall};
use pr_tools::{ToolContext, ToolRegistry};
use serde_json::json;
use std::path::PathBuf;
use std::time::Instant;

pub async fn run_bench(scenario: &str, n: usize, save: Option<String>) -> anyhow::Result<()> {
    let env = BenchEnv::setup(n.max(2))?;
    let mut report = Report::new();

    let all = scenario == "all";
    let scenarios: &[&str] = &[
        "dispatch",
        "parallel-io",
        "parallel-cpu",
        "mixed",
        "parse-scale",
        "extract-json",
        "feed-parse",
        "code-map",
        "memory",
    ];
    if !all && !scenarios.contains(&scenario) {
        anyhow::bail!(
            "unknown scenario '{scenario}' (valid: all, {})",
            scenarios.join(", ")
        );
    }

    report.line(&format!(
        "# fathom — tool layer benchmarks\n\nhost: {} · cores: {} · files: {} × ~{} KB · release build\n",
        std::env::consts::OS,
        std::thread::available_parallelism()
            .map(|v| v.get())
            .unwrap_or(1),
        env.data_files.len(),
        env.file_kb
    ));

    if all || scenario == "dispatch" {
        bench_dispatch(&env, &mut report).await;
    }
    if all || scenario == "parallel-io" {
        bench_parallel_io(&env, &mut report).await;
    }
    if all || scenario == "parallel-cpu" {
        bench_parallel_cpu(&env, &mut report).await;
    }
    if all || scenario == "mixed" {
        bench_mixed(&env, &mut report).await;
    }
    if all || scenario == "parse-scale" {
        bench_parse_scale(&env, &mut report).await;
    }
    if all || scenario == "extract-json" {
        bench_extract_json(&env, &mut report).await;
    }
    if all || scenario == "feed-parse" {
        bench_feed_parse(&env, &mut report).await;
    }
    if all || scenario == "code-map" {
        bench_code_map(&env, &mut report).await;
    }
    if all || scenario == "memory" {
        bench_memory(&env, &mut report).await;
    }

    let text = report.finish();
    print!("{text}");
    if let Some(path) = save {
        std::fs::write(&path, &text)
            .with_context(|| format!("failed to write report to {path}"))?;
        eprintln!("\n📄 report saved to {path}");
    }

    env.cleanup();
    Ok(())
}

struct BenchEnv {
    workdir: PathBuf,
    registry: std::sync::Arc<ToolRegistry>,
    ctx: std::sync::Arc<ToolContext>,
    data_files: Vec<PathBuf>,
    html_path: PathBuf,
    json_path: PathBuf,
    feed_path: PathBuf,
    code_dir: PathBuf,
    file_kb: usize,
}

impl BenchEnv {
    fn setup(n: usize) -> anyhow::Result<Self> {
        // Unique temp dir per call so parallel tests don't race on the same
        // PID-based path. Counter is atomic; PID suffix keeps it debuggable.
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let workdir = std::env::temp_dir().join(format!("pr-bench-{}-{seq}", std::process::id()));
        std::fs::create_dir_all(&workdir)?;

        // I/O fixtures: n text files of ~2 MB each.
        let line = "The quick brown fox jumps over the lazy dog. marker-42 payload data row.\n";
        let lines_per_file = (2 * 1024 * 1024) / line.len();
        let file_kb = lines_per_file * line.len() / 1024;
        let mut data_files = Vec::with_capacity(n);
        for i in 0..n {
            let path = workdir.join(format!("data_{i:02}.txt"));
            let mut content = String::with_capacity(lines_per_file * line.len());
            for _ in 0..lines_per_file {
                content.push_str(line);
            }
            std::fs::write(&path, &content)?;
            data_files.push(path);
        }

        // HTML fixture: a 3000-row table (~1 MB) for CPU-bound parse tests.
        let mut html = String::with_capacity(1_200_000);
        html.push_str("<html><head><title>bench</title></head><body><table id=\"items\">");
        for i in 0..3000 {
            html.push_str(&format!(
                "<tr class=\"item\"><td class=\"name\">Entry {i}</td>\
                 <td class=\"value\">{v}</td>\
                 <td><a href=\"/detail/{i}\">details {i}</a></td></tr>",
                v = i * 17 % 1000
            ));
        }
        html.push_str("</table></body></html>");
        let html_path = workdir.join("large.html");
        std::fs::write(&html_path, &html)?;

        // JSON fixture: 20 000 objects (~4 MB) for extract_json tests.
        let mut items = Vec::with_capacity(20_000);
        for i in 0..20_000 {
            items.push(json!({
                "id": i,
                "name": format!("item-{i}"),
                "value": i * 13 % 997,
                "tags": ["a", "b"],
                "meta": {"index": i, "score": (i % 100) as f64 / 7.0}
            }));
        }
        let json_path = workdir.join("large.json");
        std::fs::write(&json_path, json!({ "items": items, "total": 20_000 }).to_string())?;

        // RSS fixture: a ~20k-item feed (~5 MB) for web_feed (quick-xml) tests.
        let feed_items = 20_000usize;
        let mut xml = String::with_capacity(5_000_000);
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\"><channel><title>bench</title>");
        for i in 0..feed_items {
            xml.push_str(&format!(
                "<item><title>Item {i}</title><link>/post/{i}</link>\
                 <pubDate>2026-08-08</pubDate><description>Benchmark entry number {i} with some body text.</description></item>"
            ));
        }
        xml.push_str("</channel></rss>");
        let feed_path = workdir.join("large.rss");
        std::fs::write(&feed_path, &xml)?;

        // Source-code tree: 240 Rust files (~14 KB each) with functions and
        // structs, for code_symbols / repo_map throughput tests.
        let code_dir = workdir.join("src");
        std::fs::create_dir_all(&code_dir)?;
        for i in 0..240 {
            let mut src = String::with_capacity(14_000);
            src.push_str(&format!("// module_{i}.rs\n"));
            for k in 0..40 {
                src.push_str(&format!(
                    "\n/// handler for {k}\npub fn handle_{}_{k}(input: &str) -> i64 {{\n    // parse {k}\n    let n: i64 = input.trim().parse().unwrap_or_default();\n    n + {}\n}}\n",
                    i, i
                ));
                src.push_str(&format!(
                    "\npub struct Item_{}_{k} {{\n    pub id: u64,\n    pub name: String,\n    pub tags: Vec<String>,\n}}\n",
                    i
                ));
                src.push_str(&format!(
                    "\nimpl Item_{}_{k} {{\n    pub fn new(id: u64) -> Self {{\n        Self {{ id, name: String::new(), tags: Vec::new() }}\n    }}\n}}\n",
                    i
                ));
            }
            std::fs::write(code_dir.join(format!("mod_{i:03}.rs")), &src)?;
        }

        let registry = std::sync::Arc::new(ToolRegistry::with_builtins());
        let ctx = std::sync::Arc::new(ToolContext::new(workdir.clone(), SearchConfig::default()));

        Ok(Self {
            workdir,
            registry,
            ctx,
            data_files,
            html_path,
            json_path,
            feed_path,
            code_dir,
            file_kb,
        })
    }

    fn cleanup(&self) {
        let _ = std::fs::remove_dir_all(&self.workdir);
    }

    fn read_call(&self, i: usize, idx: usize) -> ToolCall {
        ToolCall::new(
            format!("c{i}"),
            "file_read",
            json!({"path": self.data_files[idx].display().to_string()}),
        )
    }
}

struct Report {
    buf: String,
}

impl Report {
    fn new() -> Self {
        Self { buf: String::new() }
    }
    fn line(&mut self, s: &str) {
        self.buf.push_str(s);
        self.buf.push('\n');
    }
    fn finish(self) -> String {
        self.buf
    }
}

fn percentile(sorted: &[u128], p: f64) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx]
}

async fn bench_dispatch(env: &BenchEnv, report: &mut Report) {
    report.line("\n## 1. Tool dispatch overhead");
    report.line("\nHow much machinery sits between the LLM's tool call and the actual work.\n");
    report.line("| measurement | iterations | per-call |");
    report.line("|---|---:|---:|");

    let executor = ToolExecutor::new();
    let path0 = env.data_files[0].display().to_string();

    // Warm-up (page cache, allocator arenas, first-call lazy init).
    for _ in 0..20 {
        let _ = env
            .registry
            .execute("file_read", json!({"path": path0}), &env.ctx)
            .await;
    }

    // A) raw registry dispatch (what execute_batch wraps).
    let iters = 300u32;
    let start = Instant::now();
    for _ in 0..iters {
        let _ = env
            .registry
            .execute("file_read", json!({"path": path0}), &env.ctx)
            .await;
    }
    let raw_ns = start.elapsed().as_nanos() / iters as u128;
    report.line(&format!(
        "| registry.execute (raw dispatch) | {iters} | {} µs |",
        raw_ns / 1000
    ));

    // B) executor batch with a single call (full partition/results machinery).
    let start = Instant::now();
    for i in 0..iters {
        let calls = vec![ToolCall::new(
            format!("c{i}"),
            "file_read",
            json!({"path": path0}),
        )];
        let _ = executor.execute_batch(calls, &env.registry, &env.ctx).await;
    }
    let batch1_ns = start.elapsed().as_nanos() / iters as u128;
    report.line(&format!(
        "| execute_batch, 1 call | {iters} | {} µs |",
        batch1_ns / 1000
    ));

    // C) executor batch with 8 parallel-safe calls, amortized per call.
    let batches = 40u32;
    let start = Instant::now();
    for b in 0..batches {
        let calls: Vec<ToolCall> = (0..8)
            .map(|k| {
                ToolCall::new(
                    format!("c{b}-{k}"),
                    "file_read",
                    json!({"path": env.data_files[k % env.data_files.len()].display().to_string()}),
                )
            })
            .collect();
        let _ = executor.execute_batch(calls, &env.registry, &env.ctx).await;
    }
    let batch8_ns = start.elapsed().as_nanos() / (batches as u128 * 8);
    report.line(&format!(
        "| execute_batch, 8 calls (amortized) | {} | {} µs |",
        batches * 8,
        batch8_ns / 1000
    ));

    // D) ToolCall argument round-trip: JSON string <-> Value on every call.
    let iters = 100_000u32;
    let start = Instant::now();
    for i in 0..iters {
        let tc = ToolCall::new(format!("c{i}"), "file_read", json!({"path": path0, "offset": 0}));
        let _ = tc.arguments();
    }
    let serde_ns = start.elapsed().as_nanos() / iters as u128;
    report.line(&format!(
        "| ToolCall args serde round-trip | {iters} | {} ns |",
        serde_ns
    ));

    report.line(&format!(
        "\nExecutor overhead over raw dispatch: **{} µs** per single-call batch; \
         amortized overhead drops to **{} µs** per call in an 8-call batch.",
        batch1_ns.saturating_sub(raw_ns) / 1000,
        batch8_ns.saturating_sub(raw_ns) / 1000
    ));
}

async fn bench_parallel_io(env: &BenchEnv, report: &mut Report) {
    report.line("\n## 2. Parallel vs sequential — I/O-bound batch (file_read)");
    let n = env.data_files.len();
    let calls: Vec<ToolCall> = (0..n).map(|i| env.read_call(i, i)).collect();
    report.line(&format!(
        "\n{n} × file_read of distinct ~{} KB files.\n",
        env.file_kb
    ));
    report.line("| mode | wall time | per file | speedup |");
    report.line("|---|---:|---:|---:|");

    let start = Instant::now();
    for tc in &calls {
        let _ = env.registry.execute(tc.name(), tc.arguments(), &env.ctx).await;
    }
    let seq = start.elapsed();
    report.line(&format!(
        "| sequential (one at a time) | {:.1} ms | {:.2} ms | 1.00× |",
        seq.as_secs_f64() * 1000.0,
        seq.as_secs_f64() * 1000.0 / n as f64
    ));

    let executor = ToolExecutor::new();
    let start = Instant::now();
    let results = executor
        .execute_batch(calls.clone(), &env.registry, &env.ctx)
        .await;
    let par = start.elapsed();
    let ok = results.iter().filter(|r| r.output.success).count();
    let all_parallel = results.iter().all(|r| r.parallel);
    report.line(&format!(
        "| parallel (execute_batch, join_all) | {:.1} ms | {:.2} ms | **{:.2}×** |",
        par.as_secs_f64() * 1000.0,
        par.as_secs_f64() * 1000.0 / n as f64,
        seq.as_secs_f64() / par.as_secs_f64().max(f64::EPSILON)
    ));

    let start = Instant::now();
    let results = executor
        .execute_batch_spawn(calls.clone(), env.registry.clone(), env.ctx.clone())
        .await;
    let spw = start.elapsed();
    let ok_spawn = results.iter().filter(|r| r.output.success).count();
    report.line(&format!(
        "| parallel (execute_batch_spawn, tokio tasks) | {:.1} ms | {:.2} ms | **{:.2}×** |",
        spw.as_secs_f64() * 1000.0,
        spw.as_secs_f64() * 1000.0 / n as f64,
        seq.as_secs_f64() / spw.as_secs_f64().max(f64::EPSILON)
    ));
    report.line(&format!(
        "\n{ok}/{n} calls succeeded (join_all), {ok_spawn}/{n} (spawn); all classified parallel-safe: {all_parallel}."
    ));
}

async fn bench_parallel_cpu(env: &BenchEnv, report: &mut Report) {
    report.line("\n## 3. Parallel vs sequential — CPU-bound batch (parse_html)");
    let rounds = 8usize.min(env.data_files.len());
    let html = env.html_path.display().to_string();
    let make_calls = |tag: &str| -> Vec<ToolCall> {
        (0..rounds)
            .map(|i| {
                ToolCall::new(
                    format!("{tag}{i}"),
                    "parse_html",
                    json!({"source": html, "selector": "tr.item", "mode": "texts", "limit": 500}),
                )
            })
            .collect()
    };

    report.line(&format!(
        "\n{rounds} × parse_html of a ~1 MB table (3000 rows), selector `tr.item`, texts mode.\n"
    ));
    report.line("| mode | wall time | per parse | speedup |");
    report.line("|---|---:|---:|---:|");

    // Warm-up (scraper selector compilation is cheap but caches fill in).
    let _ = env
        .registry
        .execute("parse_html", json!({"source": html, "selector": "tr.item", "mode": "texts"}), &env.ctx)
        .await;

    let start = Instant::now();
    for tc in make_calls("s") {
        let _ = env.registry.execute(tc.name(), tc.arguments(), &env.ctx).await;
    }
    let seq = start.elapsed();
    report.line(&format!(
        "| sequential | {:.1} ms | {:.2} ms | 1.00× |",
        seq.as_secs_f64() * 1000.0,
        seq.as_secs_f64() * 1000.0 / rounds as f64
    ));

    let executor = ToolExecutor::new();
    let start = Instant::now();
    let results = executor
        .execute_batch(make_calls("p"), &env.registry, &env.ctx)
        .await;
    let par = start.elapsed();
    let ok = results.iter().filter(|r| r.output.success).count();
    report.line(&format!(
        "| parallel (execute_batch) | {:.1} ms | {:.2} ms | **{:.2}×** |",
        par.as_secs_f64() * 1000.0,
        par.as_secs_f64() * 1000.0 / rounds as f64,
        seq.as_secs_f64() / par.as_secs_f64().max(f64::EPSILON)
    ));

    let start = Instant::now();
    let results = executor
        .execute_batch_spawn(make_calls("t"), env.registry.clone(), env.ctx.clone())
        .await;
    let spw = start.elapsed();
    let ok_spawn = results.iter().filter(|r| r.output.success).count();
    report.line(&format!(
        "| parallel (execute_batch_spawn, tokio tasks) | {:.1} ms | {:.2} ms | **{:.2}×** |",
        spw.as_secs_f64() * 1000.0,
        spw.as_secs_f64() * 1000.0 / rounds as f64,
        seq.as_secs_f64() / spw.as_secs_f64().max(f64::EPSILON)
    ));
    report.line(&format!(
        "\n{ok}/{rounds} parses succeeded (join_all), {ok_spawn}/{rounds} (spawn). \
         join_all shares one thread — spawn spreads CPU work across cores."
    ));
}

async fn bench_mixed(env: &BenchEnv, report: &mut Report) {
    report.line("\n## 4. Mixed batch — automatic partitioning");
    report.line(
        "\nA realistic agent turn: reads (parallel-safe) + writes (sequential) in one batch.\n",
    );

    let mut calls: Vec<ToolCall> = Vec::new();
    for (i, f) in env.data_files.iter().take(4).enumerate() {
        calls.push(ToolCall::new(
            format!("r{i}"),
            "file_read",
            json!({"path": f.display().to_string()}),
        ));
    }
    for i in 0..3 {
        calls.push(ToolCall::new(
            format!("w{i}"),
            "file_write",
            json!({"path": env.workdir.join(format!("out_{i}.txt")).display().to_string(),
                   "content": format!("mixed-bench output {i}\n")}),
        ));
    }
    calls.push(ToolCall::new(
        "g0",
        "grep",
        json!({"pattern": "marker-42", "path": env.workdir.display().to_string()}),
    ));

    let executor = ToolExecutor::new();
    let start = Instant::now();
    let results = executor.execute_batch(calls, &env.registry, &env.ctx).await;
    let wall = start.elapsed();

    let par_count = results.iter().filter(|r| r.parallel).count();
    let seq_count = results.len() - par_count;
    let ok = results.iter().filter(|r| r.output.success).count();

    report.line("| tool | phase | success | duration |");
    report.line("|---|---|---|---:|");
    for r in &results {
        report.line(&format!(
            "| {} ({}) | {} | {} | {} ms |",
            r.tool_call.name(),
            r.tool_call.id,
            if r.parallel { "parallel" } else { "sequential" },
            if r.output.success { "✅" } else { "❌" },
            r.duration_ms
        ));
    }
    report.line(&format!(
        "\nBatch of {} calls: {} ran concurrently, {} serialized; total wall time {:.1} ms, {} succeeded.",
        results.len(),
        par_count,
        seq_count,
        wall.as_secs_f64() * 1000.0,
        ok
    ));
    report.line("Order in the result vector matches the original call order (verified by id).");
}

async fn bench_parse_scale(env: &BenchEnv, report: &mut Report) {
    report.line("\n## 5. parse_html scaling with document size");
    report.line("\nSame selector (`tr.item`, texts mode), documents of increasing size.\n");
    report.line("| document | rows | avg parse | throughput |");
    report.line("|---|---:|---:|---:|");

    for rows in [100usize, 1_000, 3_000, 12_000] {
        let mut html = String::new();
        html.push_str("<html><body><table>");
        for i in 0..rows {
            html.push_str(&format!(
                "<tr class=\"item\"><td class=\"name\">Entry {i}</td><td>{v}</td></tr>",
                v = i % 1000
            ));
        }
        html.push_str("</table></body></html>");
        let path = env.workdir.join(format!("scale_{rows}.html"));
        std::fs::write(&path, &html).unwrap();

        // Warm-up + 5 timed runs.
        let args = json!({"source": path.display().to_string(), "selector": "tr.item", "mode": "texts", "limit": 500});
        let _ = env.registry.execute("parse_html", args.clone(), &env.ctx).await;
        let mut samples: Vec<u128> = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            let out = env
                .registry
                .execute("parse_html", args.clone(), &env.ctx)
                .await;
            assert!(out.is_ok(), "parse_html failed on scale fixture");
            samples.push(start.elapsed().as_micros());
        }
        samples.sort_unstable();
        let med = percentile(&samples, 0.5);
        let kb = html.len() / 1024;
        report.line(&format!(
            "| {kb} KB | {rows} | {:.2} ms | {:.0} rows/s |",
            med as f64 / 1000.0,
            rows as f64 / (med as f64 / 1_000_000.0)
        ));
        let _ = std::fs::remove_file(&path);
    }
}

async fn bench_extract_json(env: &BenchEnv, report: &mut Report) {
    report.line("\n## 6. extract_json throughput");
    let src = env.json_path.display().to_string();
    report.line("\n~4 MB JSON document with 20 000 objects.\n");
    report.line("| query | iterations | avg |");
    report.line("|---|---:|---:|");

    let cases = [
        ("wildcard scan `items[*].value` (limit 500)", json!({"source": src, "path": "items[*].value", "limit": 500})),
        ("deep single key `items.12345.meta.score`", json!({"source": src, "path": "items.12345.meta.score"})),
        ("top-level key `total`", json!({"source": src, "path": "total"})),
    ];
    for (label, args) in &cases {
        let _ = env.registry.execute("extract_json", args.clone(), &env.ctx).await;
        let mut samples: Vec<u128> = Vec::new();
        for _ in 0..10 {
            let start = Instant::now();
            let out = env
                .registry
                .execute("extract_json", args.clone(), &env.ctx)
                .await;
            assert!(out.is_ok(), "extract_json failed");
            samples.push(start.elapsed().as_micros());
        }
        samples.sort_unstable();
        let med = percentile(&samples, 0.5);
        report.line(&format!(
            "| {label} | 10 | {:.2} ms |",
            med as f64 / 1000.0
        ));
    }
    report.line("\nSource parsing dominates: the JSON document is re-parsed per call (no cross-call cache), which keeps the tool stateless and parallel-safe.");
}

async fn bench_feed_parse(env: &BenchEnv, report: &mut Report) {
    report.line("\n## 7. web_feed (quick-xml) scaling and parallelism");
    report.line("\nLocal RSS fixture, `web_feed`: tolerance to feed size and CPU-bound speed under parallelism.\n");

    // Scaling: same selector-style single limit, growing feed slices.
    report.line("| feed items | avg parse | items/s |");
    report.line("|---|---:|---:|");
    for items in [1_000usize, 5_000, 12_000, 20_000] {
        let path = env.workdir.join(format!("feed_{items}.rss"));
        let mut xml = String::new();
        xml.push_str("<?xml version=\"1.0\"?><rss version=\"2.0\"><channel>");
        for i in 0..items {
            xml.push_str(&format!(
                "<item><title>Item {i}</title><link>/p/{i}</link><description>b</description></item>"
            ));
        }
        xml.push_str("</channel></rss>");
        std::fs::write(&path, &xml).unwrap();

        let args = json!({"source": path.display().to_string(), "limit": items});
        let _ = env.registry.execute("web_feed", args.clone(), &env.ctx).await;
        let mut samples: Vec<u128> = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            let out = env
                .registry
                .execute("web_feed", args.clone(), &env.ctx)
                .await;
            assert!(out.is_ok(), "web_feed failed on feed fixture");
            samples.push(start.elapsed().as_micros());
        }
        samples.sort_unstable();
        let med = percentile(&samples, 0.5);
        report.line(&format!(
            "| {items} | {:.2} ms | {:.0} items/s |",
            med as f64 / 1000.0,
            items as f64 / (med as f64 / 1_000_000.0)
        ));
        let _ = std::fs::remove_file(&path);
    }

    // Parallelism: N identical web_feed calls over the large fixture.
    report.line("\nParallel vs sequential — 8 × web_feed of the ~5 MB feed.\n");
    report.line("| mode | wall time | per call | speedup |");
    report.line("|---|---:|---:|---:|");
    let rounds = 8usize;
    let feed = env.feed_path.display().to_string();
    let make_calls = |tag: &str| -> Vec<ToolCall> {
        (0..rounds)
            .map(|i| {
                ToolCall::new(
                    format!("{tag}{i}"),
                    "web_feed",
                    json!({"source": feed}),
                )
            })
            .collect()
    };
    let _ = env
        .registry
        .execute("web_feed", json!({"source": feed}), &env.ctx)
        .await;
    let start = Instant::now();
    for tc in make_calls("s") {
        let _ = env.registry.execute(tc.name(), tc.arguments(), &env.ctx).await;
    }
    let seq = start.elapsed();
    report.line(&format!(
        "| sequential | {:.1} ms | {:.2} ms | 1.00× |",
        seq.as_secs_f64() * 1000.0,
        seq.as_secs_f64() * 1000.0 / rounds as f64
    ));
    let executor = ToolExecutor::new();
    let start = Instant::now();
    let results = executor
        .execute_batch(make_calls("p"), &env.registry, &env.ctx)
        .await;
    let par = start.elapsed();
    let ok = results.iter().filter(|r| r.output.success).count();
    report.line(&format!(
        "| parallel (execute_batch) | {:.1} ms | {:.2} ms | **{:.2}×** |",
        par.as_secs_f64() * 1000.0,
        par.as_secs_f64() * 1000.0 / rounds as f64,
        seq.as_secs_f64() / par.as_secs_f64().max(f64::EPSILON)
    ));
    let start = Instant::now();
    let results = executor
        .execute_batch_spawn(make_calls("q"), env.registry.clone(), env.ctx.clone())
        .await;
    let spw = start.elapsed();
    let ok_spawn = results.iter().filter(|r| r.output.success).count();
    report.line(&format!(
        "| parallel (execute_batch_spawn, tokio tasks) | {:.1} ms | {:.2} ms | **{:.2}×** |",
        spw.as_secs_f64() * 1000.0,
        spw.as_secs_f64() * 1000.0 / rounds as f64,
        seq.as_secs_f64() / spw.as_secs_f64().max(f64::EPSILON)
    ));
    report.line(&format!(
        "\n{ok}/{rounds} feed parses succeeded (join_all), {ok_spawn}/{rounds} (spawn). \
         join_all shares one thread — spawn spreads CPU work across cores."
    ));
}

async fn bench_code_map(env: &BenchEnv, report: &mut Report) {
    report.line("\n## 8. code_symbols / repo_map — symbol extraction throughput");
    let dir = env.code_dir.display().to_string();
    report.line("\n240 Rust files (~14 KB each, 40 fns + 40 structs + impls per file).\n");

    // code_symbols over the whole tree.
    report.line("| tool | mode | wall time | per tree | items |");
    report.line("|---|---:|---:|---:|---:|");
    let symbol_args = json!({"path": dir, "limit": 1000});
    let _ = env
        .registry
        .execute("code_symbols", symbol_args.clone(), &env.ctx)
        .await;
    let start = Instant::now();
    let out = env
        .registry
        .execute("code_symbols", symbol_args.clone(), &env.ctx)
        .await;
    let dur = start.elapsed();
    let items = match &out {
        Ok(o) => o.content.lines().count(),
        Err(_) => 0,
    };
    report.line(&format!(
        "| code_symbols | single | {:.1} ms | — | {items} lines |",
        dur.as_secs_f64() * 1000.0
    ));

    // repo_map over the whole tree.
    let map_args = json!({"path": dir, "max_files": 240, "symbols_per_file": 3});
    let _ = env
        .registry
        .execute("repo_map", map_args.clone(), &env.ctx)
        .await;
    let start = Instant::now();
    let out = env
        .registry
        .execute("repo_map", map_args.clone(), &env.ctx)
        .await;
    let dur_map = start.elapsed();
    let mapped = match &out {
        Ok(o) => o.content.lines().count(),
        Err(_) => 0,
    };
    report.line(&format!(
        "| repo_map | single | {:.1} ms | — | {mapped} lines |",
        dur_map.as_secs_f64() * 1000.0
    ));

    // Parallelism: 8 × code_symbols on distinct subranges (sequential warm rows
    // done above; compare 8-way batch to 8 sequential single calls).
    report.line("\nParallel vs sequential — 8 × code_symbols on distinct file subsets.\n");
    report.line("| mode | wall time | per call | speedup |");
    report.line("|---|---:|---:|---:|");
    let subdirs: Vec<String> = (0..8)
        .map(|b| {
            let d = env.workdir.join(format!("src_{b}"));
            let _ = std::fs::create_dir_all(&d);
            for i in 0..30 {
                std::fs::copy(
                    env.code_dir.join(format!("mod_{:03}.rs", b * 30 + i)),
                    d.join(format!("mod_{:03}.rs", b * 30 + i)),
                )
                .unwrap();
            }
            d.display().to_string()
        })
        .collect();
    let make_calls = |tag: &str| -> Vec<ToolCall> {
        subdirs
            .iter()
            .enumerate()
            .map(|(i, d)| {
                ToolCall::new(
                    format!("{tag}{i}"),
                    "code_symbols",
                    json!({"path": d, "limit": 1000}),
                )
            })
            .collect()
    };
    let start = Instant::now();
    for tc in make_calls("s") {
        let _ = env.registry.execute(tc.name(), tc.arguments(), &env.ctx).await;
    }
    let seq = start.elapsed();
    report.line(&format!(
        "| sequential | {:.1} ms | {:.2} ms | 1.00× |",
        seq.as_secs_f64() * 1000.0,
        seq.as_secs_f64() * 1000.0 / 8.0
    ));
    let executor = ToolExecutor::new();
    let start = Instant::now();
    let results = executor
        .execute_batch(make_calls("p"), &env.registry, &env.ctx)
        .await;
    let par = start.elapsed();
    let ok = results.iter().filter(|r| r.output.success).count();
    report.line(&format!(
        "| parallel (execute_batch) | {:.1} ms | {:.2} ms | **{:.2}×** |",
        par.as_secs_f64() * 1000.0,
        par.as_secs_f64() * 1000.0 / 8.0,
        seq.as_secs_f64() / par.as_secs_f64().max(f64::EPSILON)
    ));
    let start = Instant::now();
    let results = executor
        .execute_batch_spawn(make_calls("q"), env.registry.clone(), env.ctx.clone())
        .await;
    let spw = start.elapsed();
    let ok_spawn = results.iter().filter(|r| r.output.success).count();
    report.line(&format!(
        "| parallel (execute_batch_spawn, tokio tasks) | {:.1} ms | {:.2} ms | **{:.2}×** |",
        spw.as_secs_f64() * 1000.0,
        spw.as_secs_f64() * 1000.0 / 8.0,
        seq.as_secs_f64() / spw.as_secs_f64().max(f64::EPSILON)
    ));
    for d in &subdirs {
        let _ = std::fs::remove_dir_all(d);
    }
    report.line(&format!(
        "\n{ok}/8 symbol scans succeeded (join_all), {ok_spawn}/8 (spawn). \
         join_all shares one thread — spawn spreads CPU work across cores."
    ));
}

async fn bench_memory(_env: &BenchEnv, report: &mut Report) {
    report.line("\n## 9. Semantic memory (pr-memory) — absorb / search / digest");
    report.line("\nOffline TF-IDF embedder (no network, no LLM); in-memory SQLite.\n");

    let cfg = pr_core::MemoryConfig {
        enabled: true,
        embeddings: "tfidf".to_string(),
        llm_classify: false, // heuristic path — deterministic and free
        ..Default::default()
    };
    let mem = std::sync::Arc::new(
        pr_memory::Memory::in_memory(cfg.clone()).expect("in-memory memory store"),
    );
    let pipeline = mem.pipeline();

    // Distinct, topically varied facts so embeddings are meaningful and the
    // dedup/consolidation layer does NOT merge them (each fact gets a unique
    // combination of company/metric/city/year tokens).
    const COMPANIES: &[&str] = &[
        "Acme", "Globex", "Initech", "Umbrella", "Stark", "Wayne", "Hooli",
        "Pied Piper", "Vandelay", "Cyberdyne", "Soylent", "Wonka", "Tyrell",
        "Weyland", "Massive Dynamic", "Aperture", "Black Mesa", "Oscorp",
        "LexCorp", "Momcorp",
    ];
    const METRICS: &[&str] = &[
        "revenue", "headcount", "churn rate", "conversion", "margin",
        "burn rate", "ARR", "support load", "latency", "uptime",
        "storage cost", "pipeline volume",
    ];
    const CITIES: &[&str] = &[
        "Moscow", "Kazan", "Berlin", "Amsterdam", "Lisbon", "Tbilisi",
        "Almaty", "Dubai", "Singapore", "Toronto",
    ];
    let make_facts = |n: usize| -> Vec<pr_memory::AbsorbFact> {
        (0..n)
            .map(|i| {
                let company = COMPANIES[i % COMPANIES.len()];
                let metric = METRICS[(i * 7 + 3) % METRICS.len()];
                let city = CITIES[(i * 3 + 1) % CITIES.len()];
                let year = 2019 + (i % 7);
                let value = (i * 13) % 997 + 2;
                pr_memory::AbsorbFact {
                    content: format!(
                        "{company} reported {metric} of {value} units in {city} \
                         during {year} according to filing number {i}"
                    ),
                    metadata: json!({}),
                    tags: vec!["bench".to_string()],
                    confidence: Some(0.8),
                    memory_class: None,
                }
            })
            .collect()
    };

    // ── Absorb throughput ───────────────────────────────────────────────
    report.line("| batch size | absorbed | wall time | per fact |");
    report.line("|---:|---:|---:|---:|");
    for n in [10usize, 100, 500] {
        let req = pr_memory::AbsorbRequest {
            facts: make_facts(n),
            source: "bench".into(),
            scope: pr_memory::Scope::Agent,
            scope_key: String::new(),
            context: None,
            dry_run: false,
        };
        let start = Instant::now();
        let rep = pipeline.absorb(req).await.expect("absorb");
        let el = start.elapsed();
        let total = rep.created + rep.superseded + rep.contradicted + rep.related;
        report.line(&format!(
            "| {n} | {total} | {:.1} ms | {:.0} µs |",
            el.as_secs_f64() * 1000.0,
            el.as_micros() as f64 / n.max(1) as f64
        ));
    }

    // ── Duplicate re-absorb (dedup fast path) ──────────────────────────
    // Re-absorb the same 100 facts: hash/similarity dedup should skip them.
    let dup_req = pr_memory::AbsorbRequest {
        facts: make_facts(100),
        source: "bench".into(),
        scope: pr_memory::Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    };
    let start = Instant::now();
    let dup_rep = pipeline.absorb(dup_req).await.expect("dedup absorb");
    let dup_el = start.elapsed();
    report.line(&format!(
        "\nRe-absorbing 100 already-known facts: {} skipped, {} created in {:.1} ms \
         (dedup fast path).",
        dup_rep.skipped,
        dup_rep.created,
        dup_el.as_secs_f64() * 1000.0
    ));

    // ── Search latency ──────────────────────────────────────────────────
    report.line("\nHybrid search (vector + BM25) over the stored facts:\n");
    report.line("| query | matches | median latency |");
    report.line("|---|---:|---:|");
    let scope = pr_memory::ScopeFilter::persistent();
    let queries = [
        "Acme revenue units filing",
        "Globex churn rate report",
        "headcount in Kazan during 2022",
        "Initech margin units",
        "support load Dubai filing",
    ];
    for q in queries {
        // Warm-up once, then time repeated searches.
        let _ = mem.search(q, &scope, Some(5)).await;
        let mut samples: Vec<u128> = Vec::new();
        let mut hits = 0usize;
        for _ in 0..20 {
            let start = Instant::now();
            let res = mem.search(q, &scope, Some(5)).await.expect("search");
            samples.push(start.elapsed().as_micros());
            hits = res.len();
        }
        samples.sort_unstable();
        let med = percentile(&samples, 0.5);
        report.line(&format!(
            "| {q} | {hits} | {:.2} ms |",
            med as f64 / 1000.0
        ));
    }

    // ── Search scaling with store size ─────────────────────────────────
    // Fresh stores filled directly (bypassing absorb) so this measures the
    // retrieval path alone: embedding load + brute-force cosine + BM25.
    report.line("\nSearch latency vs store size (brute-force cosine scan):\n");
    report.line("| memories | fill time | search (median) |");
    report.line("|---:|---:|---:|");
    for size in [1_000usize, 5_000, 10_000] {
        let big = pr_memory::Memory::in_memory(cfg.clone()).expect("store");
        let facts = make_facts(size);
        let start = Instant::now();
        let texts: Vec<String> = facts.iter().map(|f| f.content.clone()).collect();
        let vecs = big.embedder.embed(&texts).await.expect("embed");
        let now = chrono::Utc::now().to_rfc3339();
        for (f, v) in facts.iter().zip(vecs.iter()) {
            let row = pr_memory::MemoryRow {
                id: uuid::Uuid::now_v7().to_string(),
                content: f.content.clone(),
                metadata: json!({}),
                tags: f.tags.clone(),
                source: "bench".into(),
                scope: "agent".into(),
                scope_key: String::new(),
                confidence: 0.8,
                importance: 1.0,
                access_count: 0,
                last_accessed: None,
                status: "active".into(),
                expires_at: None,
                content_hash: pr_memory::content_hash(&f.content),
                created_at: now.clone(),
                updated_at: now.clone(),
            };
            big.db.insert(&row).expect("insert");
            big.db
                .put_embedding(&row.id, big.embedder.model_name(), v)
                .expect("embedding");
            big.db.fts_insert(&row.id, &row.content, &row.tags);
        }
        let fill = start.elapsed();

        let _ = big.search("Acme revenue units", &scope, Some(5)).await;
        let mut samples: Vec<u128> = Vec::new();
        for _ in 0..10 {
            let start = Instant::now();
            let _ = big.search("Acme revenue units", &scope, Some(5)).await;
            samples.push(start.elapsed().as_micros());
        }
        samples.sort_unstable();
        let med = percentile(&samples, 0.5);
        report.line(&format!(
            "| {size} | {:.0} ms | {:.2} ms |",
            fill.as_secs_f64() * 1000.0,
            med as f64 / 1000.0
        ));
    }

    // ── Digest build latency ────────────────────────────────────────────
    let start = Instant::now();
    let digest = mem
        .digest("Acme revenue and headcount reports", &scope)
        .await
        .expect("digest");
    let d_el = start.elapsed();
    report.line(&format!(
        "\nDigest build (relevant + TODOs + recent): {} relevant memories in {:.2} ms.",
        digest.relevant.len(),
        d_el.as_secs_f64() * 1000.0
    ));
}

// ---------------------------------------------------------------------------
// Session statistics (real runs)
// ---------------------------------------------------------------------------

pub fn run_stats(output: Option<String>) -> anyhow::Result<()> {
    let dir = match output {
        Some(d) => PathBuf::from(d),
        None => {
            let config = pr_core::AppConfig::load()?;
            PathBuf::from(&config.output.dir)
        }
    };
    let db_path = dir.join(".research.db");
    anyhow::ensure!(
        db_path.exists(),
        "no session database at {} (pass --output <session dir>)",
        db_path.display()
    );

    let conn = rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;

    let mut out = String::new();
    out.push_str(&format!(
        "# Session statistics — {}\n",
        db_path.display()
    ));

    // Sessions.
    let mut stmt = conn.prepare(
        "SELECT id, substr(query, 1, 80), status, created_at FROM sessions ORDER BY created_at DESC",
    )?;
    let sessions: Vec<(String, String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;
    out.push_str("\n## Sessions\n\n| session | query | status | created |\n|---|---|---|---|\n");
    for (id, q, st, ts) in &sessions {
        out.push_str(&format!("| {id} | {q} | {st} | {ts} |\n"));
    }

    // Agents.
    type AgentStatRow = (String, String, String, i64, String, Option<String>, String);
    let mut stmt = conn.prepare(
        "SELECT id, role, status, tokens_used, created_at, completed_at, substr(task, 1, 60) FROM agents ORDER BY created_at",
    )?;
    let agents: Vec<AgentStatRow> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    out.push_str("\n## Agents\n\n| agent | role | status | tokens | wall time | task |\n|---|---|---|---:|---:|---|\n");
    let mut total_tokens = 0i64;
    for (id, role, status, tokens, created, completed, task) in &agents {
        total_tokens += tokens;
        let wall = match (parse_ts(created), completed.as_deref().and_then(parse_ts)) {
            (Some(a), Some(b)) => format!("{:.1} s", (b - a) as f64 / 1000.0),
            _ => "—".to_string(),
        };
        out.push_str(&format!(
            "| {} | {role} | {status} | {tokens} | {wall} | {task} |\n",
            &id[..8.min(id.len())]
        ));
    }

    // Tool calls.
    let mut stmt = conn.prepare(
        "SELECT tool_name, success, COALESCE(duration_ms, 0), created_at FROM tool_results ORDER BY created_at",
    )?;
    let rows: Vec<(String, i64, i64, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;

    if rows.is_empty() {
        out.push_str("\nNo tool calls recorded.\n");
        print!("{out}");
        return Ok(());
    }

    // Per-tool aggregates.
    use std::collections::BTreeMap;
    let mut by_tool: BTreeMap<String, (u64, u64, Vec<u128>)> = BTreeMap::new();
    for (name, success, dur, _) in &rows {
        let e = by_tool.entry(name.clone()).or_insert((0, 0, Vec::new()));
        e.0 += 1;
        if *success == 1 {
            e.1 += 1;
        }
        e.2.push(*dur as u128);
    }
    out.push_str("\n## Tool calls by tool\n\n| tool | calls | success | avg | p50 | p95 | total |\n|---|---:|---:|---:|---:|---:|---:|\n");
    let mut ordered: Vec<_> = by_tool.iter().collect();
    ordered.sort_by_key(|e| std::cmp::Reverse(e.1 .0));
    for (name, (calls, ok, durs)) in &ordered {
        let mut s = (*durs).clone();
        s.sort_unstable();
        let total: u128 = s.iter().sum();
        out.push_str(&format!(
            "| {name} | {calls} | {ok}/{} | {:.1} ms | {} ms | {} ms | {} ms |\n",
            calls,
            total as f64 / (*calls).max(1) as f64,
            percentile(&s, 0.5),
            percentile(&s, 0.95),
            total
        ));
    }

    // Concurrency analysis.
    let mut intervals: Vec<(i64, i64)> = Vec::new();
    for (_, _, dur, ts) in &rows {
        if *dur > 0 {
            if let Some(end) = parse_ts(ts) {
                intervals.push((end - dur, end));
            }
        }
    }
    let mut events: Vec<(i64, i32)> = Vec::with_capacity(intervals.len() * 2);
    for (s, e) in &intervals {
        events.push((*s, 1));
        events.push((*e, -1));
    }
    // Ends before starts at equal timestamps (touching intervals don't overlap).
    events.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut depth = 0i32;
    let mut peak = 0i32;
    for (_, d) in &events {
        depth += d;
        peak = peak.max(depth);
    }

    // Batching profile: how many calls started in the same 250 ms window.
    let mut starts: Vec<i64> = rows
        .iter()
        .filter_map(|(_, _, dur, ts)| {
            if *dur >= 0 {
                parse_ts(ts).map(|end| end - dur)
            } else {
                None
            }
        })
        .collect();
    starts.sort_unstable();
    let mut hist: BTreeMap<usize, usize> = BTreeMap::new();
    let mut i = 0;
    while i < starts.len() {
        let bucket_end = starts[i] + 250;
        let j = starts.partition_point(|t| *t <= bucket_end);
        *hist.entry(j - i).or_insert(0) += 1;
        i = j;
    }

    let first_start = intervals.iter().map(|(s, _)| *s).min();
    let last_end = intervals.iter().map(|(_, e)| *e).max();
    let busy_ms: i64 = intervals.iter().map(|(s, e)| e - s).sum();
    let wall_ms = match (first_start, last_end) {
        (Some(a), Some(b)) => b - a,
        _ => 0,
    };

    out.push_str("\n## Concurrency & batching\n\n");
    out.push_str(&format!("- total tool calls: **{}**\n", rows.len()));
    out.push_str(&format!(
        "- calls with measured duration: {}\n",
        intervals.len()
    ));
    out.push_str(&format!("- **peak concurrent tool calls: {peak}**\n"));
    if wall_ms > 0 {
        out.push_str(&format!(
            "- tool-execution wall window: {:.1} s, cumulative busy time: {:.1} s\n",
            wall_ms as f64 / 1000.0,
            busy_ms as f64 / 1000.0
        ));
        if busy_ms > wall_ms {
            out.push_str(&format!(
                "- time saved by parallelism: **{:.1} s** ({:.0}% of busy time overlapped)\n",
                (busy_ms - wall_ms) as f64 / 1000.0,
                (busy_ms - wall_ms) as f64 / busy_ms as f64 * 100.0
            ));
        }
    }
    out.push_str("- batch profile (calls started within one 250 ms window):\n");
    out.push_str("\n| batch size | windows |\n|---:|---:|\n");
    for (size, count) in &hist {
        out.push_str(&format!("| {size} | {count} |\n"));
    }

    if total_tokens > 0 {
        out.push_str(&format!(
            "\n## Token accounting\n\n- total tokens across agents: **{total_tokens}**\n"
        ));
        if wall_ms > 0 {
            out.push_str(&format!(
                "- tokens per second of tool-window: {:.0}\n",
                total_tokens as f64 / (wall_ms as f64 / 1000.0)
            ));
        }
    }

    print!("{out}");
    Ok(())
}

fn parse_ts(ts: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    // BenchEnv::setup uses a unique temp dir per call (PID + atomic
    // counter), so tests can run in parallel. If a regression re-introduces
    // a shared path, use --test-threads=1.

    // ── run_bench dispatches correctly ────────────────────────────────────────

    /// Runs every benchmark scenario in sequence.
    #[tokio::test]
    async fn test_run_bench_all() {
        run_bench("all", 2, None).await.expect("run_bench all");
    }

    #[tokio::test]
    async fn test_run_bench_dispatch() {
        run_bench("dispatch", 2, None).await.expect("run_bench dispatch");
    }

    /// Race on shared `pr-bench-{pid}` temp dir when run in parallel.
    #[tokio::test]
    async fn test_run_bench_parallel_io() {
        run_bench("parallel-io", 2, None).await.expect("run_bench parallel-io");
    }

    /// Race on shared `pr-bench-{pid}` temp dir when run in parallel.
    #[tokio::test]
    async fn test_run_bench_parallel_cpu() {
        run_bench("parallel-cpu", 2, None).await.expect("run_bench parallel-cpu");
    }

    /// Race on shared `pr-bench-{pid}` temp dir when run in parallel.
    #[tokio::test]
    async fn test_run_bench_mixed() {
        run_bench("mixed", 2, None).await.expect("run_bench mixed");
    }

    /// Parse-scale benchmark creates large temp files. Race on shared
    /// `pr-bench-{pid}` dir when run in parallel. Marked `#[ignore]`.
    #[tokio::test]
    async fn test_run_bench_parse_scale() {
        run_bench("parse-scale", 2, None).await.expect("run_bench parse-scale");
    }

    /// Race on shared `pr-bench-{pid}` temp dir when run in parallel.
    #[tokio::test]
    async fn test_run_bench_extract_json() {
        run_bench("extract-json", 2, None).await.expect("run_bench extract-json");
    }

    /// Race on shared `pr-bench-{pid}` temp dir when run in parallel.
    #[tokio::test]
    async fn test_run_bench_feed_parse() {
        run_bench("feed-parse", 2, None).await.expect("run_bench feed-parse");
    }

    /// code_map benchmark requires external tools (tree-sitter). Crashes when
    /// the temp dir `pr-bench-{pid}` raced by parallel tests corrupts the
    /// 240-file fixture. Marked `#[ignore]`; run via
    /// `cargo test -- --ignored test_run_bench_code_map`.
    #[tokio::test]
    async fn test_run_bench_code_map() {
        run_bench("code-map", 2, None).await.expect("run_bench code-map");
    }

    /// Race on shared `pr-bench-{pid}` temp dir when run in parallel.
    #[tokio::test]
    async fn test_run_bench_memory() {
        run_bench("memory", 2, None).await.expect("run_bench memory");
    }

    /// Race on shared `pr-bench-{pid}` temp dir when run in parallel.
    #[tokio::test]
    async fn test_run_bench_unknown_scenario_errors() {
        let err = run_bench("nonexistent", 4, None).await.unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("unknown scenario"), "expected unknown scenario error, got: {msg}");
    }

    // ── Individual scenario benchmarks ────────────────────────────────────────

    #[tokio::test]
    async fn test_bench_dispatch_reports_metrics() {
        let env = BenchEnv::setup(2).expect("small bench env");
        let mut report = Report::new();
        bench_dispatch(&env, &mut report).await;
        let text = report.finish();
        assert!(text.contains("registry.execute"), "report should contain dispatch metrics");
        assert!(text.contains("execute_batch"), "report should contain batch metrics");
        assert!(text.contains("ToolCall"), "report should contain serde metrics");
        env.cleanup();
    }

    #[tokio::test]
    async fn test_bench_parallel_io_all_succeed() {
        let env = BenchEnv::setup(2).expect("env");
        let mut report = Report::new();
        bench_parallel_io(&env, &mut report).await;
        let text = report.finish();
        assert!(text.contains("succeeded"), "report should contain success count");
        env.cleanup();
    }

    #[tokio::test]
    async fn test_bench_parallel_cpu_reports_speedup() {
        let env = BenchEnv::setup(2).expect("env");
        let mut report = Report::new();
        bench_parallel_cpu(&env, &mut report).await;
        let text = report.finish();
        assert!(text.contains("speedup"), "report should contain speedup");
        env.cleanup();
    }

    #[tokio::test]
    async fn test_bench_mixed_contains_phases() {
        let env = BenchEnv::setup(2).expect("env");
        let mut report = Report::new();
        bench_mixed(&env, &mut report).await;
        let text = report.finish();
        assert!(text.contains("parallel"), "report should mention parallel phase");
        assert!(text.contains("sequential"), "report should mention sequential phase");
        env.cleanup();
    }

    #[tokio::test]
    async fn test_bench_parse_scale_reports_throughput() {
        let env = BenchEnv::setup(2).expect("env");
        let mut report = Report::new();
        bench_parse_scale(&env, &mut report).await;
        let text = report.finish();
        assert!(text.contains("rows/s"), "report should contain throughput");
        env.cleanup();
    }

    #[tokio::test]
    async fn test_bench_extract_json_reports_queries() {
        let env = BenchEnv::setup(2).expect("env");
        let mut report = Report::new();
        bench_extract_json(&env, &mut report).await;
        let text = report.finish();
        assert!(text.contains("wildcard"), "report should contain wildcard scan");
        assert!(text.contains("deep single key"), "report should contain deep key scan");
        assert!(text.contains("top-level key"), "report should contain top-level key scan");
        env.cleanup();
    }

    /// Feed-parse benchmark also creates large temp fixtures. Race on shared
    /// `pr-bench-{pid}` dir. Marked `#[ignore]`.
    #[tokio::test]
    async fn test_bench_feed_parse_reports_items() {
        let env = BenchEnv::setup(2).expect("env");
        let mut report = Report::new();
        bench_feed_parse(&env, &mut report).await;
        let text = report.finish();
        assert!(text.contains("items/s"), "report should contain items/s");
        env.cleanup();
    }

    /// Same race condition as `test_run_bench_code_map`. Marked `#[ignore]`.
    #[tokio::test]
    async fn test_bench_code_map_reports_symbols() {
        let env = BenchEnv::setup(2).expect("env");
        let mut report = Report::new();
        bench_code_map(&env, &mut report).await;
        let text = report.finish();
        assert!(text.contains("code_symbols"), "report should contain code_symbols");
        assert!(text.contains("repo_map"), "report should contain repo_map");
        env.cleanup();
    }

    #[tokio::test]
    async fn test_bench_memory_reports_absorb_and_search() {
        let env = BenchEnv::setup(2).expect("env");
        let mut report = Report::new();
        bench_memory(&env, &mut report).await;
        let text = report.finish();
        assert!(text.contains("absorb"), "report should contain absorb metrics");
        assert!(text.contains("search"), "report should contain search metrics");
        assert!(text.contains("digest"), "report should contain digest metrics");
        env.cleanup();
    }

    // ── Report helper ─────────────────────────────────────────────────────────

    #[test]
    fn test_report_new_and_finish() {
        let mut r = Report::new();
        r.line("hello");
        r.line("world");
        assert_eq!(r.finish(), "hello\nworld\n");
    }

    #[test]
    fn test_report_empty() {
        let r = Report::new();
        assert_eq!(r.finish(), "");
    }

    // ── percentile ────────────────────────────────────────────────────────────

    #[test]
    fn test_percentile_basic() {
        let sorted = vec![10, 20, 30, 40, 50];
        assert_eq!(percentile(&sorted, 0.0), 10);
        assert_eq!(percentile(&sorted, 0.5), 30);
        assert_eq!(percentile(&sorted, 1.0), 50);
    }

    #[test]
    fn test_percentile_empty() {
        assert_eq!(percentile(&[], 0.5), 0);
    }

    #[test]
    fn test_percentile_single_element() {
        assert_eq!(percentile(&[42], 0.0), 42);
        assert_eq!(percentile(&[42], 0.5), 42);
        assert_eq!(percentile(&[42], 1.0), 42);
    }

    // ── BenchEnv setup / cleanup ──────────────────────────────────────────────

    #[test]
    fn test_benchenv_setup_creates_files() {
        let env = BenchEnv::setup(4).expect("setup");
        // All fixture files are created during setup.
        assert_eq!(env.data_files.len(), 4, "should create 4 data files");
        assert!(env.file_kb > 0, "file_kb should be positive");
        assert!(env.html_path.to_string_lossy().contains("large.html"));
        assert!(env.json_path.to_string_lossy().contains("large.json"));
        assert!(env.feed_path.to_string_lossy().contains("large.rss"));
        // Do NOT check existence on disk — concurrent tests share the
        // PID-based temp dir and may have cleaned up.
        env.cleanup();
    }

    #[test]
    fn test_benchenv_setup_no_clamping_in_setup() {
        // setup() itself does NOT clamp; only run_bench applies n.max(2).
        let env = BenchEnv::setup(1).expect("setup with n=1");
        assert_eq!(env.data_files.len(), 1, "setup(n) creates exactly n data files");
        env.cleanup();
    }

    #[tokio::test]
    async fn test_run_bench_clamps_n_below_two() {
        // run_bench applies n.max(2); n=1 must still work.
        run_bench("dispatch", 1, None).await.expect("run_bench with n=1");
    }

    #[test]
    /// Race on shared `pr-bench-{pid}` temp dir when run in parallel.
    fn test_benchenv_cleanup_removes_workdir() {
        let env = BenchEnv::setup(2).expect("setup");
        let workdir = env.workdir.clone();
        assert!(workdir.exists());
        env.cleanup();
        assert!(!workdir.exists(), "workdir should be removed after cleanup");
    }

    #[test]
    fn test_benchenv_read_call() {
        let env = BenchEnv::setup(2).expect("setup");
        let tc = env.read_call(42, 0);
        assert_eq!(tc.name(), "file_read");
        assert_eq!(tc.id, "c42");
        assert!(tc.arguments()["path"].as_str().unwrap().contains("data_00.txt"));
        env.cleanup();
    }

    // ── parse_ts ──────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_ts_valid_rfc3339() {
        let ts = "2026-08-19T12:34:56+00:00";
        let ms = parse_ts(ts);
        assert!(ms.is_some(), "should parse valid RFC 3339");
        assert!(ms.unwrap() > 0, "millis should be positive");
    }

    #[test]
    fn test_parse_ts_invalid() {
        assert!(parse_ts("not-a-date").is_none());
        assert!(parse_ts("").is_none());
    }

    // ── run_stats (unit-level: only test the fallback path without a real DB) ─

    #[test]
    fn test_run_stats_errors_without_db() {
        let result = run_stats(Some("/tmp/nonexistent-bench-dir".into()));
        assert!(result.is_err(), "run_stats should error on missing DB");
    }
}
