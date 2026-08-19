//! Per-model concurrency throttles (ouroboros `model_concurrency.py` +
//! `fallback_cooldown.py` inspiration).
//!
//! Two orthogonal throttles keep a multi-agent swarm from self-inflicting
//! provider rate limits:
//!
//! - **`ModelSemaphore`**: a bounded semaphore per (model) — default 3 — so a
//!   fan-out of sub-agents queues *saturate* instead of slamming the same model
//!   with a fan of concurrent requests that each get 429'd.
//! - **`FallbackCooldown`**: after a 429/5xx the provider is known to be
//!   throttled; we record an expiry and refuse to "re-molten" it until the
//!   cooldown window elapses, so the swarm does not hammer a rate-limited
//!   endpoint round after round from separate agents.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, Semaphore};

/// Default concurrent requests allowed per model lane.
pub const DEFAULT_LANE_CONCURRENCY: usize = 3;
/// Default cooldown after a 429 / 5xx before the model lane is retried.
pub const DEFAULT_COOLDOWN: Duration = Duration::from_secs(30);
/// Cooldown after an explicit 429 (worse than a generic 5xx).
pub const RATE_LIMIT_COOLDOWN: Duration = Duration::from_secs(60);

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// A bounded per-model semaphore keyed by model id.
#[derive(Clone)]
pub struct ModelSemaphore {
    lanes: Arc<Mutex<HashMap<String, Arc<Semaphore>>>>,
    permits: usize,
}

impl ModelSemaphore {
    pub fn new(permits: usize) -> Self {
        Self {
            lanes: Arc::new(Mutex::new(HashMap::new())),
            permits: permits.max(1),
        }
    }

    /// Acquire a permit for `model`, running `f` while holding it, then release.
    pub async fn acquire<'a, T>(
        &'a self,
        model: &str,
        f: impl std::future::Future<Output = T> + Send,
    ) -> T {
        let sem = {
            let mut lanes = self.lanes.lock().await;
            lanes
                .entry(model.to_string())
                .or_insert_with(|| Arc::new(Semaphore::new(self.permits)))
                .clone()
        };
        let _permit = sem.acquire().await.expect("semaphore not closed");
        f.await
    }
}

impl Default for ModelSemaphore {
    fn default() -> Self {
        Self::new(DEFAULT_LANE_CONCURRENCY)
    }
}

/// A 429 / 5xx-aware cooldown per model lane.
#[derive(Clone)]
pub struct FallbackCooldown {
    expires: Arc<Mutex<HashMap<String, u64>>>,
    /// Default cooldown window for a generic limit signal.
    default: Duration,
    /// Window used when the signal was an explicit HTTP 429.
    rate_limit: Duration,
}

impl FallbackCooldown {
    pub fn new(default: Duration, rate_limit: Duration) -> Self {
        Self {
            expires: Arc::new(Mutex::new(HashMap::new())),
            default,
            rate_limit,
        }
    }

    /// Record that `model` was throttled (HTTP 429 or 5xx). `is_rate_limit`
    /// selects the more aggressive window for a real 429.
    pub async fn note_limit(&self, model: &str, is_rate_limit: bool) {
        let until = now_millis()
            + (if is_rate_limit { self.rate_limit } else { self.default })
                .as_millis() as u64;
        self.expires.lock().await.insert(model.to_string(), until);
    }

    /// Whether `model` is currently in cooldown. Prunes stale entries lazily.
    pub async fn is_cooldown(&self, model: &str) -> bool {
        let now = now_millis();
        let mut m = self.expires.lock().await;
        match m.get(model) {
            Some(&until) => {
                if now >= until {
                    m.remove(model);
                    false
                } else {
                    true
                }
            }
            None => false,
        }
    }

    /// If the lane is cooling down, return how long to wait; else `None`.
    pub async fn wait_hint(&self, model: &str) -> Option<Duration> {
        let now = now_millis();
        let m = self.expires.lock().await;
        m.get(model).and_then(|&until| {
            if now < until {
                Some(Duration::from_millis(until - now))
            } else {
                None
            }
        })
    }
}

impl Default for FallbackCooldown {
    fn default() -> Self {
        Self::new(DEFAULT_COOLDOWN, RATE_LIMIT_COOLDOWN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn semaphore_limits_concurrent_calls() {
        let sem = ModelSemaphore::new(2);
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));

        let mut tasks = Vec::new();
        for _ in 0..6 {
            let sem = sem.clone();
            let active = active.clone();
            let max_active = max_active.clone();
            tasks.push(tokio::spawn(async move {
                sem.acquire("deepseek", async {
                    let cur = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_active.fetch_max(cur, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    active.fetch_sub(1, Ordering::SeqCst);
                })
                .await;
            }));
        }
        for t in tasks {
            t.await.unwrap();
        }
        assert!(max_active.load(Ordering::SeqCst) <= 2, "concurrency must be ≤ 2");
    }

    #[tokio::test]
    async fn seperate_models_have_separate_lanes() {
        let sem = ModelSemaphore::new(1);
        // Two different models can run concurrently even at permits=1 because
        // each lane is independent.
        let a = tokio::spawn({
            let sem = sem.clone();
            async move {
                sem.acquire("model-a", async { tokio::time::sleep(Duration::from_millis(10)).await }).await
            }
        });
        let b = tokio::spawn({
            let sem = sem.clone();
            async move {
                sem.acquire("model-b", async { tokio::time::sleep(Duration::from_millis(10)).await }).await
            }
        });
        a.await.unwrap();
        b.await.unwrap();
    }

    #[tokio::test]
    async fn cooldown_engages_and_expires() {
        let cd = FallbackCooldown::new(Duration::from_millis(80), Duration::from_millis(200));
        assert!(!cd.is_cooldown("m").await);
        cd.note_limit("m", false).await;
        assert!(cd.is_cooldown("m").await);
        assert!(cd.wait_hint("m").await.is_some());
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert!(!cd.is_cooldown("m").await);
    }

    #[tokio::test]
    async fn rate_limit_uses_longer_window() {
        let cd = FallbackCooldown::new(Duration::from_millis(50), Duration::from_millis(300));
        cd.note_limit("m", true).await;
        assert!(cd.is_cooldown("m").await);
        // After the default window, still cooling down because it was a 429.
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(cd.is_cooldown("m").await);
    }
}
