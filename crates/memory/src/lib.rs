//! Long-term semantic memory for the research agent.
//!
//! Mem0/Memora-inspired design adapted to the SQLite-first stack:
//!
//! - **append-only** memories with typed edges (`supersedes`,
//!   `contradicts`, `related_to`) instead of in-place rewrites;
//! - **hybrid retrieval**: cosine similarity + BM25 keyword fusion with
//!   temporal decay (docs/mem0/ARCHITECTURE.md, docs/memora/SEARCH-PATTERNS.md);
//! - **absorb pipeline**: validation → secret scan → consolidation →
//!   dedup → LLM/heuristic classification → edges + history
//!   (docs/memora/ABSORB-GUIDE.md);
//! - **scopes** `user` / `agent` / `run` (mem0 namespacing);
//! - **digest**: deterministic pre-session context load
//!   (docs/memora/DIGEST-GUIDE.md).

pub mod absorb;
pub mod db;
pub mod distill;
pub mod embed;
pub mod gc;
pub mod graph;
pub mod search;
pub mod secrets;

pub use absorb::*;
pub use db::*;
pub use distill::*;
pub use embed::*;
pub use gc::*;
pub use graph::*;
pub use search::*;
pub use secrets::detect_secrets;

use pr_core::config::{LlmConfig, MemoryConfig};
use pr_llm::LlmProvider;
use std::path::PathBuf;
use std::sync::Arc;

/// The memory subsystem: store + embedder + config, shareable across the
/// coordinator, runtimes and tools.
pub struct Memory {
    pub db: Arc<MemoryDb>,
    pub embedder: Arc<dyn Embedder>,
    pub config: MemoryConfig,
}

impl Memory {
    /// Open (or create) the memory database according to config.
    pub fn open(config: &MemoryConfig, llm_config: &LlmConfig) -> anyhow::Result<Self> {
        let path = if config.db_path.is_empty() {
            default_memory_db_path()
        } else {
            PathBuf::from(&config.db_path)
        };
        let db = Arc::new(MemoryDb::open(&path)?);
        let embedder = build_embedder(config, llm_config);
        db.meta_set("embedding_model", embedder.model_name());
        Ok(Self {
            db,
            embedder,
            config: config.clone(),
        })
    }

    /// In-memory store for tests.
    pub fn in_memory(config: MemoryConfig) -> anyhow::Result<Self> {
        let db = Arc::new(MemoryDb::in_memory()?);
        let embedder = Arc::new(TfidfEmbedder::new());
        Ok(Self {
            db,
            embedder,
            config,
        })
    }

    /// An absorb pipeline without LLM classification (fast path used by
    /// deterministic autosave hooks).
    pub fn pipeline(&self) -> AbsorbPipeline {
        AbsorbPipeline::new(self.db.clone(), self.embedder.clone())
    }

    /// An absorb pipeline with LLM classification attached.
    pub fn pipeline_with_llm(&self, llm: Arc<dyn LlmProvider>) -> AbsorbPipeline {
        AbsorbPipeline::new(self.db.clone(), self.embedder.clone())
            .with_llm(llm, self.config.llm_classify)
    }

    /// Default search scope for a session: persistent agent+user memories
    /// plus the session's own run-scoped facts.
    pub fn session_scope(&self, session_id: &str) -> ScopeFilter {
        ScopeFilter::persistent().add(Scope::Run, session_id)
    }

    /// Hybrid search with configured defaults.
    pub async fn search(
        &self,
        query: &str,
        scope: &ScopeFilter,
        top_k: Option<usize>,
    ) -> anyhow::Result<Vec<SearchHit>> {
        hybrid_search(
            &self.db,
            &self.embedder,
            &SearchParams {
                query: query.to_string(),
                top_k: top_k.unwrap_or(self.config.top_k as usize),
                min_score: self.config.min_score,
                semantic_weight: self.config.semantic_weight,
                temporal_decay: self.config.temporal_decay,
                scope: scope.clone(),
            },
        )
        .await
    }

    /// Pre-session digest for prompt injection.
    pub async fn digest(&self, topic: &str, scope: &ScopeFilter) -> anyhow::Result<Digest> {
        build_digest(
            &self.db,
            &self.embedder,
            topic,
            scope,
            self.config.top_k as usize,
            self.config.min_score,
            self.config.semantic_weight,
            self.config.temporal_decay,
        )
        .await
    }

    /// Render a bounded digest block for the system prompt. Errors and
    /// empty digests both yield an empty string — memory must never block
    /// an agent from starting (hermes consolidation-cap philosophy).
    pub async fn digest_block(&self, topic: &str, scope: &ScopeFilter, max_chars: usize) -> String {
        match self.digest(topic, scope).await {
            Ok(d) => d.to_prompt_block(max_chars),
            Err(e) => {
                tracing::warn!("memory digest failed: {e}");
                String::new()
            }
        }
    }

