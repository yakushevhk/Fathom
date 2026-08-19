//! The absorb pipeline: how facts enter long-term memory (Memora model).
//!
//! ```text
//! facts → validate → secret scan → consolidate (N→1) → embed
//!       → dedup by hash → find candidates (cosine) → classify
//!       → apply verdict: duplicate | supersede | contradict | coexist | related | new
//! ```
//!
//! Nothing is ever overwritten: `supersede` creates a new row plus an edge
//! to the outdated one, `contradict` keeps both versions alive (the agent
//! sees both sides; confidence and recency break ties downstream),
//! `coexist` keeps both when they apply to different contexts.

use crate::db::{content_hash, MemoryDb, MemoryRow, Scope};
use crate::embed::{cosine, Embedder};
use crate::secrets;
use pr_core::message::Message;
use pr_llm::{CompletionRequest, LlmProvider};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Minimum/maximum fact sizes (Memora: self-contained facts, 50-500 chars
/// recommended; we enforce hard bounds and warn on the soft ones).
pub const MIN_FACT_CHARS: usize = 3;
pub const MAX_FACT_CHARS: usize = 5000;
/// Below this a fact is "too generic" — short facts break dedup/retrieval.
pub const SOFT_MIN_FACT_CHARS: usize = 20;
/// Facts closer than this within one batch are consolidated into one.
pub const CONSOLIDATION_SIMILARITY: f32 = 0.85;
/// Candidates must be at least this similar to be considered at all.
pub const CANDIDATE_THRESHOLD: f32 = 0.55;
/// Heuristic (no-LLM) dedup threshold.
pub const HEURISTIC_DUPLICATE: f32 = 0.97;
const MAX_CANDIDATES: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AbsorbVerdict {
    Duplicate,
    Supersede,
    Contradict,
    Coexist,
    Related,
    New,
}

impl AbsorbVerdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Duplicate => "duplicate",
            Self::Supersede => "supersede",
            Self::Contradict => "contradict",
            Self::Coexist => "coexist",
            Self::Related => "related",
            Self::New => "new",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "duplicate" | "dup" => Some(Self::Duplicate),
            "supersede" | "supersedes" | "update" => Some(Self::Supersede),
            "contradict" | "contradicts" | "conflict" => Some(Self::Contradict),
            "coexist" | "contextual" | "both_valid" => Some(Self::Coexist),
            "related" | "related_to" => Some(Self::Related),
            "new" | "add" => Some(Self::New),
            _ => None,
        }
    }
}

/// Memory durability class: how long a fact should be retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryClass {
    /// True only in this session — auto-archived after distillation.
    Ephemeral,
    /// True for months — default class, never auto-archived by TTL.
    Durable,
    /// True until a known event/date — `expires_at` should be set.
    Expiring,
}

/// One fact to absorb.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbsorbFact {
    pub content: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub tags: Vec<String>,
    /// 0.0-1.0; defaults to 0.8 when absent.
    #[serde(default)]
    pub confidence: Option<f64>,
    /// Durability class: "ephemeral", "durable", or "expiring".
    /// `None` defaults to Durable.
    #[serde(default)]
    pub memory_class: Option<String>,
}

/// A batch absorb request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbsorbRequest {
    pub facts: Vec<AbsorbFact>,
    /// Provenance marker, e.g. `session:<id>`, `research-acme-2026-08`.
    pub source: String,
    #[serde(default = "default_scope")]
    pub scope: Scope,
    #[serde(default)]
    pub scope_key: String,
    /// Hint for the classifier (not stored).
    #[serde(default)]
    pub context: Option<String>,
    /// Compute the full plan without writing anything.
    #[serde(default)]
    pub dry_run: bool,
}

fn default_scope() -> Scope {
    Scope::Agent
}

/// Per-fact outcome line for reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbsorbDetail {
    pub fact: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linked_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub similarity: Option<f32>,
    pub reason: String,
}

/// Aggregate result of one absorb call.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AbsorbReport {
    pub created: usize,
    pub skipped: usize,
    pub superseded: usize,
    pub contradicted: usize,
    pub coexisted: usize,
    pub related: usize,
    pub consolidated: usize,
    pub rejected: usize,
    pub details: Vec<AbsorbDetail>,
}

