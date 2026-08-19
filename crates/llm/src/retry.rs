use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use pr_core::PrResult;

/// Global attempt counter used as a cheap jitter source (no rand dep):
/// concurrent agents do not all back off on the same schedule.
static JITTER_SEQ: AtomicU64 = AtomicU64::new(0);

fn jitter(delay: Duration) -> Duration {
    let seq = JITTER_SEQ.fetch_add(1, Ordering::Relaxed);
    // Full-jitter-ish: shift the delay by up to ±25% deterministically.
    let ms = delay.as_millis() as u64;
    let spread = ms / 4;
    let delta = if spread == 0 { 0 } else { (seq % (2 * spread + 1)) as i64 - spread as i64 };
    Duration::from_millis((ms as i64 + delta).max(50) as u64)
}

/// Execute an async operation with exponential backoff + jitter.
///
/// Only *retryable* errors are retried (see [`PrError::is_retryable`]):
/// network/timeout failures and HTTP 408/429/5xx. Permanent failures
/// (400/401/403/404, oversized responses, tool/config errors) return
/// immediately instead of burning attempts and quota.
///
/// When the error carries a `Retry-After` hint (HTTP 429), it is honored.
pub async fn with_retry<F, Fut, T>(f: F, max_retries: u32) -> PrResult<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = PrResult<T>>,
{
    let mut delay = Duration::from_millis(500);
    for attempt in 0..=max_retries {
        match f().await {
            Ok(val) => return Ok(val),
            Err(e) if attempt < max_retries && e.is_retryable() => {
                let wait = e
                    .retry_after_secs()
                    .map(|s| Duration::from_secs(s.min(60)))
                    .unwrap_or(delay);
                let wait = jitter(wait);
                tracing::warn!(
                    "Attempt {}/{} failed: {}, retrying in {:?}",
                    attempt + 1,
                    max_retries + 1,
                    e,
                    wait
                );
                tokio::time::sleep(wait).await;
                delay = (delay * 2).min(Duration::from_secs(60));
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pr_core::PrError;
    use std::sync::atomic::AtomicU32;
    use std::sync::Arc;

    #[tokio::test]
    async fn succeeds_on_first_try() {
        let result = with_retry(|| async { Ok::<_, PrError>(42) }, 3).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn retries_then_succeeds() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result = with_retry(
            move || {
                let attempts = attempts_clone.clone();
                async move {
                    let count = attempts.fetch_add(1, Ordering::SeqCst);
                    if count < 2 {
                        Err(PrError::Llm(format!("failure {}", count)))
                    } else {
                        Ok(99)
                    }
                }
            },
            3,
        )
        .await;

        assert_eq!(result.unwrap(), 99);
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn exhausts_retries_and_fails() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result: PrResult<i32> = with_retry(
            move || {
                let attempts = attempts_clone.clone();
                async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    Err(PrError::Llm("always fails".to_string()))
                }
            },
            2,
        )
        .await;

        assert!(result.is_err());
        // 1 initial attempt + 2 retries = 3 total
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn zero_retries_fails_immediately() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result: PrResult<i32> = with_retry(
            move || {
                let attempts = attempts_clone.clone();
                async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    Err(PrError::Llm("fail".to_string()))
                }
            },
            0,
        )
        .await;

        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn permanent_http_errors_are_not_retried() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result: PrResult<i32> = with_retry(
            move || {
                let attempts = attempts_clone.clone();
                async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    Err(PrError::Http {
                        status: 401,
                        message: "unauthorized".into(),
                        retry_after: None,
                    })
                }
            },
            3,
        )
        .await;

        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 1, "401 must not retry");
    }

    #[tokio::test]
    async fn rate_limited_errors_retry() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result = with_retry(
            move || {
                let attempts = attempts_clone.clone();
                async move {
                    let count = attempts.fetch_add(1, Ordering::SeqCst);
                    if count == 0 {
                        Err(PrError::Http {
                            status: 429,
                            message: "rate limited".into(),
                            retry_after: None,
                        })
                    } else {
                        Ok(7)
                    }
                }
            },
            2,
        )
        .await;

        assert_eq!(result.unwrap(), 7);
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn response_too_large_is_not_retried() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result: PrResult<i32> = with_retry(
            move || {
                let attempts = attempts_clone.clone();
                async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    Err(PrError::ResponseTooLarge("huge".into()))
                }
            },
            3,
        )
        .await;

        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn jitter_stays_within_bounds() {
        let base = Duration::from_millis(1000);
        for _ in 0..100 {
            let j = jitter(base);
            assert!(j.as_millis() >= 750 && j.as_millis() <= 1250);
        }
    }
}