    /// Re-embed every memory with the current model (after switching
    /// embedding backends/models). Returns the number of rows re-embedded.
    pub async fn rebuild_embeddings(&self) -> anyhow::Result<usize> {
        let pending = self.db.ids_without_embedding(self.embedder.model_name())?;
        if pending.is_empty() {
            return Ok(0);
        }
        let mut done = 0usize;
        for chunk in pending.chunks(32) {
            let texts: Vec<String> = chunk.iter().map(|(_, c)| c.clone()).collect();
            let vecs = self.embedder.embed(&texts).await?;
            for ((id, _), v) in chunk.iter().zip(vecs.iter()) {
                self.db.put_embedding(id, self.embedder.model_name(), v)?;
                done += 1;
            }
        }
        Ok(done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> MemoryConfig {
        MemoryConfig::default()
    }

    #[tokio::test]
    async fn open_in_memory_and_roundtrip() {
        let mem = Memory::in_memory(config()).unwrap();
        let report = mem
            .pipeline()
            .absorb(AbsorbRequest {
                facts: vec![AbsorbFact {
                    content: "the office is at 12 Tverskaya street in Moscow".into(),
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    confidence: None,
                    memory_class: None,
                }],
                source: "unit".into(),
                scope: Scope::Agent,
                scope_key: String::new(),
                context: None,
                dry_run: false,
            })
            .await
            .unwrap();
        assert_eq!(report.created, 1);

        let hits = mem.search("tverskaya office address", &ScopeFilter::persistent(), None).await.unwrap();
        assert!(!hits.is_empty());
    }

    #[tokio::test]
    async fn digest_block_bounded_and_fault_tolerant() {
        let mem = Memory::in_memory(config()).unwrap();
        // Empty store => empty block.
        assert!(mem.digest_block("anything", &ScopeFilter::persistent(), 2000).await.is_empty());

        mem.pipeline()
            .absorb(AbsorbRequest {
                facts: vec![AbsorbFact {
                    content: "verified email ceo@acme.ru belongs to Maria Ivanova".into(),
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    confidence: Some(0.95),
                    memory_class: None,
                }],
                source: "unit".into(),
                scope: Scope::Agent,
                scope_key: String::new(),
                context: None,
                dry_run: false,
            })
            .await
            .unwrap();
        let block = mem.digest_block("acme ceo email", &ScopeFilter::persistent(), 2000).await;
        assert!(block.contains("Long-term memory digest"));
        assert!(block.contains("Maria Ivanova"));
    }

    #[tokio::test]
    async fn session_scope_includes_run_memories() {
        let mem = Memory::in_memory(config()).unwrap();
        let mut req = AbsorbRequest {
            facts: vec![AbsorbFact {
                content: "found the team page listing twelve employees".into(),
                metadata: serde_json::json!({}),
                tags: vec![],
                confidence: None,
                memory_class: None,
            }],
            source: "sess-x".into(),
            scope: Scope::Run,
            scope_key: "sess-x".into(),
            context: None,
            dry_run: false,
        };
        mem.pipeline().absorb(req.clone()).await.unwrap();

        // Persistent-only scope does not see run facts...
        let hits = mem.search("team page employees", &ScopeFilter::persistent(), None).await.unwrap();
        assert!(hits.is_empty());
        // ...but the session scope does.
        let hits = mem
            .search("team page employees", &mem.session_scope("sess-x"), None)
            .await
            .unwrap();
        assert!(!hits.is_empty());
        req.dry_run = true; // keep req used
        let _ = req;
    }

    #[tokio::test]
    async fn rebuild_embeddings_covers_unembedded_rows() {
        let mem = Memory::in_memory(config()).unwrap();
        // One memory goes through the pipeline (embedded immediately)...
        mem.pipeline()
            .absorb(AbsorbRequest {
                facts: vec![AbsorbFact {
                    content: "rebuild candidate fact for embedding migration".into(),
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    confidence: None,
                    memory_class: None,
                }],
                source: "unit".into(),
                scope: Scope::Agent,
                scope_key: String::new(),
                context: None,
                dry_run: false,
            })
            .await
            .unwrap();
        assert_eq!(mem.rebuild_embeddings().await.unwrap(), 0);

        // ...another is inserted raw without an embedding (simulates rows
        // written before an embedding model switch). Rebuild picks it up.
        let now = chrono::Utc::now().to_rfc3339();
        mem.db
            .insert(&MemoryRow {
                id: uuid::Uuid::now_v7().to_string(),
                content: "legacy row embedded by an older model run".into(),
                metadata: serde_json::json!({}),
                tags: vec![],
                source: "legacy".into(),
                scope: "agent".into(),
                scope_key: String::new(),
                confidence: 0.8,
                importance: 1.0,
                access_count: 0,
                last_accessed: None,
                status: "active".into(),
                expires_at: None,
                content_hash: content_hash("legacy row embedded by an older model run"),
                created_at: now.clone(),
                updated_at: now,
            })
            .unwrap();
        assert_eq!(mem.rebuild_embeddings().await.unwrap(), 1);
        assert_eq!(
            mem.db
                .load_embeddings(&ScopeFilter::persistent(), mem.embedder.model_name())
                .unwrap()
                .len(),
            2
        );
    }
}
