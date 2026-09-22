//! Smoke tests for the AX benchmark suite: each scenario must complete
//! with zero failures and report sane metrics (ops/sec > 0, latencies
//! non-negative, monotonic percentiles).

use pr_ax::bench::{self, LatencyStats};
use pr_ax::{AxController, AxStore};
use tempfile::TempDir;

fn assert_sane(r: &bench::BenchResult) {
    assert!(r.ops_per_sec > 0.0, "{}: ops/sec must be > 0", r.name);
    assert!(r.wall_ms > 0.0, "{}: wall_ms must be > 0", r.name);
    let l = &r.latency;
    assert!(
        l.min_ms <= l.p50_ms && l.p50_ms <= l.p95_ms && l.p95_ms <= l.p99_ms,
        "{}: percentile ordering broken: {l:?}",
        r.name
    );
    assert_eq!(r.failures, 0, "{}: expected 0 failures", r.name);
}

#[test]
fn latency_stats_ordering() {
    let mut v: Vec<f64> = (1..=100).map(|i| i as f64).collect();
    let s = LatencyStats::from_ms(&mut v);
    assert_eq!(s.count, 100);
    assert!(s.p50_ms < s.p95_ms && s.p95_ms < s.p99_ms);
    assert_eq!(s.min_ms, 1.0);
    assert_eq!(s.max_ms, 100.0);
    let empty = LatencyStats::from_ms(&mut []);
    assert_eq!(empty.count, 0);
    assert_eq!(empty.min_ms, 0.0);
    assert_eq!(empty.max_ms, 0.0);
}

#[test]
fn manifest_parse_bench() {
    let r = bench::manifest_parse(50).unwrap();
    assert_sane(&r);
}

#[test]
fn event_log_append_bench() {
    let tmp = TempDir::new().unwrap();
    let store = AxStore::in_memory(tmp.path()).unwrap();
    let r = bench::event_log_append(&store, 100).unwrap();
    assert_sane(&r);
}

#[test]
fn apply_dispatch_bench() {
    let tmp = TempDir::new().unwrap();
    let ctl = AxController::in_memory(tmp.path()).unwrap();
    let r = bench::apply_dispatch(&ctl, 10).unwrap();
    assert_sane(&r);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_actors_bench() {
    let tmp = TempDir::new().unwrap();
    let ctl = AxController::open(tmp.path().join("c")).unwrap();
    let r = bench::concurrent_actors(&ctl, "default", 5, 30)
        .await
        .unwrap();
    assert_sane(&r);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lifecycle_bench() {
    let tmp = TempDir::new().unwrap();
    let ctl = AxController::open(tmp.path().join("lc")).unwrap();
    let r = bench::lifecycle(&ctl, "default", 3, 30).await.unwrap();
    assert_sane(&r);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn suite_end_to_end() {
    let tmp = TempDir::new().unwrap();
    let results = bench::run_suite(tmp.path(), 5, 30).await.unwrap();
    assert_eq!(results.len(), 5);
    for r in &results {
        assert_sane(r);
    }
    let table = bench::render(&results);
    assert!(table.contains("SCENARIO"));
    assert!(table.contains("concurrent-actors-5"));
    assert!(serde_json::to_string(&results).is_ok());
}