impl AbsorbReport {
    /// One-line human summary for tool output.
    pub fn summary_line(&self) -> String {
        let mut parts = vec![format!("{} created", self.created)];
        if self.superseded > 0 {
            parts.push(format!("{} superseded", self.superseded));
        }
        if self.contradicted > 0 {
            parts.push(format!("{} contradicted", self.contradicted));
        }
        if self.coexisted > 0 {
            parts.push(format!("{} coexisted", self.coexisted));
        }
        if self.related > 0 {
            parts.push(format!("{} linked", self.related));
        }
        if self.skipped > 0 {
            parts.push(format!("{} duplicates skipped", self.skipped));
        }
        if self.consolidated > 0 {
            parts.push(format!("{} consolidated", self.consolidated));
        }
        if self.rejected > 0 {
            parts.push(format!("{} rejected", self.rejected));
        }
        parts.join(", ")
    }
}

/// The pipeline itself. Stateless except for its references — safe to share.
pub struct AbsorbPipeline {
    pub db: Arc<MemoryDb>,
    pub embedder: Arc<dyn Embedder>,
    /// Optional LLM for pairwise classification; without it the pipeline
    /// falls back to a conservative similarity heuristic.
    pub llm: Option<Arc<dyn LlmProvider>>,
    /// Allow LLM classification (config switch; `llm` may still be None).
    pub llm_classify: bool,
}

impl AbsorbPipeline {
    pub fn new(db: Arc<MemoryDb>, embedder: Arc<dyn Embedder>) -> Self {
        Self {
            db,
            embedder,
            llm: None,
            llm_classify: false,
        }
    }

    pub fn with_llm(mut self, llm: Arc<dyn LlmProvider>, enabled: bool) -> Self {
        self.llm = Some(llm);
        self.llm_classify = enabled;
        self
    }

