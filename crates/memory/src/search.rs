//! Hybrid retrieval over the memory store.
//!
//! Fuses two signals (Memora SEARCH-PATTERNS model):
//!
//! ```text
//! score = w · cosine_similarity + (1 − w) · bm25_normalized
//! score × max(0, 1 − temporal_decay · days_old)   // mem0 temporal boost
//! score × (0.8 + 0.1 · reinforcement + 0.1 · confidence)  // memory engineering
//! ```
//!
//! Where `reinforcement = min(access_count / 10, 1.0)` and `confidence`
//! is the stored fact confidence (0.0–1.0).  Both act as gentle multipliers
//! so they never dominate over semantic/keyword relevance.
//!
//! Plus `follow` resolution of supersession chains and a deterministic
//! [`digest`] aggregator used to load session context at startup.

use crate::db::{MemoryDb, MemoryRow, ScopeFilter};
use crate::embed::{cosine, Embedder};
use std::sync::Arc;

/// How supersession chains are resolved when reading memories
/// (Memora LINKING-GUIDE `follow` parameter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Follow {
    /// Only memories not superseded by a newer version (search default).
    #[default]
    Active,
    /// Resolve a memory id forward along `supersedes` edges to its newest
    /// version.
    Latest,
    /// The full chain of versions, oldest first.
    FullHistory,
}

impl std::str::FromStr for Follow {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.to_lowercase().as_str() {
            "" | "active" => Ok(Self::Active),
            "latest" => Ok(Self::Latest),
            "full_history" | "full" | "history" => Ok(Self::FullHistory),
            other => anyhow::bail!("unknown follow mode '{other}', use active/latest/full_history"),
        }
    }
}

/// Parameters of a hybrid search.
#[derive(Debug, Clone)]
pub struct SearchParams {
    pub query: String,
    pub top_k: usize,
    /// Minimum fused score (0 disables the cutoff).
    pub min_score: f32,
    /// `score = w·semantic + (1−w)·keyword`.
    pub semantic_weight: f32,
    /// Linear freshness decay per day (0 disables).
    pub temporal_decay: f32,
    pub scope: ScopeFilter,
}

/// One ranked search hit.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchHit {
    pub memory: MemoryRow,
    /// Final fused score after temporal decay.
    pub score: f32,
    pub semantic: f32,
    pub keyword: f32,
}

