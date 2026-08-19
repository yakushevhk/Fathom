//! Embedding backends for semantic memory search.
//!
//! Two backends, mirroring Memora's layering:
//!
//! - [`TfidfEmbedder`] — dependency-free offline fallback. Uses the
//!   signed hashing trick over tokens: deterministic, lexical-only, but
//!   good enough for exact-ish recall without any external service.
//! - [`OpenAiEmbedder`] — any OpenAI-compatible `/v1/embeddings` endpoint
//!   (OpenAI, OpenRouter, Ollama, vLLM, LM Studio, ...).
//!
//! Vectors from different models are never compared: the store tags every
//! embedding with the model name and search only loads same-model rows.

use async_trait::async_trait;
use std::sync::Arc;

/// Vector similarity in [0, 1] for L2-normalized inputs.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>().max(0.0)
}

#[async_trait]
pub trait Embedder: Send + Sync {
    /// Model name stored next to every vector (mixing models is forbidden).
    fn model_name(&self) -> &str;
    /// Embed a batch of texts. Implementations must return one vector per
    /// input, in order.
    async fn embed(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>>;
}

// ── TF-IDF hashing fallback ──────────────────────────────────────────────────

/// Offline lexical embedder: tokens are hashed into a fixed number of
/// buckets with signed contributions, then L2-normalized. No network, no
/// model files — usable in tests and air-gapped environments.
pub struct TfidfEmbedder {
    dim: usize,
}

pub const TFIDF_MODEL_NAME: &str = "tfidf-hash-512";

impl TfidfEmbedder {
    pub fn new() -> Self {
        Self { dim: 512 }
    }
}

impl Default for TfidfEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

/// Lowercase alphanumeric/Cyrillic tokens of length >= 2.
fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.to_lowercase().chars() {
        if ch.is_alphanumeric() {
            cur.push(ch);
        } else if cur.len() >= 2 {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.clear();
        }
    }
    if cur.len() >= 2 {
        out.push(cur);
    }
    out
}

fn embed_one_tfidf(text: &str, dim: usize) -> Vec<f32> {
    use std::hash::{Hash, Hasher};
    let mut v = vec![0f32; dim];
    for tok in tokenize(text) {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        tok.hash(&mut h);
        let hash = h.finish();
        let idx = (hash % dim as u64) as usize;
        // Signed hashing reduces systematic collision bias.
        let sign = if (hash >> 63) & 1 == 0 { 1.0 } else { -1.0 };
        v[idx] += sign;
    }
    l2_normalize(&mut v);
    v
}

fn l2_normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-12 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

#[async_trait]
impl Embedder for TfidfEmbedder {
    fn model_name(&self) -> &str {
        TFIDF_MODEL_NAME
    }

    async fn embed(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|t| embed_one_tfidf(t, self.dim))
            .collect())
    }
}

// ── OpenAI-compatible embeddings ─────────────────────────────────────────────

/// Embedder for any OpenAI-compatible `/embeddings` endpoint.
pub struct OpenAiEmbedder {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
    /// Max texts per request (providers cap batch sizes).
    batch: usize,
}

impl OpenAiEmbedder {
    pub fn new(base_url: &str, api_key: &str, model: &str) -> Self {
        Self {
            client: pr_core::http_client(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            batch: 64,
        }
    }
}

#[async_trait]
impl Embedder for OpenAiEmbedder {
    fn model_name(&self) -> &str {
        &self.model
    }

    async fn embed(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!("{}/embeddings", self.base_url);
        let mut out = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(self.batch) {
            let body = serde_json::json!({
                "model": self.model,
                "input": chunk,
            });
            let resp = self
                .client
                .post(&url)
                .bearer_auth(&self.api_key)
                .json(&body)
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("embeddings request failed: {e}"))?;
            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                anyhow::bail!(
                    "embeddings endpoint returned {status}: {}",
                    &text[..text.len().min(300)]
                );
            }
            let parsed: serde_json::Value = resp.json().await?;
            let data = parsed
                .get("data")
                .and_then(|d| d.as_array())
                .ok_or_else(|| anyhow::anyhow!("embeddings response missing data array"))?;
            if data.len() != chunk.len() {
                anyhow::bail!(
                    "embeddings endpoint returned {} vectors for {} inputs",
                    data.len(),
                    chunk.len()
                );
            }
            for item in data {
                let vec = item
                    .get("embedding")
                    .and_then(|e| e.as_array())
                    .ok_or_else(|| anyhow::anyhow!("embedding item missing vector"))?
                    .iter()
                    .map(|f| f.as_f64().unwrap_or(0.0) as f32)
                    .collect::<Vec<f32>>();
                out.push(vec);
            }
        }
        Ok(out)
    }
}

