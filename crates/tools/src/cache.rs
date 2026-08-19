//! Session-scoped caches shared across tool calls via [`crate::registry::ToolContext`]
//! (fleet report B15/B16).
//!
//! These caches only skip network/DNS work — they never change what a tool
//! returns:
//!
//! - [`FetchCache`] memoizes successful `web_fetch` bodies for 10 minutes so
//!   agents that re-read the same URL do not re-download it.
//! - [`MxCache`] memoizes DNS-over-HTTPS MX lookups per domain so repeated
//!   `verify_email` calls for addresses at the same domain skip the DoH round
//!   trip. The cached value is the full MX record list (an empty list means
//!   "no MX exists"); the complete list is needed so a cache hit can
//!   reproduce byte-identical tool output.
//!
//! Both caches are tiny (bounded entry count + TTL) and cheap to clone
//! (everything lives behind an `Arc`), so a session can share one instance
//! across all its agents.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Maximum number of entries in [`FetchCache`]; the oldest entry (by
/// insertion time) is evicted when the cap is exceeded.
pub const FETCH_CACHE_CAP: usize = 64;

/// Maximum number of entries in [`MxCache`].
pub const MX_CACHE_CAP: usize = 256;

/// Default time-to-live for cached entries: 10 minutes.
pub const CACHE_TTL: Duration = Duration::from_secs(10 * 60);

// ─── FetchCache ───

/// Cached fetch result: `(body, content_type)`.
pub type FetchEntry = Arc<(String, String)>;

/// Session-scoped cache of successful HTTP fetches, keyed by URL.
///
/// Bounded at [`FETCH_CACHE_CAP`] entries (oldest-by-insertion evicted first)
/// with a [`CACHE_TTL`] expiry per entry.
#[derive(Clone, Debug, Default)]
pub struct FetchCache {
    inner: Arc<Mutex<HashMap<String, (FetchEntry, Instant)>>>,
    cap: usize,
    ttl: Duration,
}

impl FetchCache {
    /// Create a cache with the default cap (64) and TTL (10 minutes).
    pub fn new() -> Self {
        Self::with_limits(FETCH_CACHE_CAP, CACHE_TTL)
    }

    /// Create a cache with an explicit cap and TTL (mainly for tests).
    pub fn with_limits(cap: usize, ttl: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            cap: cap.max(1),
            ttl,
        }
    }

    /// Return the cached `(body, content_type)` for `url`, or `None` on a
    /// miss or when the entry has expired (expired entries are removed).
    pub fn get(&self, url: &str) -> Option<FetchEntry> {
        let mut map = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        match map.get(url) {
            Some((entry, inserted)) if inserted.elapsed() < self.ttl => Some(Arc::clone(entry)),
            Some(_) => {
                map.remove(url);
                None
            }
            None => None,
        }
    }

    /// Store a fetch result. Evicts expired entries, then — if still over
    /// the cap — the oldest entry by insertion time.
    pub fn insert(&self, url: &str, body: String, content_type: String) {
        let mut map = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if map.len() >= self.cap {
            // Cheap opportunistic cleanup before falling back to LRU-by-insertion.
            map.retain(|_, (_, inserted)| inserted.elapsed() < self.ttl);
            while map.len() >= self.cap {
                let oldest = map
                    .iter()
                    .min_by_key(|(_, (_, inserted))| *inserted)
                    .map(|(k, _)| k.clone());
                match oldest {
                    Some(key) => {
                        map.remove(&key);
                    }
                    None => break,
                }
            }
        }
        map.insert(url.to_string(), (Arc::new((body, content_type)), Instant::now()));
    }

    /// Number of entries currently stored (including any expired-but-unpurged ones).
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ─── MxCache ───

/// Session-scoped cache of MX lookup results, keyed by (lower-cased) domain.
///
/// The value is the sorted list of MX exchanges returned by the
/// DNS-over-HTTPS lookup; an empty list records "this domain definitively
/// has no MX". Only definitive answers are ever stored by the caller, so a
/// hit always reproduces the original lookup result. Bounded at
/// [`MX_CACHE_CAP`] entries with the same [`CACHE_TTL`] expiry.
#[derive(Clone, Debug, Default)]
pub struct MxCache {
    inner: Arc<Mutex<HashMap<String, (Arc<Vec<String>>, Instant)>>>,
    cap: usize,
    ttl: Duration,
}

impl MxCache {
    /// Create a cache with the default cap (256) and TTL (10 minutes).
    pub fn new() -> Self {
        Self::with_limits(MX_CACHE_CAP, CACHE_TTL)
    }