/// Hybrid semantic + keyword search over active memories.
pub async fn hybrid_search(
    db: &MemoryDb,
    embedder: &Arc<dyn Embedder>,
    params: &SearchParams,
) -> anyhow::Result<Vec<SearchHit>> {
    let w = params.semantic_weight.clamp(0.0, 1.0);
    let candidate_pool = params.top_k.max(1) * 4;

    // ── Semantic leg ──────────────────────────────────────────────────
    let mut semantic_scores: Vec<(String, f32)> = Vec::new();
    let qvec = embedder.embed(&[params.query.clone()]).await?;
    let stored = db.load_embeddings(&params.scope, embedder.model_name())?;
    if let Some(q) = qvec.first() {
        for (id, v) in &stored {
            semantic_scores.push((id.clone(), cosine(q, v)));
        }
        semantic_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        semantic_scores.truncate(candidate_pool);
    }

    // ── Keyword leg ───────────────────────────────────────────────────
    let keyword_scores = db.keyword_search(&params.query, &params.scope, candidate_pool)?;

    // Normalize each signal to [0, 1] within its candidate set (min-max),
    // then fuse. Zero-scored candidates carry no signal and are dropped —
    // otherwise an all-zero set would normalize to 1.0 and pollute results.
    let normalize = |scores: &[(String, f32)]| -> std::collections::HashMap<String, f32> {
        let mut out = std::collections::HashMap::new();
        let scores: Vec<&(String, f32)> = scores.iter().filter(|(_, s)| *s > 1e-9).collect();
        if scores.is_empty() {
            return out;
        }
        let max = scores.iter().map(|s| s.1).fold(f32::MIN, f32::max);
        let min = scores.iter().map(|s| s.1).fold(f32::MAX, f32::min);
        let span = max - min;
        for (id, s) in scores {
            out.insert(id.clone(), if span < 1e-6 { 1.0 } else { (s - min) / span });
        }
        out
    };
    let sem_norm = normalize(&semantic_scores);
    let kw_raw: Vec<(String, f32)> = keyword_scores
        .into_iter()
        .map(|(id, s)| (id, s as f32))
        .collect();
    let kw_norm = normalize(&kw_raw);

    // Union of candidate ids.
    let mut ids: Vec<String> = sem_norm.keys().chain(kw_norm.keys()).cloned().collect();
    ids.sort();
    ids.dedup();

    let now = chrono::Utc::now();
    let mut hits: Vec<SearchHit> = Vec::new();
    for id in ids {
        let Some(row) = db.get(&id)? else { continue };
        if row.status != "active" || row.is_expired() {
            continue;
        }
        let sem = sem_norm.get(&id).copied().unwrap_or(0.0);
        let kw = kw_norm.get(&id).copied().unwrap_or(0.0);
        let fused = w * sem + (1.0 - w) * kw;
        let decayed = apply_temporal_decay(fused, &row.created_at, now, params.temporal_decay);

        // Reinforcement: frequently accessed memories rank higher.
        // Confidence: more certain facts rank higher.
        // Both are gentle multipliers (0.8 + 0.1*r + 0.1*c) so they
        // never dominate over semantic/keyword relevance.
        let reinforcement = (row.access_count as f32 / 10.0).min(1.0);
        let conf = row.confidence as f32;
        let score = decayed * (0.8 + 0.1 * reinforcement + 0.1 * conf);

        if params.min_score > 0.0 && score < params.min_score {
            continue;
        }
        hits.push(SearchHit {
            memory: row,
            score,
            semantic: sem,
            keyword: kw,
        });
    }
    // Rank: score first, importance as tie-breaker.
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.memory.importance.partial_cmp(&a.memory.importance).unwrap_or(std::cmp::Ordering::Equal))
    });
    hits.truncate(params.top_k);
    db.record_access(&hits.iter().map(|h| h.memory.id.clone()).collect::<Vec<_>>());
    Ok(hits)
}

fn apply_temporal_decay(score: f32, created_at: &str, now: chrono::DateTime<chrono::Utc>, decay: f32) -> f32 {
    if decay <= 0.0 {
        return score;
    }
    let Ok(created) = chrono::DateTime::parse_from_rfc3339(created_at) else {
        return score;
    };
    let days = (now - created.with_timezone(&chrono::Utc)).num_seconds().max(0) as f32 / 86_400.0;
    score * (1.0 - decay * days).max(0.0)
}

// ── Follow resolution ────────────────────────────────────────────────────────