/// Pick an embedder from config: `tfidf` forces the offline backend,
/// `openai` forces the HTTP backend (fails fast on missing credentials),
/// `auto` uses HTTP when credentials exist and falls back to TF-IDF.
pub fn build_embedder(
    memory_cfg: &pr_core::MemoryConfig,
    llm_cfg: &pr_core::LlmConfig,
) -> Arc<dyn Embedder> {
    let base_url = if memory_cfg.embedding_base_url.is_empty() {
        llm_cfg.base_url.clone()
    } else {
        memory_cfg.embedding_base_url.clone()
    };
    let api_key = if memory_cfg.embedding_api_key.is_empty() {
        llm_cfg.api_key.clone()
    } else {
        memory_cfg.embedding_api_key.clone()
    };

    match memory_cfg.embeddings.to_lowercase().as_str() {
        "tfidf" => Arc::new(TfidfEmbedder::new()),
        "openai" => Arc::new(OpenAiEmbedder::new(&base_url, &api_key, &memory_cfg.embedding_model)),
        _ => {
            // auto
            if api_key.is_empty() || base_url.is_empty() {
                tracing::info!("memory embeddings: no API credentials, using offline TF-IDF backend");
                Arc::new(TfidfEmbedder::new())
            } else {
                // Credentials alone do not guarantee the endpoint implements
                // /embeddings (many chat-only gateways return 404). Wrap in a
                // fallback so a failing provider degrades to offline TF-IDF
                // instead of breaking every memory operation.
                Arc::new(FallbackEmbedder::new(Arc::new(OpenAiEmbedder::new(
                    &base_url,
                    &api_key,
                    &memory_cfg.embedding_model,
                ))))
            }
        }
    }
}

/// Wraps a primary (HTTP) embedder and permanently degrades to the offline
/// TF-IDF backend after the first failure — Memora's non-strict mode. The
/// model name flips with the backend, so vectors from the two regimes are
/// never compared against each other (`rebuild_embeddings` re-embeds the
/// old rows under the new model).
pub struct FallbackEmbedder {
    primary: Arc<dyn Embedder>,
    fallback: TfidfEmbedder,
    poisoned: std::sync::atomic::AtomicBool,
}