    /// Create a cache with an explicit cap and TTL (mainly for tests).
    pub fn with_limits(cap: usize, ttl: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            cap: cap.max(1),
            ttl,
        }
    }

    /// Return the cached MX record list for `domain` (`Some` with an empty
    /// vec means "definitively no MX"), or `None` on a miss/expired entry.
    pub fn get(&self, domain: &str) -> Option<Arc<Vec<String>>> {
        let mut map = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        match map.get(domain) {
            Some((records, inserted)) if inserted.elapsed() < self.ttl => {
                Some(Arc::clone(records))
            }
            Some(_) => {
                map.remove(domain);
                None
            }
            None => None,
        }
    }

    /// Whether the domain is present in the cache (regardless of MX presence).
    pub fn contains(&self, domain: &str) -> bool {
        self.get(domain).is_some()
    }

    /// Store an MX result for `domain` (definitive answers only). Evicts the
    /// oldest entry by insertion time once the cap is reached.
    pub fn insert(&self, domain: &str, records: Vec<String>) {
        let mut map = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if map.len() >= self.cap {
            map.retain(|_, (_, inserted)| inserted.elapsed() < self.ttl);
            while map.len() >= self.cap {
                let oldest = map
                    .iter()
                    .min_by_key(|(_, (_, inserted))| *inserted)
                    .map(|(k, _)| k.clone());
                match oldest {
                    Some(key) => {
                        map.remove(&key);
                    }
                    None => break,
                }
            }
        }
        map.insert(domain.to_string(), (Arc::new(records), Instant::now()));
    }

    /// Number of entries currently stored.
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ─── Tests ───

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_cache_hit_returns_body_and_content_type() {
        let cache = FetchCache::new();
        assert!(cache.is_empty());
        assert!(cache.get("https://example.com").is_none());

        cache.insert(
            "https://example.com",
            "<html>hello</html>".to_string(),
            "text/html; charset=utf-8".to_string(),
        );
        let hit = cache.get("https://example.com").expect("cache hit");
        assert_eq!(hit.0, "<html>hello</html>");
        assert_eq!(hit.1, "text/html; charset=utf-8");
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_fetch_cache_ttl_expiry() {
        let cache = FetchCache::with_limits(FETCH_CACHE_CAP, Duration::from_millis(20));
        cache.insert("https://example.com", "body".to_string(), "text/plain".to_string());
        assert!(cache.get("https://example.com").is_some());

        std::thread::sleep(Duration::from_millis(40));
        assert!(
            cache.get("https://example.com").is_none(),
            "entry should expire after the TTL"
        );
        assert!(cache.is_empty(), "expired entry should be removed");
    }

    #[test]
    fn test_fetch_cache_evicts_oldest_by_insertion_order() {
        let cache = FetchCache::with_limits(3, CACHE_TTL);
        cache.insert("https://a.test", "a".to_string(), "text/plain".to_string());
        std::thread::sleep(Duration::from_millis(5));
        cache.insert("https://b.test", "b".to_string(), "text/plain".to_string());
        std::thread::sleep(Duration::from_millis(5));
        cache.insert("https://c.test", "c".to_string(), "text/plain".to_string());
        assert_eq!(cache.len(), 3);

        // Inserting a 4th entry evicts the oldest ("a").
        cache.insert("https://d.test", "d".to_string(), "text/plain".to_string());
        assert_eq!(cache.len(), 3);
        assert!(cache.get("https://a.test").is_none(), "oldest entry evicted");
        assert!(cache.get("https://b.test").is_some());
        assert!(cache.get("https://c.test").is_some());
        assert!(cache.get("https://d.test").is_some());
    }

    #[test]
    fn test_mx_cache_hit() {
        let cache = MxCache::new();
        assert!(cache.get("example.com").is_none());

        cache.insert(
            "example.com",
            vec!["mail.example.com".to_string(), "backup.example.com".to_string()],
        );
        let hit = cache.get("example.com").expect("cache hit");
        assert_eq!(&**hit, &["mail.example.com", "backup.example.com"]);
        assert!(cache.contains("example.com"));

        // A definitive "no MX" answer is cached as an empty list.
        cache.insert("nomx.example", Vec::new());
        let empty = cache.get("nomx.example").expect("negative results are cached too");
        assert!(empty.is_empty());
    }

    #[test]
    fn test_mx_cache_ttl_and_capacity() {
        let cache = MxCache::with_limits(2, Duration::from_millis(20));
        cache.insert("one.test", vec!["mx.one.test".to_string()]);
        std::thread::sleep(Duration::from_millis(5));
        cache.insert("two.test", vec!["mx.two.test".to_string()]);
        assert_eq!(cache.len(), 2);

        // Over capacity: the oldest entry is evicted.
        cache.insert("three.test", vec!["mx.three.test".to_string()]);
        assert_eq!(cache.len(), 2);
        assert!(cache.get("one.test").is_none());
        assert!(cache.get("two.test").is_some());
        assert!(cache.get("three.test").is_some());

        // TTL expiry applies to MX entries too.
        std::thread::sleep(Duration::from_millis(40));
        assert!(cache.get("two.test").is_none());
        assert!(cache.get("three.test").is_none());
    }
}
