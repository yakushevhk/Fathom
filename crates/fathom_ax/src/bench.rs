//! AX benchmark suite: synthetic high-load scenarios for the controller,
//! event log, and actor lifecycle. Runs as `fathom ax bench` and as
//! `#[tokio::test]` smoke benchmarks.
//!
//! Every scenario returns a [`BenchResult`]: ops throughput, a latency
//! distribution, peak RSS (`VmHWM`), and a failure count.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use serde::Serialize;

use crate::manifest::phase;
use crate::{AxController, AxManifest, AxResult, AxStore, ObjectMeta};

/// Latency distribution in milliseconds.
#[derive(Debug, Clone, Serialize)]
pub struct LatencyStats {
    pub count: usize,
    pub min_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
    pub mean_ms: f64,
}

impl LatencyStats {
    pub fn from_ms(samples: &mut [f64]) -> Self {
        samples.sort_by(f64::total_cmp);
        let pick = |q: f64| {
            if samples.is_empty() {
                0.0
            } else {
                samples[((samples.len() - 1) as f64 * q) as usize]
            }
        };
        Self {
            count: samples.len(),
            min_ms: samples.first().copied().unwrap_or(0.0),
            p50_ms: pick(0.50),
            p95_ms: pick(0.95),
            p99_ms: pick(0.99),
            max_ms: samples.last().copied().unwrap_or(0.0),
            mean_ms: if samples.is_empty() {
                0.0
            } else {
                samples.iter().sum::<f64>() / samples.len() as f64
            },
        }
    }
}

/// One scenario's outcome.
#[derive(Debug, Clone, Serialize)]
pub struct BenchResult {
    pub name: String,
    pub ops: usize,
    pub wall_ms: f64,
    pub ops_per_sec: f64,
    pub latency: LatencyStats,
    /// Peak resident memory of the process at the end of the run (VmHWM).
    pub peak_rss_kb: u64,
    pub failures: usize,
}

/// Peak resident set size (VmHWM) of this process, in KiB.
pub fn peak_rss_kb() -> u64 {
    let Ok(s) = std::fs::read_to_string("/proc/self/status") else {
        return 0;
    };
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("VmHWM:") {
            return rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
        }
    }
    0
}

fn bench_task_yaml(name: &str, command: &str) -> String {
    format!(
        r#"apiVersion: ax.io/v1alpha1
kind: Task
metadata:
  name: {name}
spec:
  command: [{command}]
"#
    )
}