impl FallbackEmbedder {
    pub fn new(primary: Arc<dyn Embedder>) -> Self {
        Self {
            primary,
            fallback: TfidfEmbedder::new(),
            poisoned: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl Embedder for FallbackEmbedder {
    fn model_name(&self) -> &str {
        if self.poisoned.load(std::sync::atomic::Ordering::Relaxed) {
            self.fallback.model_name()
        } else {
            self.primary.model_name()
        }
    }

    async fn embed(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
        if self.poisoned.load(std::sync::atomic::Ordering::Relaxed) {
            return self.fallback.embed(texts).await;
        }
        match self.primary.embed(texts).await {
            Ok(v) => Ok(v),
            Err(e) => {
                tracing::warn!(
                    "embeddings endpoint failed ({e}); falling back to offline TF-IDF for this process"
                );
                self.poisoned.store(true, std::sync::atomic::Ordering::Relaxed);
                self.fallback.embed(texts).await
            }
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tfidf_similar_texts_rank_higher() {
        let emb = TfidfEmbedder::new();
        // The hashing embedder is purely lexical, so "related" must share
        // most tokens with the anchor; the unrelated text shares none.
        let texts = vec![
            "billing service runs on postgres version sixteen".to_string(),
            "the billing service runs on postgres version sixteen today".to_string(),
            "the chief executive announced a marketing campaign".to_string(),
        ];
        let vecs = emb.embed(&texts).await.unwrap();
        let sim_related = cosine(&vecs[0], &vecs[1]);
        let sim_unrelated = cosine(&vecs[0], &vecs[2]);
        assert!(
            sim_related > sim_unrelated,
            "related {sim_related} should exceed unrelated {sim_unrelated}"
        );
        assert!(sim_related > 0.7, "high token overlap should be strong, got {sim_related}");
    }

    #[tokio::test]
    async fn tfidf_vectors_normalized() {
        let emb = TfidfEmbedder::new();
        let vecs = emb.embed(&["hello world example".to_string()]).await.unwrap();
        let norm: f32 = vecs[0].iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }

    #[tokio::test]
    async fn tfidf_empty_text_gives_zero_vector() {
        let emb = TfidfEmbedder::new();
        let vecs = emb.embed(&["".to_string(), "!!".to_string()]).await.unwrap();
        assert_eq!(vecs.len(), 2);
        assert!(vecs[0].iter().all(|x| *x == 0.0));
    }

    #[tokio::test]
    async fn tfidf_deterministic() {
        let emb = TfidfEmbedder::new();
        let a = emb.embed(&["same text".to_string()]).await.unwrap();
        let b = emb.embed(&["same text".to_string()]).await.unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn cosine_bounds() {
        assert!((cosine(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
        assert_eq!(cosine(&[], &[1.0]), 0.0);
        assert_eq!(cosine(&[1.0], &[1.0, 2.0]), 0.0);
    }

    #[test]
    fn tokenize_keeps_cyrillic_and_drops_short() {
        let toks = tokenize("Компания ООО «Ромашка» — 7 сотрудников!");
        assert!(toks.contains(&"ромашка".to_string()));
        assert!(toks.contains(&"компания".to_string()));
        assert!(toks.contains(&"ооо".to_string()));
        // Single-char tokens are dropped.
        assert!(!toks.contains(&"7".to_string()));
    }

    #[test]
    fn build_embedder_auto_falls_back_to_tfidf() {
        let mut mem = pr_core::MemoryConfig::default();
        mem.embeddings = "auto".into();
        let llm = pr_core::LlmConfig::default(); // empty api_key
        let emb = build_embedder(&mem, &llm);
        assert_eq!(emb.model_name(), TFIDF_MODEL_NAME);
    }

    #[test]
    fn build_embedder_explicit_tfidf() {
        let mut mem = pr_core::MemoryConfig::default();
        mem.embeddings = "tfidf".into();
        let mut llm = pr_core::LlmConfig::default();
        llm.api_key = "sk-test".into();
        let emb = build_embedder(&mem, &llm);
        assert_eq!(emb.model_name(), TFIDF_MODEL_NAME);
    }

    #[test]
    fn build_embedder_openai_uses_config_overrides() {
        let mut mem = pr_core::MemoryConfig::default();
        mem.embeddings = "openai".into();
        mem.embedding_base_url = "https://emb.example/v1".into();
        mem.embedding_api_key = "sk-emb".into();
        mem.embedding_model = "nomic-embed-text".into();
        let llm = pr_core::LlmConfig::default();
        let emb = build_embedder(&mem, &llm);
        assert_eq!(emb.model_name(), "nomic-embed-text");
    }

    /// A broken primary (e.g. a chat-only gateway without /embeddings) must
    /// degrade to TF-IDF instead of failing every memory operation.
    struct BrokenEmbedder;

    #[async_trait]
    impl Embedder for BrokenEmbedder {
        fn model_name(&self) -> &str {
            "broken-model"
        }
        async fn embed(&self, _texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
            Err(anyhow::anyhow!("404 Not Found"))
        }
    }

    #[tokio::test]
    async fn fallback_embedder_degrades_to_tfidf() {
        let emb = FallbackEmbedder::new(Arc::new(BrokenEmbedder));
        assert_eq!(emb.model_name(), "broken-model");
        let vecs = emb.embed(&["some text about billing".to_string()]).await.unwrap();
        assert_eq!(vecs.len(), 1);
        // After the failure the backend (and its model name) flipped.
        assert_eq!(emb.model_name(), TFIDF_MODEL_NAME);
        let again = emb.embed(&["some text about billing".to_string()]).await.unwrap();
        assert_eq!(vecs, again, "fallback must be deterministic");
    }
}