/// Resolve a memory id under a follow mode.
///
/// - `Active`: the row itself if it is still active, else None.
/// - `Latest`: walk `supersedes` edges forward to the newest version.
/// - `FullHistory`: the whole chain oldest→newest (the requested id
///   resolved backwards to the chain root first).
pub fn resolve_follow(db: &MemoryDb, id: &str, follow: Follow) -> anyhow::Result<Vec<MemoryRow>> {
    let Some(row) = db.get(id)? else { return Ok(Vec::new()) };
    match follow {
        Follow::Active => Ok(if row.status == "active" { vec![row] } else { Vec::new() }),
        Follow::Latest => {
            let mut cur = row;
            let mut hops = 0;
            while let Some(next_id) = db.superseded_by(&cur.id)? {
                let Some(next) = db.get(&next_id)? else { break };
                cur = next;
                hops += 1;
                if hops > 100 {
                    anyhow::bail!("supersession chain too long at memory {id} (cycle?)");
                }
            }
            Ok(vec![cur])
        }
        Follow::FullHistory => {
            // Find the chain root (walk backwards while something supersedes us...
            // edges point newer→older, so the root is the oldest: walk from the
            // given row along outgoing supersedes edges to the end, then rebuild).
            let mut chain = vec![row];
            // Extend backwards (older versions): follow edges from current to older.
            loop {
                let edges = db.edges_of(&chain.last().unwrap().id)?;
                let older = edges
                    .iter()
                    .find(|e| e.edge_type == "supersedes" && e.from_id == chain.last().unwrap().id)
                    .map(|e| e.to_id.clone());
                match older.and_then(|oid| db.get(&oid).ok()).flatten() {
                    Some(r) => chain.push(r),
                    None => break,
                }
                if chain.len() > 100 {
                    anyhow::bail!("supersession chain too long at memory {id}");
                }
            }
            // Extend forwards (newer versions) from the requested id.
            loop {
                let newest = chain.first().unwrap();
                match db.superseded_by(&newest.id)? {
                    Some(nid) => match db.get(&nid)? {
                        Some(r) => chain.insert(0, r),
                        None => break,
                    },
                    None => break,
                }
                if chain.len() > 100 {
                    anyhow::bail!("supersession chain too long at memory {id}");
                }
            }
            chain.reverse(); // oldest first
            chain.dedup_by(|a, b| a.id == b.id);
            Ok(chain)
        }
    }
}

// ── Digest ───────────────────────────────────────────────────────────────────

/// Deterministic pre-session context load (Memora DIGEST-GUIDE): one call
/// returns buckets of real memories (with ids for verification), not
/// generated prose.
#[derive(Debug, Clone, Default)]
pub struct Digest {
    pub topic: String,
    pub relevant: Vec<SearchHit>,
    /// Memories tagged `todo` (metadata type `todo`) still open.
    pub open_todos: Vec<MemoryRow>,
    /// Most recently stored memories in scope.
    pub recent: Vec<MemoryRow>,
}

impl Digest {
    pub fn is_empty(&self) -> bool {
        self.relevant.is_empty() && self.open_todos.is_empty() && self.recent.is_empty()
    }

    /// Render as a system-prompt block, bounded by `max_chars`.
    pub fn to_prompt_block(&self, max_chars: usize) -> String {
        if self.is_empty() {
            return String::new();
        }
        let mut block = format!("## Long-term memory digest (topic: {})\n\n", self.topic);

        if !self.relevant.is_empty() {
            block.push_str("### Relevant memories\n");
            for hit in &self.relevant {
                block.push_str(&format!(
                    "- [{}] {} (source: {}, confidence: {:.2})\n",
                    short_id(&hit.memory.id),
                    hit.memory.content,
                    if hit.memory.source.is_empty() { "unknown" } else { &hit.memory.source },
                    hit.memory.confidence,
                ));
            }
        }
        if !self.open_todos.is_empty() {
            block.push_str("\n### Open TODOs\n");
            for t in &self.open_todos {
                block.push_str(&format!("- [{}] {}\n", short_id(&t.id), t.content));
            }
        }
        if !self.recent.is_empty() {
            block.push_str("\n### Recently added\n");
            for r in &self.recent {
                block.push_str(&format!("- [{}] {}\n", short_id(&r.id), r.content));
            }
        }
        block.push_str(
            "\nUse memory_search/memory_digest tools to pull more detail; \
             memory_boost a memory id when it proved useful.\n",
        );

        if block.len() <= max_chars {
            return block;
        }
        // Over budget: keep relevant memories only, trimmed entry by entry.
        let mut trimmed = format!("## Long-term memory digest (topic: {})\n\n### Relevant memories\n", self.topic);
        for hit in &self.relevant {
            let line = format!("- {}\n", hit.memory.content);
            if trimmed.len() + line.len() > max_chars {
                break;
            }
            trimmed.push_str(&line);
        }
        trimmed
    }
}

