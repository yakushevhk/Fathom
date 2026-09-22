use crate::VirtualUri;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// High-performance thread-safe in-memory & on-disk Virtual Artifact Store.
/// Backs `artifact://<id>` zero-copy output dereferencing and large tool truncation spillovers.
#[derive(Debug, Clone, Default)]
pub struct ArtifactStore {
    artifacts: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl ArtifactStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Store binary/text artifact under key and return virtual URI handle (`artifact://<key>`).
    pub async fn put(&self, key: &str, content: &[u8]) -> String {
        let mut guard = self.artifacts.write().await;
        guard.insert(key.to_string(), content.to_vec());
        format!("artifact://{}", key)
    }

    /// Retrieve full artifact content by URI.
    pub async fn get(&self, uri: &VirtualUri) -> Option<Vec<u8>> {
        match uri {
            VirtualUri::Artifact { id } => {
                let guard = self.artifacts.read().await;
                guard.get(id).cloned()
            }
            _ => None,
        }
    }

    /// Retrieve range-sliced artifact content (for large output paging).
    pub async fn slice(&self, uri: &VirtualUri, offset: usize, limit: usize) -> Option<Vec<u8>> {
        let full = self.get(uri).await?;
        if offset >= full.len() {
            return Some(Vec::new());
        }
        let end = (offset + limit).min(full.len());
        Some(full[offset..end].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn put_returns_artifact_uri() {
        let store = ArtifactStore::new();
        let uri = store.put("k1", b"hello").await;
        assert_eq!(uri, "artifact://k1");
    }

    #[tokio::test]
    async fn get_roundtrip() {
        let store = ArtifactStore::new();
        let uri = store.put("k2", b"data-42").await;
        let vuri = VirtualUri::Artifact { id: "k2".into() };
        assert_eq!(store.get(&vuri).await.unwrap(), b"data-42");
        assert_eq!(uri, "artifact://k2");
    }

    #[tokio::test]
    async fn get_missing_returns_none() {
        let store = ArtifactStore::new();
        let vuri = VirtualUri::Artifact { id: "nope".into() };
        assert!(store.get(&vuri).await.is_none());
    }

    #[tokio::test]
    async fn get_non_artifact_uri_returns_none() {
        let store = ArtifactStore::new();
        let uri = VirtualUri::Memory {
            section: "root".into(),
        };
        assert!(store.get(&uri).await.is_none());
    }

    #[tokio::test]
    async fn slice_within_bounds() {
        let store = ArtifactStore::new();
        store.put("s", b"abcdefghij").await;
        let vuri = VirtualUri::Artifact { id: "s".into() };
        assert_eq!(store.slice(&vuri, 2, 4).await.unwrap(), b"cdef");
    }

    #[tokio::test]
    async fn slice_clamps_to_end() {
        let store = ArtifactStore::new();
        store.put("s", b"abc").await;
        let vuri = VirtualUri::Artifact { id: "s".into() };
        assert_eq!(store.slice(&vuri, 1, 100).await.unwrap(), b"bc");
    }

    #[tokio::test]
    async fn slice_past_end_is_empty_not_none() {
        let store = ArtifactStore::new();
        store.put("s", b"abc").await;
        let vuri = VirtualUri::Artifact { id: "s".into() };
        assert_eq!(store.slice(&vuri, 10, 5).await.unwrap(), Vec::<u8>::new());
    }

    #[tokio::test]
    async fn overwrite_replaces_content() {
        let store = ArtifactStore::new();
        store.put("k", b"v1").await;
        store.put("k", b"v2").await;
        let vuri = VirtualUri::Artifact { id: "k".into() };
        assert_eq!(store.get(&vuri).await.unwrap(), b"v2");
    }

    #[tokio::test]
    async fn empty_content_roundtrip() {
        let store = ArtifactStore::new();
        store.put("e", b"").await;
        let vuri = VirtualUri::Artifact { id: "e".into() };
        assert_eq!(store.get(&vuri).await.unwrap(), Vec::<u8>::new());
    }
}
