//! Lightweight Prometheus-style metrics for the research agent server.
//!
//! These types are intentionally dependency-free: counters and gauges are
//! atomic integers and histograms keep fixed bucket counts, so updates are
//! lock-free and cheap enough to run on every request.

use std::fmt::Write as _;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

/// Default histogram buckets for request duration, in seconds.
pub const DEFAULT_DURATION_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// A monotonically increasing counter.
#[derive(Debug, Default)]
pub struct Counter {
    value: AtomicU64,
}

impl Counter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_by(&self, n: u64) {
        self.value.fetch_add(n, Ordering::Relaxed);
    }

    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
}

/// A value that can go up and down.
#[derive(Debug, Default)]
pub struct Gauge {
    value: AtomicI64,
}

impl Gauge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec(&self) {
        self.value.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn set(&self, v: i64) {
        self.value.store(v, Ordering::Relaxed);
    }

    pub fn get(&self) -> i64 {
        self.value.load(Ordering::Relaxed)
    }
}

/// A fixed-bucket histogram.
///
/// Bucket counts are stored cumulatively, as required by the Prometheus
/// histogram format: bucket `le="X"` counts all observations `<= X`.
#[derive(Debug)]
pub struct Histogram {
    buckets: Vec<f64>,
    /// Cumulative observations `<=` the matching bucket bound.
    counts: Vec<AtomicU64>,
    count: AtomicU64,
    /// Sum of all observations, stored as `f64::to_bits`.
    sum_bits: AtomicU64,
}

impl Histogram {
    pub fn new(mut buckets: Vec<f64>) -> Self {
        buckets.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let counts = buckets.iter().map(|_| AtomicU64::new(0)).collect();
        Self {
            buckets,
            counts,
            count: AtomicU64::new(0),
            sum_bits: AtomicU64::new(0),
        }
    }

    pub fn observe(&self, value: f64) {
        for (i, bound) in self.buckets.iter().enumerate() {
            if value <= *bound {
                self.counts[i].fetch_add(1, Ordering::Relaxed);
            }
        }
        self.count.fetch_add(1, Ordering::Relaxed);
        let _ = self.sum_bits.fetch_update(Ordering::AcqRel, Ordering::Acquire, |bits| {
            Some((f64::from_bits(bits) + value).to_bits())
        });
    }

    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    pub fn sum(&self) -> f64 {
        f64::from_bits(self.sum_bits.load(Ordering::Acquire))
    }

    /// Render this histogram in Prometheus text exposition format.
    pub fn render(&self, name: &str, help: &str) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "# HELP {name} {help}");
        let _ = writeln!(out, "# TYPE {name} histogram");
        for (i, bound) in self.buckets.iter().enumerate() {
            let n = self.counts[i].load(Ordering::Relaxed);
            let _ = writeln!(out, "{name}_bucket{{le=\"{bound}\"}} {n}");
        }
        let total = self.count();
        let _ = writeln!(out, "{name}_bucket{{le=\"+Inf\"}} {total}");
        let _ = writeln!(out, "{name}_sum {}", self.sum());
        let _ = writeln!(out, "{name}_count {total}");
        out
    }
}

fn render_counter(out: &mut String, name: &str, help: &str, counter: &Counter) {
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} counter");
    let _ = writeln!(out, "{name} {}", counter.get());
}

fn render_gauge(out: &mut String, name: &str, help: &str, gauge: &Gauge) {
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} gauge");
    let _ = writeln!(out, "{name} {}", gauge.get());
}

