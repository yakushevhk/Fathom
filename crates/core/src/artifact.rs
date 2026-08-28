use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::{PrError, PrResult, VirtualUri};

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
