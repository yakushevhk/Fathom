use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Manages per-file locks to prevent concurrent modification of the same file.
///
/// Each file path gets its own `tokio::sync::Mutex`. Locks are created lazily
/// and shared via `Arc`. The internal map is protected by its own mutex to
/// allow safe concurrent registration of new file paths.
pub struct FileLockManager {
    locks: Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
}

impl FileLockManager {
    /// Create a new, empty lock manager.
    pub fn new() -> Self {
        Self {
            locks: Mutex::new(HashMap::new()),
        }
    }

    /// Get or create a lock for the given canonical path.
    async fn get_lock(&self, path: &Path) -> Arc<Mutex<()>> {
        let canonical = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf());

        let mut map = self.locks.lock().await;
        map.entry(canonical)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Execute a future while holding the per-file lock for `path`.
    ///
    /// If another task is already holding the lock for this path, the caller
    /// will wait until it is released. Different paths can proceed concurrently.
    ///
    /// # Example
    /// ```ignore
    /// let result = file_locks.with_lock(path, async {
    ///     // ... read/modify/write the file ...
    ///     Ok(42)
    /// }).await?;
    /// ```
    pub async fn with_lock<F, Fut, R>(&self, path: &Path, f: F) -> anyhow::Result<R>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<R>>,
    {
        let lock = self.get_lock(path).await;
        let _guard = lock.lock().await;
        f().await
    }

    /// Number of file locks currently registered.
    pub async fn lock_count(&self) -> usize {
        self.locks.lock().await.len()
    }
}

impl Default for FileLockManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_basic_lock() {
        let manager = FileLockManager::new();
        let tmp = tempfile::TempDir::new().unwrap();
        let file = tmp.path().join("test.txt");
        std::fs::write(&file, "data").unwrap();

        let result = manager
            .with_lock(&file, || async { Ok(42) })
            .await
            .unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_lock_serializes_same_file() {
        let manager = Arc::new(FileLockManager::new());
        let tmp = tempfile::TempDir::new().unwrap();
        let file = tmp.path().join("concurrent.txt");
        std::fs::write(&file, "0").unwrap();

        let order = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();

        for i in 0..5 {
            let mgr = manager.clone();
            let path = file.clone();
            let ord = order.clone();
            handles.push(tokio::spawn(async move {
                mgr.with_lock(&path, || async {
                    // Simulate some work while holding the lock.
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    ord.lock().await.push(i);
                    Ok(())
                })
                .await
                .unwrap();
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        // All operations must have completed.
        let order = order.lock().await;
        assert_eq!(order.len(), 5);
        // Since they all serialize on the same lock, they run in acquisition order.
        assert_eq!(*order, vec![0, 1, 2, 3, 4]);
    }

    #[tokio::test]
    async fn test_different_files_proceed_concurrently() {
        let manager = Arc::new(FileLockManager::new());
        let tmp = tempfile::TempDir::new().unwrap();
        let f1 = tmp.path().join("a.txt");
        let f2 = tmp.path().join("b.txt");
        std::fs::write(&f1, "a").unwrap();
        std::fs::write(&f2, "b").unwrap();

        let results = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();

        for (i, path) in [f1.clone(), f2.clone()].iter().enumerate() {
            let mgr = manager.clone();
            let p = path.clone();
            let res = results.clone();
            handles.push(tokio::spawn(async move {
                let start = std::time::Instant::now();
                mgr.with_lock(&p, || async {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    res.lock().await.push(i);
                    Ok(())
                })
                .await
                .unwrap();
                start.elapsed()
            }));
        }

        let mut durations = Vec::new();
        for h in handles {
            durations.push(h.await.unwrap());
        }

        // Both should have run concurrently, so neither should have waited
        // for the other's full 50ms sleep.
        let total = durations.iter().map(|d| d.as_millis()).sum::<u128>();
        // If serialized, total would be ~100ms. Concurrent should be ~50ms.
        // Allow generous margin for CI.
        assert!(
            total < 150,
            "Expected concurrent execution but total was {total}ms"
        );
    }

    #[tokio::test]
    async fn test_lock_count() {
        let manager = FileLockManager::new();
        let tmp = tempfile::TempDir::new().unwrap();
        let f1 = tmp.path().join("x.txt");
        let f2 = tmp.path().join("y.txt");
        std::fs::write(&f1, "x").unwrap();
        std::fs::write(&f2, "y").unwrap();

        assert_eq!(manager.lock_count().await, 0);

        manager.with_lock(&f1, || async { Ok(()) }).await.unwrap();
        assert_eq!(manager.lock_count().await, 1);

        manager.with_lock(&f2, || async { Ok(()) }).await.unwrap();
        assert_eq!(manager.lock_count().await, 2);

        // Same file reuses existing lock.
        manager.with_lock(&f1, || async { Ok(()) }).await.unwrap();
        assert_eq!(manager.lock_count().await, 2);
    }
}