    pub async fn absorb(&self, req: AbsorbRequest) -> anyhow::Result<AbsorbReport> {
        let mut report = AbsorbReport::default();
        if req.facts.is_empty() {
            return Ok(report);
        }

        // Pull the request fields out before consuming `req.facts`.
        let source = req.source;
        let scope = req.scope;
        let scope_key = req.scope_key;
        let dry_run = req.dry_run;

        // ── Validate + secret scan ────────────────────────────────────
        let mut clean: Vec<AbsorbFact> = Vec::new();
        for fact in req.facts {
            let content = fact.content.trim().to_string();
            if content.chars().count() < MIN_FACT_CHARS {
                report.rejected += 1;
                report.details.push(AbsorbDetail {
                    fact: truncate(&content, 80),
                    action: "rejected".into(),
                    memory_id: None,
                    linked_to: None,
                    similarity: None,
                    reason: format!("too short (<{MIN_FACT_CHARS} chars)"),
                });
                continue;
            }
            if content.chars().count() > MAX_FACT_CHARS {
                report.rejected += 1;
                report.details.push(AbsorbDetail {
                    fact: truncate(&content, 80),
                    action: "rejected".into(),
                    memory_id: None,
                    linked_to: None,
                    similarity: None,
                    reason: format!("too long (>{MAX_FACT_CHARS} chars)"),
                });
                continue;
            }
            let found = secrets::detect_secrets(&content);
            if !found.is_empty() {
                report.rejected += 1;
                report.details.push(AbsorbDetail {
                    fact: truncate(&content, 80),
                    action: "rejected".into(),
                    memory_id: None,
                    linked_to: None,
                    similarity: None,
                    reason: secrets::rejection_reason(&found),
                });
                continue;
            }
            clean.push(AbsorbFact { content, ..fact });
        }
        if clean.is_empty() {
            return Ok(report);
        }

        // ── Consolidation (N→1 within this batch) ─────────────────────
        let consolidated = self.consolidate(&mut clean).await?;
        report.consolidated = consolidated;

        // ── Embed all surviving facts in one batch ────────────────────
        let texts: Vec<String> = clean.iter().map(|f| embed_text(f)).collect();
        let vectors = self.embedder.embed(&texts).await?;

        // Existing embeddings for candidate lookup.
        let scope_filter = crate::db::ScopeFilter::new().add(scope, scope_key.clone());
        let existing = self
            .db
            .load_embeddings(&scope_filter, self.embedder.model_name())?;

        for (i, fact) in clean.into_iter().enumerate() {
            let Some(vec) = vectors.get(i).cloned() else { continue };

            // Cheap exact-dedup before anything else.
            let hash = content_hash(&fact.content);
            if let Some(existing_row) =
                self.db.find_by_hash(&hash, scope.as_str(), &scope_key)?
            {
                report.skipped += 1;
                report.details.push(AbsorbDetail {
                    fact: truncate(&fact.content, 80),
                    action: "duplicate".into(),
                    memory_id: Some(existing_row.id),
                    linked_to: None,
                    similarity: Some(1.0),
                    reason: "identical content already stored".into(),
                });
                continue;
            }

            // Candidates by cosine similarity.
            let mut candidates: Vec<(String, f32)> = existing
                .iter()
                .map(|(id, v)| (id.clone(), cosine(&vec, v)))
                .filter(|(_, s)| *s >= CANDIDATE_THRESHOLD)
                .collect();
            candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            candidates.truncate(MAX_CANDIDATES);

            // Classify.
            let (verdict, target_id, reason) = if candidates.is_empty() {
                (AbsorbVerdict::New, None, "no similar memories".to_string())
            } else if let Some((tid, verdict, reason)) = self
                .classify(&fact, &candidates)
                .await
            {
                (verdict, Some(tid), reason)
            } else {
                // Heuristic fallback: confident near-duplicates → skip,
                // consolidation-threshold + shared subject → merge,
                // otherwise treat as new.
                let (tid, sim) = &candidates[0];
                if *sim >= HEURISTIC_DUPLICATE {
                    (
                        AbsorbVerdict::Duplicate,
                        Some(tid.clone()),
                        format!("similarity {sim:.2} ≥ {HEURISTIC_DUPLICATE}"),
                    )
                } else if *sim >= CONSOLIDATION_SIMILARITY
                    && self.db.get(tid)?.map_or(false, |r| shares_subject(&fact.content, &r.content))
                {
                    // Cross-call consolidation: merge into existing row.
                    if !dry_run {
                        self.db.merge_into(tid, &fact.content, &fact.tags, fact.confidence.unwrap_or(0.8))?;
                        self.db.log_history(tid, "merged", None, Some(&fact.content));
                    }
                    report.consolidated += 1;
                    report.details.push(AbsorbDetail {
                        fact: truncate(&fact.content, 80),
                        action: "merged".into(),
                        memory_id: Some(tid.clone()),
                        linked_to: None,
                        similarity: Some(*sim),
                        reason: format!("cross-call merge (sim={sim:.2})"),
                    });
                    continue;
                } else {
                    (AbsorbVerdict::New, None, "heuristic: below duplicate threshold".into())
                }
            };

            let similarity = candidates.first().map(|c| c.1);

            match verdict {
                AbsorbVerdict::Duplicate => {
                    report.skipped += 1;
                    report.details.push(AbsorbDetail {
                        fact: truncate(&fact.content, 80),
                        action: "duplicate".into(),
                        memory_id: target_id,
                        linked_to: None,
                        similarity,
                        reason,
                    });
                }
                other => {
                    let row = self.make_row(&source, scope, &scope_key, &fact, &hash);
                    if !dry_run {
                        self.db.insert(&row)?;
                        self.db.put_embedding(&row.id, self.embedder.model_name(), &vec)?;
                        self.db.fts_insert(&row.id, &row.content, &row.tags);
                        self.db.log_history(&row.id, "add", None, Some(&row.content));
                        // Entity graph ingestion (mem0): facts may carry
                        // declared entities/relations in their metadata.
                        if fact.metadata.get("entities").is_some()
                            || fact.metadata.get("relations").is_some()
                        {
                            if let Err(e) =
                                crate::graph::ingest_entities(&self.db, &fact.metadata, &row.id, &source)
                            {
                                tracing::warn!("entity ingestion failed: {e}");
                            }
                        }
                    }
                    let action = other.as_str().to_string();
                    if let Some(tid) = &target_id {
                        if !dry_run {
                            self.db.add_edge(&row.id, tid, edge_type_for(other), Some(&reason))?;
                            if other == AbsorbVerdict::Supersede {
                                self.db.set_status(tid, "superseded")?;
                                if let Some(old) = self.db.get(tid)? {
                                    self.db.log_history(
                                        tid,
                                        "superseded",
                                        Some(&old.content),
                                        Some(&row.id),
                                    );
                                }
                            } else {
                                self.db.log_history(&row.id, &action, None, Some(tid));
                            }
                        }
                    }
                    match other {
                        AbsorbVerdict::Supersede => report.superseded += 1,
                        AbsorbVerdict::Contradict => report.contradicted += 1,
                        AbsorbVerdict::Coexist => report.coexisted += 1,
                        AbsorbVerdict::Related => report.related += 1,
                        _ => report.created += 1,
                    }
                    report.details.push(AbsorbDetail {
                        fact: truncate(&fact.content, 80),
                        action,
                        memory_id: Some(row.id.clone()),
                        linked_to: target_id,
                        similarity,
                        reason,
                    });
                }
            }
        }

        Ok(report)
    }