/// Server-wide metrics registry.
#[derive(Debug)]
pub struct Metrics {
    pub sessions_total: Counter,
    pub sessions_active: Gauge,
    pub agents_spawned: Counter,
    pub tokens_used: Counter,
    pub tool_calls: Counter,
    pub requests_total: Counter,
    pub request_duration: Histogram,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            sessions_total: Counter::new(),
            sessions_active: Gauge::new(),
            agents_spawned: Counter::new(),
            tokens_used: Counter::new(),
            tool_calls: Counter::new(),
            requests_total: Counter::new(),
            request_duration: Histogram::new(DEFAULT_DURATION_BUCKETS.to_vec()),
        }
    }

    /// Render all metrics in Prometheus text exposition format.
    pub fn render_metrics(&self) -> String {
        let mut out = String::new();
        render_counter(
            &mut out,
            "pr_sessions_total",
            "Total number of research sessions created",
            &self.sessions_total,
        );
        render_gauge(
            &mut out,
            "pr_sessions_active",
            "Number of research sessions currently running",
            &self.sessions_active,
        );
        render_counter(
            &mut out,
            "pr_agents_spawned_total",
            "Total number of research agents spawned",
            &self.agents_spawned,
        );
        render_counter(
            &mut out,
            "pr_tokens_used_total",
            "Total number of LLM tokens used by completed agents",
            &self.tokens_used,
        );
        render_counter(
            &mut out,
            "pr_tool_calls_total",
            "Total number of tool calls completed",
            &self.tool_calls,
        );
        render_counter(
            &mut out,
            "pr_http_requests_total",
            "Total number of HTTP requests served",
            &self.requests_total,
        );
        out.push_str(&self.request_duration.render(
            "pr_request_duration_seconds",
            "HTTP request duration in seconds",
        ));
        out
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_increments() {
        let c = Counter::new();
        assert_eq!(c.get(), 0);
        c.inc();
        c.inc_by(41);
        assert_eq!(c.get(), 42);
    }

    #[test]
    fn gauge_goes_up_and_down() {
        let g = Gauge::new();
        g.inc();
        g.inc();
        g.dec();
        assert_eq!(g.get(), 1);
        g.set(-5);
        assert_eq!(g.get(), -5);
    }

    #[test]
    fn histogram_tracks_buckets_sum_and_count() {
        let h = Histogram::new(vec![0.1, 1.0, 10.0]);
        h.observe(0.05); // <= 0.1, 1.0, 10.0
        h.observe(0.5); // <= 1.0, 10.0
        h.observe(20.0); // none

        assert_eq!(h.count(), 3);
        assert!((h.sum() - 20.55).abs() < 1e-9);

        let rendered = h.render("test_duration_seconds", "help text");
        assert!(rendered.contains("# TYPE test_duration_seconds histogram"));
        assert!(rendered.contains("test_duration_seconds_bucket{le=\"0.1\"} 1"));
        assert!(rendered.contains("test_duration_seconds_bucket{le=\"1\"} 2"));
        assert!(rendered.contains("test_duration_seconds_bucket{le=\"10\"} 2"));
        assert!(rendered.contains("test_duration_seconds_bucket{le=\"+Inf\"} 3"));
        assert!(rendered.contains("test_duration_seconds_count 3"));
    }

    #[test]
    fn histogram_buckets_are_sorted() {
        let h = Histogram::new(vec![10.0, 0.1, 1.0]);
        h.observe(0.5);
        let rendered = h.render("x", "y");
        let pos_01 = rendered.find("le=\"0.1\"").unwrap();
        let pos_1 = rendered.find("le=\"1\"").unwrap();
        let pos_10 = rendered.find("le=\"10\"").unwrap();
        assert!(pos_01 < pos_1 && pos_1 < pos_10);
    }

    #[test]
    fn render_metrics_contains_all_series() {
        let m = Metrics::new();
        m.sessions_total.inc();
        m.sessions_active.inc();
        m.agents_spawned.inc_by(3);
        m.tokens_used.inc_by(100);
        m.tool_calls.inc_by(7);
        m.requests_total.inc();
        m.request_duration.observe(0.01);

        let out = m.render_metrics();
        assert!(out.contains("pr_sessions_total 1"));
        assert!(out.contains("pr_sessions_active 1"));
        assert!(out.contains("pr_agents_spawned_total 3"));
        assert!(out.contains("pr_tokens_used_total 100"));
        assert!(out.contains("pr_tool_calls_total 7"));
        assert!(out.contains("pr_http_requests_total 1"));
        assert!(out.contains("pr_request_duration_seconds_bucket{le=\"+Inf\"} 1"));
    }
}