/// Short display id: UUIDv7 leads with a timestamp, so the discriminating
/// part is the random tail — show the last 8 chars.
fn short_id(id: &str) -> String {
    id.chars().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect()
}

/// Second-pass LLM reranking of hybrid search hits.
///
/// The fused score is lexical/vector-based; an LLM can judge topical
/// relevance better for the short top-k list. One call, JSON verdict:
/// `{"order": [2, 0, 1]}` — candidate indexes most→least relevant.
/// Any failure returns the original order unchanged (search must never
/// break because a rerank call failed).
pub async fn llm_rerank(
    llm: &Arc<dyn pr_llm::LlmProvider>,
    query: &str,
    mut hits: Vec<SearchHit>,
) -> Vec<SearchHit> {
    if hits.len() < 2 {
        return hits;
    }
    let mut cand_block = String::new();
    for (i, h) in hits.iter().enumerate() {
        cand_block.push_str(&format!("[{i}] {}\n", h.memory.content));
    }
    let prompt = format!(
        "You are a search relevance judge. Query: \"{query}\"\n\n\
         Candidates:\n{cand_block}\n\
         Order the candidates by relevance to the query (most relevant first). \
         Include only indexes of candidates that are actually relevant; drop irrelevant ones.\n\
         Respond with ONLY JSON: {{\"order\": [i, ...]}}"
    );
    let req = pr_llm::CompletionRequest {
        messages: vec![pr_core::message::Message::user(&prompt)],
        tools: vec![],
        temperature: Some(0.0),
        max_tokens: Some(200),
        stream: false,
    };
    let Ok(resp) = llm.complete(&req).await else {
        return hits;
    };
    let pr_core::message::Message::Assistant { content: Some(text), .. } = &resp.message else {
        return hits;
    };
    let Some(order) = parse_rerank_order(text, hits.len()) else {
        return hits;
    };
    let mut reranked: Vec<SearchHit> = order
        .into_iter()
        .filter_map(|i| hits.get(i).cloned())
        .collect();
    // Keep anything the model dropped at the tail (original order), so no
    // result silently disappears.
    let mut seen: Vec<bool> = vec![false; hits.len()];
    for h in &reranked {
        if let Some(pos) = hits.iter().position(|x| x.memory.id == h.memory.id) {
            seen[pos] = true;
        }
    }
    for (i, h) in hits.iter_mut().enumerate() {
        if !seen[i] {
            reranked.push(h.clone());
        }
    }
    std::mem::swap(&mut hits, &mut reranked);
    hits
}

fn parse_rerank_order(text: &str, max: usize) -> Option<Vec<usize>> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(&text[start..=end]).ok()?;
    let arr = v.get("order")?.as_array()?;
    let mut order: Vec<usize> = Vec::new();
    for item in arr {
        let idx = item.as_u64()? as usize;
        if idx < max && !order.contains(&idx) {
            order.push(idx);
        }
    }
    if order.is_empty() {
        None
    } else {
        Some(order)
    }
}