    /// Merge near-duplicate facts of one batch (Memora consolidation N→1):
    /// greedy clustering by cosine > `CONSOLIDATION_SIMILARITY`; merged
    /// content keeps every variant as a semicolon-joined sentence list.
    async fn consolidate(&self, facts: &mut Vec<AbsorbFact>) -> anyhow::Result<usize> {
        if facts.len() < 2 {
            return Ok(0);
        }
        let texts: Vec<String> = facts.iter().map(embed_text).collect();
        let vecs = self.embedder.embed(&texts).await?;

        let n = facts.len();
        let mut cluster_of: Vec<usize> = (0..n).collect();
        for i in 0..n {
            for j in (i + 1)..n {
                if cluster_of[j] != j {
                    continue; // already merged into an earlier cluster
                }
                if cosine(&vecs[i], &vecs[j]) > CONSOLIDATION_SIMILARITY {
                    cluster_of[j] = cluster_of[i];
                }
            }
        }

        let mut merged: Vec<AbsorbFact> = Vec::new();
        let mut consumed = 0usize;
        for root in 0..n {
            if cluster_of[root] != root {
                continue; // absorbed into another cluster
            }
            let members: Vec<usize> = (0..n).filter(|&k| cluster_of[k] == root).collect();
            if members.len() == 1 {
                merged.push(facts[root].clone());
                continue;
            }
            consumed += members.len() - 1;
            let content = members
                .iter()
                .map(|&k| facts[k].content.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            let mut tags: Vec<String> = Vec::new();
            let mut metadata = serde_json::Map::new();
            let mut confidence: f64 = 0.0;
            let mut any_confidence = false;
            for &k in &members {
                for t in &facts[k].tags {
                    if !tags.contains(t) {
                        tags.push(t.clone());
                    }
                }
                if let Some(obj) = facts[k].metadata.as_object() {
                    for (key, val) in obj {
                        metadata.entry(key.clone()).or_insert_with(|| val.clone());
                    }
                }
                if let Some(c) = facts[k].confidence {
                    confidence = confidence.max(c);
                    any_confidence = true;
                }
            }
            merged.push(AbsorbFact {
                content,
                metadata: serde_json::Value::Object(metadata),
                tags,
                confidence: if any_confidence { Some(confidence) } else { None },
                memory_class: None,
            });
        }
        *facts = merged;
        Ok(consumed)
    }

    fn make_row(
        &self,
        source: &str,
        scope: Scope,
        scope_key: &str,
        fact: &AbsorbFact,
        hash: &str,
    ) -> MemoryRow {
        let now = chrono::Utc::now();

        // MemoryClass handling: ephemeral facts are always run-scoped;
        // expiring facts get a default 90-day TTL if none is set.
        let effective_scope = match fact.memory_class.as_deref() {
            Some("ephemeral") => Scope::Run,
            _ => scope,
        };
        let expires_at = fact.metadata.get("expires_at").and_then(|v| v.as_str()).map(String::from).or_else(|| {
            if fact.memory_class.as_deref() == Some("expiring") {
                Some((now + chrono::Duration::days(90)).to_rfc3339())
            } else {
                None
            }
        });

        let now_str = now.to_rfc3339();
        MemoryRow {
            id: uuid::Uuid::now_v7().to_string(),
            content: fact.content.clone(),
            metadata: fact.metadata.clone(),
            tags: fact.tags.clone(),
            source: source.to_string(),
            scope: effective_scope.as_str().to_string(),
            scope_key: scope_key.to_string(),
            confidence: fact.confidence.unwrap_or(0.8),
            importance: 1.0,
            access_count: 0,
            last_accessed: None,
            status: "active".into(),
            expires_at,
            content_hash: hash.to_string(),
            created_at: now_str.clone(),
            updated_at: now_str,
        }
    }

    /// LLM classification of one fact against its candidates. Returns
    /// `None` when no LLM is attached/allowed or the model produced an
    /// unparsable answer (caller falls back to the heuristic).
    async fn classify(
        &self,
        fact: &AbsorbFact,
        candidates: &[(String, f32)],
    ) -> Option<(String, AbsorbVerdict, String)> {
        let llm = self.llm.as_ref().filter(|_| self.llm_classify)?;

        let mut cand_block = String::new();
        for (idx, (id, sim)) in candidates.iter().enumerate() {
            let row = self.db.get(id).ok().flatten()?;
            cand_block.push_str(&format!(
                "[c{idx}] (similarity {sim:.2}, created {}, source: {}) {}\n",
                row.created_at,
                if row.source.is_empty() { "unknown" } else { &row.source },
                row.content
            ));
        }

        let context_hint = req_context_line(&fact.metadata);
        let prompt = format!(
            "You are a memory manager for a research agent. A new fact is being added.\n\
             Compare it against the existing candidate facts and choose EXACTLY ONE verdict:\n\
             - duplicate: the new fact says the same thing, no new information\n\
             - supersede: the new fact is a newer/corrected version of one candidate (the candidate becomes outdated)\n\
             - contradict: the new fact conflicts with one candidate; both may need to stay visible\n\
             - coexist: the new fact applies to a different context (e.g. \"at work\" vs \"for personal\") and both should remain active\n\
             - related: topically related but clearly distinct information\n\
             - new: none of the candidates cover the same subject\n\n\
             New fact: {}\n{context_hint}\nCandidates:\n{cand_block}\n\
             Respond with ONLY JSON: {{\"candidate\": \"c0\"..\"c{n}\" or null, \"verdict\": \"duplicate|supersede|contradict|coexist|related|new\", \"reason\": \"short\"}}",
            fact.content,
            n = candidates.len() - 1,
        );

        let req = CompletionRequest {
            messages: vec![Message::user(&prompt)],
            tools: vec![],
            temperature: Some(0.1),
            // Reasoning-модели (deepseek-v4-pro и т.п.) тратят бюджет на
            // chain-of-thought ДО ответа: при 300 токенах вердикт может
            // обрезаться (finish_reason=length, пустой content) и классификация
            // молча деградирует в эвристику. 2048 даёт reasoner запас, а сам
            // JSON-вердикт остаётся крошечным.
            max_tokens: Some(2048),
            stream: false,
        };
        let resp = llm.complete(&req).await.ok()?;
        let text = match &resp.message {
            Message::Assistant { content: Some(c), .. } => c.clone(),
            _ => return None,
        };
        parse_classify_json(&text).and_then(|(cand_idx, verdict, reason)| {
            match (cand_idx, verdict) {
                (Some(idx), v) if idx < candidates.len() => {
                    Some((candidates[idx].0.clone(), v, reason))
                }
                (None, AbsorbVerdict::New) | (_, AbsorbVerdict::New) => None, // plain add
                _ => None,
            }
        })
    }
}

fn req_context_line(metadata: &serde_json::Value) -> String {
    match metadata.get("context").and_then(|c| c.as_str()) {
        Some(c) if !c.is_empty() => format!("Context: {c}\n"),
        _ => String::new(),
    }
}

fn edge_type_for(v: AbsorbVerdict) -> &'static str {
    match v {
        AbsorbVerdict::Supersede => "supersedes",
        AbsorbVerdict::Contradict => "contradicts",
        AbsorbVerdict::Coexist => "related_to",
        AbsorbVerdict::Related => "related_to",
        _ => "related_to",
    }
}