/// Parse `AxManifest` documents: ops/sec for the decoder + validator.
pub fn manifest_parse(n: usize) -> AxResult<BenchResult> {
    let yaml = bench_task_yaml("bench-parse", "\"echo\", \"hi\"");
    let mut lat = Vec::with_capacity(n);
    let start = Instant::now();
    let mut failures = 0;
    for _ in 0..n {
        let t = Instant::now();
        match AxManifest::parse_documents(&yaml) {
            Ok(d) if !d.is_empty() => {}
            _ => failures += 1,
        }
        lat.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let wall = start.elapsed().as_secs_f64() * 1000.0;
    Ok(BenchResult {
        name: "manifest-parse".into(),
        ops: n,
        wall_ms: wall,
        ops_per_sec: n as f64 / (wall / 1000.0).max(f64::EPSILON),
        latency: LatencyStats::from_ms(&mut lat),
        peak_rss_kb: peak_rss_kb(),
        failures,
    })
}

/// Durable event-log append throughput (single-writer, serialized through
/// the store mutex) plus a full sequential scan back.
pub fn event_log_append(store: &AxStore, n: usize) -> AxResult<BenchResult> {
    let meta = ObjectMeta {
        name: "bench-events".into(),
        atespace: "bench".into(),
        creation_timestamp: None,
    };
    let mut lat = Vec::with_capacity(n);
    let start = Instant::now();
    let mut failures = 0;
    let seq = store.latest_seq()?;
    for i in 0..n {
        let t = Instant::now();
        if store
            .append_event("task", &meta, "bench", &serde_json::json!({"i": i}))
            .is_err()
        {
            failures += 1;
        }
        lat.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    // Plus one full scan of what we appended.
    let scan = store
        .scan_all(seq + 1, n as i64)
        .map(|e| e.len())
        .unwrap_or(0);
    if scan < n {
        failures += 1;
    }
    let wall = start.elapsed().as_secs_f64() * 1000.0;
    Ok(BenchResult {
        name: "event-log-append".into(),
        ops: n,
        wall_ms: wall,
        ops_per_sec: n as f64 / (wall / 1000.0).max(f64::EPSILON),
        latency: LatencyStats::from_ms(&mut lat),
        peak_rss_kb: peak_rss_kb(),
        failures,
    })
}

/// `apply_documents` throughput: N task manifests through the reconciler
/// (ref resolution + upsert + record + publish), serialized by the
/// single-writer design.
pub fn apply_dispatch(ctl: &Arc<AxController>, n: usize) -> AxResult<BenchResult> {
    let mut lat = Vec::with_capacity(n);
    let mut failures = 0;
    let start = Instant::now();
    for i in 0..n {
        let yaml = bench_task_yaml(&format!("bench-apply-{i}"), "\"true\"");
        let t = Instant::now();
        if ctl.apply_documents(&yaml).is_err() {
            failures += 1;
        }
        lat.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let wall = start.elapsed().as_secs_f64() * 1000.0;
    Ok(BenchResult {
        name: "apply-dispatch".into(),
        ops: n,
        wall_ms: wall,
        ops_per_sec: n as f64 / (wall / 1000.0).max(f64::EPSILON),
        latency: LatencyStats::from_ms(&mut lat),
        peak_rss_kb: peak_rss_kb(),
        failures,
    })
}

async fn wait_phase(store: &AxStore, atespace: &str, name: &str, want: &[&str], secs: u64) -> bool {
    let start = Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(secs) {
        if let Ok(t) = store.get_task(atespace, name) {
            if want.contains(&t.status.phase.as_str()) {
                return true;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    false
}

/// Concurrent actors: apply `n` tasks at once, all must reach a terminal
/// phase. Measures the controller's dispatch throughput under parallel
/// load and the process peak RSS.
pub async fn concurrent_actors(
    ctl: &Arc<AxController>,
    atespace: &str,
    n: usize,
    timeout_secs: u64,
) -> AxResult<BenchResult> {
    let start = Instant::now();
    let mut failures = 0;
    for i in 0..n {
        let yaml = bench_task_yaml(&format!("bench-c-{i}"), "\"echo\", \"ok\"");
        if ctl.apply_documents(&yaml).is_err() {
            failures += 1;
        }
    }
    // Wait for all to reach a terminal phase.
    let mut lat = Vec::with_capacity(n);
    for i in 0..n {
        let t = Instant::now();
        let name = format!("bench-c-{i}");
        if !wait_phase(
            ctl.store(),
            atespace,
            &name,
            &[phase::COMPLETED, phase::FAILED, phase::INTERRUPTED],
            timeout_secs,
        )
        .await
        {
            failures += 1;
        }
        lat.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let wall = start.elapsed().as_secs_f64() * 1000.0;
    Ok(BenchResult {
        name: format!("concurrent-actors-{n}"),
        ops: n,
        wall_ms: wall,
        ops_per_sec: n as f64 / (wall / 1000.0).max(f64::EPSILON),
        latency: LatencyStats::from_ms(&mut lat),
        peak_rss_kb: peak_rss_kb(),
        failures,
    })
}

/// Lifecycle latency: apply → Running → Suspended → Running → terminal.
/// Each transition's wall time lands in the latency distribution.
pub async fn lifecycle(
    ctl: &Arc<AxController>,
    atespace: &str,
    n: usize,
    timeout_secs: u64,
) -> AxResult<BenchResult> {
    let mut lat = Vec::with_capacity(n * 4);
    let mut failures = 0;
    let start = Instant::now();
    for i in 0..n {
        let name = format!("bench-lc-{i}");
        let yaml = bench_task_yaml(&name, "\"sh\", \"-c\", \"sleep 0.4\"");
        let mut t = Instant::now();
        if ctl.apply_documents(&yaml).is_err() {
            failures += 1;
            continue;
        }
        lat.push(t.elapsed().as_secs_f64() * 1000.0);
        t = Instant::now();
        if !wait_phase(
            ctl.store(),
            atespace,
            &name,
            &[phase::RUNNING],
            timeout_secs,
        )
        .await
        {
            failures += 1;
            continue;
        }
        lat.push(t.elapsed().as_secs_f64() * 1000.0);
        t = Instant::now();
        if ctl.suspend_task(atespace, &name).await.is_err() {
            failures += 1;
            continue;
        }
        lat.push(t.elapsed().as_secs_f64() * 1000.0);
        t = Instant::now();
        if ctl.resume_task(atespace, &name).await.is_err() {
            failures += 1;
            continue;
        }
        lat.push(t.elapsed().as_secs_f64() * 1000.0);
        t = Instant::now();
        let _ = wait_phase(
            ctl.store(),
            atespace,
            &name,
            &[phase::COMPLETED, phase::FAILED, phase::INTERRUPTED],
            timeout_secs,
        )
        .await;
        lat.push(t.elapsed().as_secs_f64() * 1000.0);
        let _ = ctl.delete_task(atespace, &name).await;
    }
    let wall = start.elapsed().as_secs_f64() * 1000.0;
    Ok(BenchResult {
        name: format!("lifecycle-{n}"),
        ops: n,
        wall_ms: wall,
        ops_per_sec: n as f64 / (wall / 1000.0).max(f64::EPSILON),
        latency: LatencyStats::from_ms(&mut lat),
        peak_rss_kb: peak_rss_kb(),
        failures,
    })
}

/// Run the full suite against an isolated state dir and return all results.
pub async fn run_suite(
    state_dir: &Path,
    tasks: usize,
    timeout_secs: u64,
) -> AxResult<Vec<BenchResult>> {
    let ctl = AxController::open(state_dir)?;
    let mut out = Vec::new();
    out.push(manifest_parse(200)?);
    out.push(event_log_append(ctl.store(), tasks * 20)?);
    out.push(apply_dispatch(&ctl, tasks)?);
    out.push(concurrent_actors(&ctl, "default", tasks, timeout_secs).await?);
    out.push(lifecycle(&ctl, "default", tasks.min(20), timeout_secs).await?);
    Ok(out)
}

/// Render the suite results as a fixed-width report.
pub fn render(results: &[BenchResult]) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "{:<24} {:>6} {:>10} {:>12} {:>9} {:>9} {:>9} {:>10} {:>7}\n",
        "SCENARIO", "OPS", "WALL(ms)", "OPS/s", "p50(ms)", "p95(ms)", "p99(ms)", "RSS(KiB)", "FAIL"
    ));
    for r in results {
        s.push_str(&format!(
            "{:<24} {:>6} {:>10.0} {:>12.0} {:>9.1} {:>9.1} {:>9.1} {:>10} {:>7}\n",
            r.name,
            r.ops,
            r.wall_ms,
            r.ops_per_sec,
            r.latency.p50_ms,
            r.latency.p95_ms,
            r.latency.p99_ms,
            r.peak_rss_kb,
            r.failures
        ));
    }
    s
}