/// Build a digest for `topic` within `scope`.
pub async fn build_digest(
    db: &MemoryDb,
    embedder: &Arc<dyn Embedder>,
    topic: &str,
    scope: &ScopeFilter,
    top_k: usize,
    min_score: f32,
    semantic_weight: f32,
    temporal_decay: f32,
) -> anyhow::Result<Digest> {
    let relevant = hybrid_search(
        db,
        embedder,
        &SearchParams {
            query: topic.to_string(),
            top_k,
            min_score,
            semantic_weight,
            temporal_decay,
            scope: scope.clone(),
        },
    )
    .await?;

    let recent = db.list(scope, Some("active"), 3)?;

    let mut open_todos = Vec::new();
    for row in db.list(scope, Some("active"), 200)? {
        let tagged_todo = row.tags.iter().any(|t| t.eq_ignore_ascii_case("todo"));
        let typed_todo = row
            .metadata
            .get("type")
            .and_then(|t| t.as_str())
            .map(|t| t.eq_ignore_ascii_case("todo"))
            .unwrap_or(false);
        if !(tagged_todo || typed_todo) {
            continue;
        }
        let closed = row
            .metadata
            .get("status")
            .and_then(|s| s.as_str())
            .map(|s| matches!(s.to_lowercase().as_str(), "closed" | "done" | "completed"))
            .unwrap_or(false)
            || row.tags.iter().any(|t| matches!(t.to_lowercase().as_str(), "done" | "closed"));
        if !closed {
            open_todos.push(row);
        }
        if open_todos.len() >= 5 {
            break;
        }
    }

    Ok(Digest {
        topic: topic.to_string(),
        relevant,
        open_todos,
        recent,
    })
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{content_hash, MemoryRow};
    use crate::embed::TfidfEmbedder;

    fn db() -> MemoryDb {
        MemoryDb::in_memory().unwrap()
    }

    fn embedder() -> Arc<dyn Embedder> {
        Arc::new(TfidfEmbedder::new())
    }

    fn row(content: &str) -> MemoryRow {
        let now = chrono::Utc::now().to_rfc3339();
        MemoryRow {
            id: uuid::Uuid::now_v7().to_string(),
            content: content.to_string(),
            metadata: serde_json::json!({}),
            tags: vec![],
            source: "test".into(),
            scope: "agent".into(),
            scope_key: String::new(),
            confidence: 0.9,
            importance: 1.0,
            access_count: 0,
            last_accessed: None,
            status: "active".into(),
            expires_at: None,
            content_hash: content_hash(content),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    async fn store(db: &MemoryDb, emb: &Arc<dyn Embedder>, r: &MemoryRow) {
        db.insert(r).unwrap();
        db.fts_insert(&r.id, &r.content, &r.tags);
        let v = emb.embed(&[r.content.clone()]).await.unwrap();
        db.put_embedding(&r.id, emb.model_name(), &v[0]).unwrap();
    }

    fn params(query: &str) -> SearchParams {
        SearchParams {
            query: query.to_string(),
            top_k: 5,
            min_score: 0.0,
            semantic_weight: 0.7,
            temporal_decay: 0.0,
            scope: ScopeFilter::persistent(),
        }
    }

    #[tokio::test]
    async fn hybrid_ranks_relevant_first() {
        let db = db();
        let emb = embedder();
        let r1 = row("Rust workspace with eight crates and 900 tests");
        let r2 = row("the cargo workspace uses rusqlite for persistence");
        let r3 = row("marketing campaign budget was approved in June");
        for r in [&r1, &r2, &r3] {
            store(&db, &emb, r).await;
        }

        let hits = hybrid_search(&db, &emb, &params("cargo workspace crates rust")).await.unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].memory.id, r1.id, "exact-topic memory should win");
        assert!(hits.iter().any(|h| h.memory.id == r2.id));
    }

    #[tokio::test]
    async fn min_score_filters_noise() {
        let db = db();
        let emb = embedder();
        store(&db, &emb, &row("completely unrelated cooking recipe")).await;

        let mut p = params("quantum computing research");
        p.min_score = 0.5;
        let hits = hybrid_search(&db, &emb, &p).await.unwrap();
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn temporal_decay_penalizes_old_memories() {
        let old = apply_temporal_decay(
            1.0,
            &(chrono::Utc::now() - chrono::Duration::days(100)).to_rfc3339(),
            chrono::Utc::now(),
            0.01,
        );
        let fresh = apply_temporal_decay(1.0, &chrono::Utc::now().to_rfc3339(), chrono::Utc::now(), 0.01);
        assert!(fresh > old);
        assert!((old - 0.0).abs() < 1e-6, "100 days × 0.01 decay bottoms out at 0");
        // Disabled decay keeps the score.
        let same = apply_temporal_decay(0.8, "not-a-date", chrono::Utc::now(), 0.0);
        assert!((same - 0.8).abs() < 1e-6);
    }

    #[tokio::test]
    async fn follow_modes_resolve_chains() {
        let db = db();
        let v1 = row("CEO is Alice");
        let v2 = row("CEO is Bob");
        let v3 = row("CEO is Carol");
        db.insert(&v1).unwrap();
        db.insert(&v2).unwrap();
        db.insert(&v3).unwrap();
        db.add_edge(&v2.id, &v1.id, "supersedes", None).unwrap();
        db.add_edge(&v3.id, &v2.id, "supersedes", None).unwrap();
        db.set_status(&v1.id, "superseded").unwrap();
        db.set_status(&v2.id, "superseded").unwrap();

        // Active: superseded rows resolve to nothing.
        assert!(resolve_follow(&db, &v1.id, Follow::Active).unwrap().is_empty());
        assert_eq!(resolve_follow(&db, &v3.id, Follow::Active).unwrap().len(), 1);

        // Latest from the oldest version reaches the newest.
        let latest = resolve_follow(&db, &v1.id, Follow::Latest).unwrap();
        assert_eq!(latest.len(), 1);
        assert_eq!(latest[0].id, v3.id);

        // FullHistory: oldest → newest.
        let chain = resolve_follow(&db, &v2.id, Follow::FullHistory).unwrap();
        assert_eq!(chain.iter().map(|r| r.content.clone()).collect::<Vec<_>>(),
                   vec!["CEO is Alice", "CEO is Bob", "CEO is Carol"]);
    }

    #[tokio::test]
    async fn digest_collects_relevant_todos_recent() {
        let db = db();
        let emb = embedder();
        store(&db, &emb, &row("the billing service runs on PostgreSQL 16")).await;

        let mut todo = row("verify emails for the Acme team");
        todo.tags = vec!["todo".into()];
        db.insert(&todo).unwrap();
        db.fts_insert(&todo.id, &todo.content, &todo.tags);

        let mut done = row("old finished task");
        done.tags = vec!["todo".into(), "done".into()];
        db.insert(&done).unwrap();

        let digest = build_digest(&db, &emb, "PostgreSQL billing", &ScopeFilter::persistent(), 5, 0.0, 0.7, 0.0)
            .await
            .unwrap();
        assert!(!digest.relevant.is_empty());
        assert_eq!(digest.open_todos.len(), 1);
        assert_eq!(digest.open_todos[0].content, "verify emails for the Acme team");
        assert!(!digest.recent.is_empty());

        let block = digest.to_prompt_block(4000);
        assert!(block.contains("Long-term memory digest"));
        assert!(block.contains("PostgreSQL 16"));
        assert!(block.contains("Open TODOs"));
    }

    #[tokio::test]
    async fn digest_prompt_block_respects_budget() {
        let db = db();
        let emb = embedder();
        let long = format!("xenon {}", "y".repeat(3000));
        store(&db, &emb, &row(&long)).await;
        let digest = build_digest(&db, &emb, "xenon", &ScopeFilter::persistent(), 5, 0.0, 1.0, 0.0)
            .await
            .unwrap();
        assert!(!digest.relevant.is_empty());
        let block = digest.to_prompt_block(500);
        assert!(block.len() <= 500, "block {} exceeds budget", block.len());
    }

    #[test]
    fn follow_from_str() {
        assert_eq!("active".parse::<Follow>().unwrap(), Follow::Active);
        assert_eq!("".parse::<Follow>().unwrap(), Follow::Active);
        assert_eq!("latest".parse::<Follow>().unwrap(), Follow::Latest);
        assert_eq!("full_history".parse::<Follow>().unwrap(), Follow::FullHistory);
        assert!("bogus".parse::<Follow>().is_err());
    }

    #[test]
    fn parse_rerank_order_validates_indexes() {
        assert_eq!(parse_rerank_order(r#"{"order": [2, 0, 1]}"#, 3), Some(vec![2, 0, 1]));
        // Out-of-range and duplicate indexes are dropped.
        assert_eq!(parse_rerank_order(r#"{"order": [5, 1, 1, 0]}"#, 2), Some(vec![1, 0]));
        assert_eq!(parse_rerank_order(r#"{"order": []}"#, 3), None);
        assert_eq!(parse_rerank_order("no json", 3), None);
        // Chatty models: JSON is extracted from the prose.
        assert_eq!(
            parse_rerank_order("Sure, here you go: {\"order\": [1]} hope this helps!", 3),
            Some(vec![1])
        );
    }

    /// Minimal mock provider returning a canned completion.
    struct CannedLlm(String);

    #[async_trait::async_trait]
    impl pr_llm::LlmProvider for CannedLlm {
        fn name(&self) -> &str {
            "canned"
        }
        fn model(&self) -> &str {
            "canned-model"
        }
        async fn complete(
            &self,
            _req: &pr_llm::CompletionRequest,
        ) -> pr_core::PrResult<pr_llm::CompletionResponse> {
            Ok(pr_llm::CompletionResponse {
                message: pr_core::message::Message::assistant(self.0.clone()),
                usage: None,
                finish_reason: Some("stop".into()),
            })
        }
        async fn stream(
            &self,
            _req: &pr_llm::CompletionRequest,
        ) -> pr_core::PrResult<
            Box<dyn futures::Stream<Item = pr_core::PrResult<pr_llm::StreamChunk>> + Send + Unpin>,
        > {
            Err(pr_core::PrError::Llm("stream not supported in mock".into()))
        }
    }

    fn hit(id: &str, content: &str) -> SearchHit {
        let now = chrono::Utc::now().to_rfc3339();
        SearchHit {
            memory: MemoryRow {
                id: id.into(),
                content: content.into(),
                metadata: serde_json::json!({}),
                tags: vec![],
                source: "t".into(),
                scope: "agent".into(),
                scope_key: String::new(),
                confidence: 0.9,
                importance: 1.0,
                access_count: 0,
                last_accessed: None,
                status: "active".into(),
                expires_at: None,
                content_hash: String::new(),
                created_at: now.clone(),
                updated_at: now,
            },
            score: 0.5,
            semantic: 0.5,
            keyword: 0.5,
        }
    }

    #[tokio::test]
    async fn llm_rerank_reorders_hits() {
        let llm: Arc<dyn pr_llm::LlmProvider> = Arc::new(CannedLlm(r#"{"order": [2, 0]}"#.into()));
        let hits = vec![
            hit("a", "cooking recipe for borscht"),
            hit("b", "another unrelated memory about sports"),
            hit("c", "postgres migration plan for billing"),
        ];
        let out = llm_rerank(&llm, "database migration", hits).await;
        assert_eq!(out[0].memory.id, "c", "model's top pick comes first");
        assert_eq!(out[1].memory.id, "a");
        assert_eq!(out[2].memory.id, "b", "dropped candidate stays at the tail");
    }

    #[tokio::test]
    async fn llm_rerank_keeps_order_on_garbage() {
        let llm: Arc<dyn pr_llm::LlmProvider> = Arc::new(CannedLlm("I cannot do that".into()));
        let hits = vec![hit("a", "one"), hit("b", "two")];
        let out = llm_rerank(&llm, "q", hits.clone()).await;
        assert_eq!(out[0].memory.id, "a");
        assert_eq!(out[1].memory.id, "b");
    }

    #[tokio::test]
    async fn llm_rerank_single_hit_passthrough() {
        let llm: Arc<dyn pr_llm::LlmProvider> = Arc::new(CannedLlm("unused".into()));
        let hits = vec![hit("a", "only one")];
        let out = llm_rerank(&llm, "q", hits).await;
        assert_eq!(out.len(), 1);
    }
}