/// Rough overlap check: do two statements share a subject?
/// Used by the cross-call consolidation heuristic.
fn shares_subject(a: &str, b: &str) -> bool {
    let a_words: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let b_words: std::collections::HashSet<&str> = b.split_whitespace().collect();
    let intersection = a_words.intersection(&b_words).count();
    let max_len = a_words.len().max(b_words.len()).max(1);
    intersection as f32 / max_len as f32 > 0.3
}

/// Embed text together with tags/metadata (Memora EMBEDDINGS: metadata
/// influences the vector).
fn embed_text(fact: &AbsorbFact) -> String {
    let mut text = fact.content.clone();
    if !fact.tags.is_empty() {
        text.push_str(" \n ");
        text.push_str(&fact.tags.join(" "));
    }
    if let Some(obj) = fact.metadata.as_object() {
        if !obj.is_empty() {
            let mut keys: Vec<&String> = obj.keys().collect();
            keys.sort();
            let pairs: Vec<String> = keys
                .iter()
                .filter(|k| k.as_str() != "context")
                .filter_map(|k| obj.get(*k).and_then(|v| v.as_str()).map(|v| format!("{k}:{v}")))
                .collect();
            if !pairs.is_empty() {
                text.push_str(" \n ");
                text.push_str(&pairs.join(" "));
            }
        }
    }
    text
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…")
    }
}

/// Parse the classifier JSON out of a possibly chatty model response.
fn parse_classify_json(text: &str) -> Option<(Option<usize>, AbsorbVerdict, String)> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(&text[start..=end]).ok()?;
    let verdict = AbsorbVerdict::parse(v.get("verdict")?.as_str()?)?;
    let reason = v
        .get("reason")
        .and_then(|r| r.as_str())
        .unwrap_or("")
        .to_string();
    let cand = v
        .get("candidate")
        .and_then(|c| c.as_str())
        .and_then(|s| s.strip_prefix('c'))
        .and_then(|n| n.parse::<usize>().ok());
    Some((cand, verdict, reason))
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScopeFilter;
    use crate::embed::TfidfEmbedder;

    fn pipeline() -> AbsorbPipeline {
        AbsorbPipeline::new(
            Arc::new(MemoryDb::in_memory().unwrap()),
            Arc::new(TfidfEmbedder::new()),
        )
    }

    fn req(facts: &[&str]) -> AbsorbRequest {
        AbsorbRequest {
            facts: facts
                .iter()
                .map(|c| AbsorbFact {
                    content: c.to_string(),
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    confidence: Some(0.9),
                    memory_class: None,
                })
                .collect(),
            source: "test".into(),
            scope: Scope::Agent,
            scope_key: String::new(),
            context: None,
            dry_run: false,
        }
    }

    #[tokio::test]
    async fn absorb_creates_memories() {
        let p = pipeline();
        let report = p
            .absorb(req(&["Acme LLC revenue grew 40% in 2025 according to Forbes"]))
            .await
            .unwrap();
        assert_eq!(report.created, 1);
        let rows = p.db.list(&ScopeFilter::persistent(), Some("active"), 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].source, "test");
        // FTS + embedding were written.
        assert!(!p.db.keyword_search("acme revenue", &ScopeFilter::persistent(), 5).unwrap().is_empty());
        assert_eq!(p.db.load_embeddings(&ScopeFilter::persistent(), p.embedder.model_name()).unwrap().len(), 1);
        // History got an add event.
        assert_eq!(p.db.history(&rows[0].id).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn exact_duplicate_skipped_by_hash() {
        let p = pipeline();
        p.absorb(req(&["the CEO of Acme announced layoffs in March"])).await.unwrap();
        let report = p
            .absorb(req(&["The CEO of Acme announced layoffs in March"]))
            .await
            .unwrap();
        assert_eq!(report.skipped, 1);
        assert_eq!(report.created, 0);
        assert_eq!(p.db.list(&ScopeFilter::persistent(), None, 10).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn near_duplicate_skipped_by_heuristic() {
        let p = pipeline();
        p.absorb(req(&["PostgreSQL 16 runs billing service production environment"]))
            .await
            .unwrap();
        // Identical token set in a different order => cosine ~1.0 for TF-IDF.
        let report = p
            .absorb(req(&["production environment billing service runs PostgreSQL 16"]))
            .await
            .unwrap();
        assert_eq!(report.skipped, 1, "heuristic should flag near-duplicate");
    }

    #[tokio::test]
    async fn secrets_rejected() {
        let p = pipeline();
        let report = p
            .absorb(req(&["the api key is sk-proj-abcdefghijklmnopqrstuvwxyz1234"]))
            .await
            .unwrap();
        assert_eq!(report.rejected, 1);
        assert!(report.details[0].reason.contains("secret"));
        assert!(p.db.list(&ScopeFilter::persistent(), None, 10).unwrap().is_empty());
    }

    #[tokio::test]
    async fn too_short_and_too_long_rejected() {
        let p = pipeline();
        let long = "x".repeat(MAX_FACT_CHARS + 1);
        let report = p.absorb(req(&["ok", &long])).await.unwrap();
        assert_eq!(report.rejected, 2);
        assert!(report.details[0].reason.contains("too short"));
        assert!(report.details[1].reason.contains("too long"));
    }

    #[tokio::test]
    async fn consolidation_merges_near_duplicates() {
        let p = pipeline();
        let report = p
            .absorb(req(&[
                "Ivan Petrov works as CTO at Acme LLC according to LinkedIn",
                "according to LinkedIn Ivan Petrov works as CTO at Acme LLC",
            ]))
            .await
            .unwrap();
        assert_eq!(report.consolidated, 1, "second variant merges into the first");
        assert_eq!(report.created, 1);
        assert_eq!(p.db.list(&ScopeFilter::persistent(), None, 10).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn dry_run_writes_nothing() {
        let p = pipeline();
        let mut r = req(&["dry run fact about the company roadmap for 2027"]);
        r.dry_run = true;
        let report = p.absorb(r).await.unwrap();
        assert_eq!(report.created, 1);
        assert!(p.db.list(&ScopeFilter::persistent(), None, 10).unwrap().is_empty());
    }

    #[tokio::test]
    async fn scopes_isolate_memories() {
        let p = pipeline();
        p.absorb(req(&["user prefers reports in Russian language"])).await.unwrap();
        let mut run_req = req(&["session found three emails for acme.com domain"]);
        run_req.scope = Scope::Run;
        run_req.scope_key = "sess-1".into();
        p.absorb(run_req).await.unwrap();

        let persistent = p.db.list(&ScopeFilter::persistent(), None, 10).unwrap();
        assert_eq!(persistent.len(), 1);
        let run_rows = p.db.list(&ScopeFilter::new().add(Scope::Run, "sess-1"), None, 10).unwrap();
        assert_eq!(run_rows.len(), 1);
    }

    #[tokio::test]
    async fn report_summary_line_format() {
        let mut r = AbsorbReport::default();
        r.created = 2;
        r.skipped = 1;
        let line = r.summary_line();
        assert!(line.contains("2 created"));
        assert!(line.contains("1 duplicates skipped"));
    }

    #[test]
    fn parse_classify_json_variants() {
        let (cand, v, _) = parse_classify_json(r#"{"candidate": "c1", "verdict": "supersede", "reason": "newer"}"#).unwrap();
        assert_eq!(cand, Some(1));
        assert_eq!(v, AbsorbVerdict::Supersede);

        let (cand, v, _) = parse_classify_json(r#"Sure! {"candidate": null, "verdict": "new", "reason": ""} hope that helps""#).unwrap();
        assert_eq!(cand, None);
        assert_eq!(v, AbsorbVerdict::New);

        assert!(parse_classify_json("no json here").is_none());
        assert!(parse_classify_json(r#"{"verdict": "bogus"}"#).is_none());
    }

    #[test]
    fn verdict_parsing_aliases() {
        assert_eq!(AbsorbVerdict::parse("UPDATE"), Some(AbsorbVerdict::Supersede));
        assert_eq!(AbsorbVerdict::parse("conflict"), Some(AbsorbVerdict::Contradict));
        assert_eq!(AbsorbVerdict::parse("add"), Some(AbsorbVerdict::New));
        assert_eq!(AbsorbVerdict::parse("???"), None);
    }

    #[test]
    fn embed_text_includes_tags_and_metadata() {
        let fact = AbsorbFact {
            content: "fact body".into(),
            metadata: serde_json::json!({"company": "Acme", "context": "hidden"}),
            tags: vec!["contact".into()],
            confidence: None,
            memory_class: None,
        };
        let text = embed_text(&fact);
        assert!(text.contains("fact body"));
        assert!(text.contains("contact"));
        assert!(text.contains("company:Acme"));
        assert!(!text.contains("hidden"), "context hint must not be embedded");
    }
}
