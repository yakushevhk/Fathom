//! Long-term semantic memory tools (mem0/Memora-inspired).
//!
//! These operate on the `pr_memory::Memory` store attached to the
//! [`ToolContext`]. They are distinct from the small file-backed `memory`
//! tool (MEMORY.md / USER.md): this is the unbounded knowledge base with
//! hybrid search, supersession chains and an absorb pipeline.
//!
//! - `memory_absorb` — store facts with dedup / conflict handling.
//! - `memory_search` — hybrid semantic + keyword retrieval.
//! - `memory_digest` — deterministic pre-session context load.
//! - `memory_boost`  — raise importance of a memory that proved useful.
//! - `memory_link`   — add a typed edge between two memories.

use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
use pr_memory::{
    AbsorbFact, AbsorbRequest, Scope, ScopeFilter,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::registry::{Tool, ToolContext};

/// Resolve the memory store from the context, or `None` when absent.
fn memory(ctx: &ToolContext) -> Option<&Arc<pr_memory::Memory>> {
    ctx.memory.as_ref()
}

/// Standard error for tools invoked without a configured memory store.
fn no_memory() -> ToolOutput {
    ToolOutput::err("memory subsystem not configured (is [memory] enabled?)")
}

/// Default scope filter: persistent memories plus this session's run facts.
fn scope_filter(ctx: &ToolContext) -> ScopeFilter {
    match &ctx.session_id {
        Some(sid) => ScopeFilter::persistent().add(Scope::Run, sid.clone()),
        None => ScopeFilter::persistent(),
    }
}

fn provenance(ctx: &ToolContext) -> String {
    match &ctx.session_id {
        Some(sid) => format!("session:{sid}"),
        None => "agent".to_string(),
    }
}

// ── memory_absorb ────────────────────────────────────────────────────────────

pub struct MemoryAbsorbTool;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct AbsorbParams {
    /// Self-contained facts to store (50-500 chars each recommended).
    facts: Vec<FactParam>,
    /// Where the facts came from, e.g. "https://acme.com/team", "user".
    /// Defaults to the current session.
    #[serde(default)]
    source: Option<String>,
    /// Scope: "agent" (general knowledge), "user" (about the user/client),
    /// or "run" (session-local). Defaults to "agent".
    #[serde(default)]
    scope: Option<String>,
    /// Optional hint for the classifier (not stored).
    #[serde(default)]
    context: Option<String>,
    /// Preview the plan without writing anything.
    #[serde(default)]
    dry_run: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct FactParam {
    content: String,
    #[serde(default)]
    tags: Vec<String>,
    /// 0.0-1.0 confidence; defaults to 0.8.
    #[serde(default)]
    confidence: Option<f64>,
    /// Free-form JSON metadata (type, company, url, ...).
    #[serde(default)]
    metadata: serde_json::Value,
}

#[async_trait]
impl Tool for MemoryAbsorbTool {
    fn name(&self) -> &str {
        "memory_absorb"
    }

    fn description(&self) -> &str {
        "Store one or more facts into long-term semantic memory. Use this to persist durable findings across sessions: verified contacts, company facts, project context, user preferences.

## Capability

Facts are deduplicated and reconciled against what is already stored: identical facts are skipped, newer versions supersede outdated ones (with an audit trail), conflicts are kept side-by-side, and near-duplicates in the same batch are consolidated. Secrets (API keys, tokens, private keys) are rejected.

## When to Use

- A finding you will want in a future session: `memory_absorb(facts=[{content: \"Acme CTO is Ivan Petrov, email ivan@acme.com (verified 2026-08)\", confidence: 0.9}])`
- A durable user/project fact: set `scope=\"user\"`.
- Preview a large batch first: `dry_run=true`.

## When NOT to Use

- Ephemeral, this-session-only context (use scratchpad).
- Large documents (use file_write).
- Anything containing secrets.

## Parameters

- `facts` (required): array of {content, tags?, confidence?, metadata?}.
- `source` (optional): provenance; defaults to the current session id.
- `scope` (optional): agent | user | run. Default agent.
- `context` (optional): classifier hint, not stored.
- `dry_run` (optional): report the plan without writing.

## Output

A summary line (created/superseded/contradicted/linked/skipped/consolidated/rejected) plus a per-fact detail list with memory ids."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(AbsorbParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: AbsorbParams = serde_json::from_value(args)?;
        let Some(mem) = memory(ctx) else {
            return Ok(no_memory());
        };

        if params.facts.is_empty() {
            return Ok(ToolOutput::err("no facts provided"));
        }

        let scope: Scope = match params.scope.as_deref().unwrap_or("agent").parse() {
            Ok(s) => s,
            Err(e) => return Ok(ToolOutput::err(e.to_string())),
        };

        let facts: Vec<AbsorbFact> = params
            .facts
            .into_iter()
            .map(|f| AbsorbFact {
                content: f.content,
                metadata: f.metadata,
                tags: f.tags,
                confidence: f.confidence,
                memory_class: None,
            })
            .collect();

        let req = AbsorbRequest {
            facts,
            source: params.source.unwrap_or_else(|| provenance(ctx)),
            scope,
            scope_key: ctx.session_id.clone().unwrap_or_default(),
            context: params.context,
            dry_run: params.dry_run,
        };

        // Use the LLM-assisted pipeline when an LLM is attached. Absorb
        // classification is high-volume: prefer the cheap fast model.
        let aux = ctx.aux_llm();
        let pipeline = match &aux {
            Some(llm) => mem.pipeline_with_llm(llm.clone()),
            None => mem.pipeline(),
        };

        match pipeline.absorb(req).await {
            Ok(report) => {
                let mut content = format!("Absorbed: {}\n", report.summary_line());
                if params.dry_run {
                    content.push_str("(dry run — nothing written)\n");
                }
                for d in &report.details {
                    content.push_str(&format!(
                        "- [{}] {} -> {}",
                        d.memory_id
                            .as_ref()
                            .map(|id| short(id))
                            .unwrap_or_else(|| "-".to_string()),
                        d.action,
                        d.fact
                    ));
                    if let Some(linked) = &d.linked_to {
                        content.push_str(&format!(" (linked to {})", short(linked)));
                    }
                    if !d.reason.is_empty() {
                        content.push_str(&format!(" — {}", d.reason));
                    }
                    content.push('\n');
                }
                Ok(ToolOutput::ok_with_meta(
                    content.trim_end().to_string(),
                    serde_json::to_value(&report).unwrap_or_default(),
                ))
            }
            Err(e) => Ok(ToolOutput::err(format!("absorb failed: {e}"))),
        }
    }
}

// ── memory_search ────────────────────────────────────────────────────────────

pub struct MemorySearchTool;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct SearchParams {
    /// Natural-language query.
    query: String,
    /// Max results (default from config, typically 5).
    #[serde(default)]
    top_k: Option<usize>,
    /// Also include archived/superseded history for this id instead of a
    /// free-text search (follow=latest|full_history).
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    follow: Option<String>,
}

#[async_trait]
impl Tool for MemorySearchTool {
    fn name(&self) -> &str {
        "memory_search"
    }

    fn description(&self) -> &str {
        "Search long-term semantic memory. Combines vector similarity with keyword (BM25) matching and recency decay, so both paraphrased and exact-term queries work.

## When to Use

- Before re-researching anything, check whether it is already known.
- Retrieve a specific memory by (prefix of) id with `id`; use `follow=\"latest\"` to get the newest version or `follow=\"full_history\"` to see the whole evolution.

## Parameters

- `query` (required unless `id` is given): natural-language query.
- `top_k` (optional): max results.
- `id` (optional): fetch one memory by id/prefix.
- `follow` (optional, with id): active | latest | full_history.

## Output

Ranked list of memories with id, content, score, source, confidence and dates."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(SearchParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: SearchParams = serde_json::from_value(args)?;
        let Some(mem) = memory(ctx) else {
            return Ok(no_memory());
        };

        // Single-memory lookup with follow resolution.
        if let Some(id) = &params.id {
            let follow: pr_memory::Follow = match params.follow.as_deref().unwrap_or("active").parse() {
                Ok(f) => f,
                Err(e) => return Ok(ToolOutput::err(e.to_string())),
            };
            let rows = pr_memory::resolve_follow(&mem.db, id, follow)?;
            if rows.is_empty() {
                return Ok(ToolOutput::err_code(
                    format!("no active memory found for id '{id}'"),
                    "not_found",
                ));
            }
            let mut content = String::new();
            for r in &rows {
                content.push_str(&format!(
                    "[{}] ({}, source: {}, conf {:.2}, created {}) {}\n",
                    short(&r.id),
                    r.status,
                    if r.source.is_empty() { "unknown" } else { &r.source },
                    r.confidence,
                    &r.created_at[..r.created_at.len().min(10)],
                    r.content
                ));
            }
            return Ok(ToolOutput::ok(content.trim_end().to_string()));
        }

        if params.query.trim().is_empty() {
            return Ok(ToolOutput::err("empty query"));
        }

        // With reranking enabled we pull a wider candidate pool, let the LLM
        // order it, then trim to top_k. Reranking prefers the cheap fast
        // model when one is configured.
        let aux = ctx.aux_llm();
        let rerank = mem.config.rerank && aux.is_some();
        let top_k = params.top_k.unwrap_or(mem.config.top_k as usize);
        let fetch_k = if rerank { top_k * 3 } else { top_k };
        let mut hits = mem.search(&params.query, &scope_filter(ctx), Some(fetch_k)).await?;
        if rerank {
            if let Some(llm) = &aux {
                hits = pr_memory::llm_rerank(llm, &params.query, hits).await;
                hits.truncate(top_k);
            }
        }
        if hits.is_empty() {
            return Ok(ToolOutput::ok("No matching memories found.".to_string()));
        }
        let mut content = String::new();
        for h in &hits {
            content.push_str(&format!(
                "[{}] score={:.2} (conf {:.2}, {}) {}\n",
                short(&h.memory.id),
                h.score,
                h.memory.confidence,
                &h.memory.created_at[..h.memory.created_at.len().min(10)],
                h.memory.content
            ));
        }
        Ok(ToolOutput::ok(content.trim_end().to_string()))
    }
}

// ── memory_digest ────────────────────────────────────────────────────────────

pub struct MemoryDigestTool;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct DigestParams {
    /// The topic/task to load context for.
    topic: String,
}

#[async_trait]
impl Tool for MemoryDigestTool {
    fn name(&self) -> &str {
        "memory_digest"
    }

    fn description(&self) -> &str {
        "Load a deterministic context digest from long-term memory for a topic: the most relevant memories, any open TODOs, and recently added facts. Call this at the start of a task to avoid re-doing known work.

## Parameters

- `topic` (required): what you are about to work on.

## Output

A markdown digest with real memory ids (verify any of them via memory_search id=...)."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(DigestParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: DigestParams = serde_json::from_value(args)?;
        let Some(mem) = memory(ctx) else {
            return Ok(no_memory());
        };
        if params.topic.trim().is_empty() {
            return Ok(ToolOutput::err("empty topic"));
        }
        let digest = mem.digest(&params.topic, &scope_filter(ctx)).await?;
        let block = digest.to_prompt_block(4000);
        if block.is_empty() {
            return Ok(ToolOutput::ok("No relevant memories for this topic yet.".to_string()));
        }
        Ok(ToolOutput::ok(block))
    }
}

// ── memory_boost ─────────────────────────────────────────────────────────────

pub struct MemoryBoostTool;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct BoostParams {
    /// Memory id (or unique prefix) that proved useful.
    id: String,
    /// Importance delta; default +0.5. Use a negative value to demote.
    #[serde(default = "default_boost")]
    amount: f64,
}

fn default_boost() -> f64 {
    0.5
}

#[async_trait]
impl Tool for MemoryBoostTool {
    fn name(&self) -> &str {
        "memory_boost"
    }

    fn description(&self) -> &str {
        "Raise (or lower) the importance of a stored memory. Boost a memory when it directly helped complete the task — importance influences ranking under tight score margins and future digests.

## Parameters

- `id` (required): memory id or unique prefix.
- `amount` (optional): importance delta, default +0.5."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(BoostParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: BoostParams = serde_json::from_value(args)?;
        let Some(mem) = memory(ctx) else {
            return Ok(no_memory());
        };
        // Resolve prefix to a concrete id first.
        let Some(row) = mem.db.get(&params.id)? else {
            return Ok(ToolOutput::err_code(
                format!("no memory found for id '{}'", params.id),
                "not_found",
            ));
        };
        mem.db.boost(&row.id, params.amount)?;
        mem.db.log_history(
            &row.id,
            "boost",
            None,
            Some(&format!("{:+}", params.amount)),
        );
        Ok(ToolOutput::ok(format!(
            "Boosted memory {} by {:+}",
            short(&row.id),
            params.amount
        )))
    }
}

// ── memory_link ──────────────────────────────────────────────────────────────

pub struct MemoryLinkTool;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct LinkParams {
    /// Source memory id (or unique prefix).
    from: String,
    /// Target memory id (or unique prefix).
    to: String,
    /// Edge type: related_to | supersedes | contradicts | implements |
    /// extends | references. Default related_to.
    #[serde(default = "default_edge")]
    edge_type: String,
    /// Why the two memories are linked.
    #[serde(default)]
    reason: Option<String>,
}

fn default_edge() -> String {
    "related_to".to_string()
}

#[async_trait]
impl Tool for MemoryLinkTool {
    fn name(&self) -> &str {
        "memory_link"
    }

    fn description(&self) -> &str {
        "Add a typed edge between two stored memories. Use `related_to` for associations, `references` for citations, `implements`/`extends` for plan/execution pairs. (`supersedes` and `contradicts` are normally created automatically by memory_absorb.)

## Parameters

- `from` (required): source memory id/prefix.
- `to` (required): target memory id/prefix.
- `edge_type` (optional): default related_to.
- `reason` (optional): free-text rationale."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(LinkParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: LinkParams = serde_json::from_value(args)?;
        let Some(mem) = memory(ctx) else {
            return Ok(no_memory());
        };
        const ALLOWED: &[&str] = &[
            "related_to",
            "supersedes",
            "contradicts",
            "implements",
            "extends",
            "references",
        ];
        if !ALLOWED.contains(&params.edge_type.as_str()) {
            return Ok(ToolOutput::err(format!(
                "unknown edge_type '{}', allowed: {}",
                params.edge_type,
                ALLOWED.join(", ")
            )));
        }
        let Some(from) = mem.db.get(&params.from)? else {
            return Ok(ToolOutput::err_code(
                format!("no memory found for 'from' id '{}'", params.from),
                "not_found",
            ));
        };
        let Some(to) = mem.db.get(&params.to)? else {
            return Ok(ToolOutput::err_code(
                format!("no memory found for 'to' id '{}'", params.to),
                "not_found",
            ));
        };
        mem.db
            .add_edge(&from.id, &to.id, &params.edge_type, params.reason.as_deref())?;
        mem.db
            .log_history(&from.id, "link", None, Some(&format!("{} -> {}", params.edge_type, to.id)));
        Ok(ToolOutput::ok(format!(
            "Linked {} --{}--> {}",
            short(&from.id),
            params.edge_type,
            short(&to.id)
        )))
    }
}

/// Short display id: UUIDv7 leads with a timestamp, so the random tail is
/// the discriminating part — show the last 8 chars. `MemoryDb::get`
/// resolves these back via suffix matching.
fn short(id: &str) -> String {
    id.chars().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect()
}

// ── memory_graph ─────────────────────────────────────────────────────────────

pub struct MemoryGraphTool;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct GraphParams {
    /// Action: "add" (entities/relations), "query" (multi-hop lookup),
    /// "list" (nodes by type).
    action: String,
    /// For add: entities to create/reuse.
    #[serde(default)]
    entities: Vec<GraphEntity>,
    /// For add: relations between entities (by name).
    #[serde(default)]
    relations: Vec<GraphRelation>,
    /// For query: the starting entity name.
    #[serde(default)]
    name: Option<String>,
    /// For query/list: entity type (person, company, ...). Optional.
    #[serde(default)]
    entity_type: Option<String>,
    /// For query: max hops (1-4, default 2).
    #[serde(default)]
    depth: Option<usize>,
    /// For list: max nodes (default 50).
    #[serde(default)]
    limit: Option<usize>,
    /// Provenance marker for added triples.
    #[serde(default)]
    source: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct GraphEntity {
    name: String,
    /// person | company | project | technology | role | location | event |
    /// product | other (default other).
    #[serde(default)]
    r#type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct GraphRelation {
    /// Source entity name.
    from: String,
    /// Target entity name.
    to: String,
    /// works_at, leads, owns, member_of, located_in, related_to, ...
    relation: String,
    #[serde(default)]
    confidence: Option<f64>,
}

#[async_trait]
impl Tool for MemoryGraphTool {
    fn name(&self) -> &str {
        "memory_graph"
    }

    fn description(&self) -> &str {
        "Entity knowledge graph (person ↔ company ↔ location ...). Stores deduplicated entities and typed relations; answers multi-hop questions like \"who leads Acme\" or \"which companies are in Kazan\".

## Actions

- `add` — register entities and relations:
  `memory_graph(action=\"add\", entities=[{name:\"Ivan Petrov\",type:\"person\"},{name:\"Acme LLC\",type:\"company\"}], relations=[{from:\"Ivan Petrov\",to:\"Acme LLC\",relation:\"works_at\"}])`
- `query` — multi-hop traversal from one entity (default depth 2):
  `memory_graph(action=\"query\", name=\"Acme LLC\")`
- `list` — nodes, optionally filtered by type.

## Relations

works_at, leads, owns, member_of, located_in, founded, uses, related_to (free vocabulary — use clear snake_case verbs).

## When to Use

- Every time you establish \"who ↔ which company/role/place\" during OSINT.
- Before reporting a person-company link, query the graph to reuse known nodes (dedup by name+type is automatic)."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(GraphParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: GraphParams = serde_json::from_value(args)?;
        let Some(mem) = memory(ctx) else {
            return Ok(no_memory());
        };

        match params.action.as_str() {
            "add" => {
                if params.entities.is_empty() && params.relations.is_empty() {
                    return Ok(ToolOutput::err("add requires entities and/or relations"));
                }
                let source = params.source.unwrap_or_else(|| provenance(ctx));
                let mut nodes = 0usize;
                let mut edges = 0usize;
                for e in &params.entities {
                    mem.db
                        .upsert_entity(&e.name, e.r#type.as_deref().unwrap_or("other"), None)?;
                    nodes += 1;
                }
                let type_of = |name: &str| -> String {
                    params
                        .entities
                        .iter()
                        .find(|e| {
                            pr_memory::normalize_name(&e.name)
                                .eq_ignore_ascii_case(&pr_memory::normalize_name(name))
                        })
                        .and_then(|e| e.r#type.clone())
                        .unwrap_or_else(|| "other".to_string())
                };
                for rel in &params.relations {
                    let from_id = mem.db.upsert_entity(&rel.from, &type_of(&rel.from), None)?;
                    let to_id = mem.db.upsert_entity(&rel.to, &type_of(&rel.to), None)?;
                    mem.db.add_entity_edge(
                        &from_id,
                        &to_id,
                        &rel.relation,
                        None,
                        &source,
                        rel.confidence.unwrap_or(0.8),
                    )?;
                    edges += 1;
                }
                Ok(ToolOutput::ok(format!(
                    "Graph updated: {nodes} entit(y/ies) upserted, {edges} relation(s) added"
                )))
            }
            "query" => {
                let Some(name) = params.name.as_deref() else {
                    return Ok(ToolOutput::err("query requires 'name'"));
                };
                let Some(node) =
                    mem.db.entity_by_name(name, params.entity_type.as_deref().unwrap_or(""))?
                else {
                    return Ok(ToolOutput::err_code(
                        format!("entity '{name}' not found in the graph"),
                        "not_found",
                    ));
                };
                let paths = pr_memory::multi_hop(&mem.db, &node, params.depth.unwrap_or(2))?;
                if paths.is_empty() {
                    return Ok(ToolOutput::ok(format!(
                        "{} ({}) — no relations in the graph",
                        node.name, node.entity_type
                    )));
                }
                let mut out = String::new();
                let mut seen = std::collections::HashSet::new();
                for p in &paths {
                    let line = p.render();
                    if seen.insert(line.clone()) {
                        out.push_str("- ");
                        out.push_str(&line);
                        out.push('\n');
                    }
                }
                Ok(ToolOutput::ok(out.trim_end().to_string()))
            }
            "list" => {
                let nodes = mem.db.list_entities(
                    params.entity_type.as_deref(),
                    params.limit.unwrap_or(50),
                )?;
                if nodes.is_empty() {
                    return Ok(ToolOutput::ok("Graph is empty.".to_string()));
                }
                let mut out = String::new();
                for n in &nodes {
                    out.push_str(&format!("- {} ({})\n", n.name, n.entity_type));
                }
                Ok(ToolOutput::ok(out.trim_end().to_string()))
            }
            other => Ok(ToolOutput::err(format!(
                "unknown memory_graph action '{other}', use add/query/list"
            ))),
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use pr_core::SearchConfig;
    use std::path::PathBuf;

    async fn ctx_with_memory() -> ToolContext {
        let mem = pr_memory::Memory::in_memory(pr_core::MemoryConfig::default()).unwrap();
        ToolContext::new(PathBuf::from("/tmp"), SearchConfig::default())
            .with_memory(Arc::new(mem))
            .with_session_id("sess-test")
    }

    #[tokio::test]
    async fn absorb_then_search_roundtrip() {
        let ctx = ctx_with_memory().await;
        let absorb = MemoryAbsorbTool;
        let out = absorb
            .execute(
                serde_json::json!({
                    "facts": [
                        {"content": "Acme LLC head office is in Kazan, Tatarstan", "confidence": 0.9}
                    ]
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out.success, "{}", out.content);
        assert!(out.content.contains("1 created"), "{}", out.content);

        let search = MemorySearchTool;
        let out = search
            .execute(serde_json::json!({"query": "where is Acme head office"}), &ctx)
            .await
            .unwrap();
        assert!(out.success);
        assert!(out.content.contains("Kazan"), "{}", out.content);
    }

    #[tokio::test]
    async fn absorb_rejects_secrets() {
        let ctx = ctx_with_memory().await;
        let out = MemoryAbsorbTool
            .execute(
                serde_json::json!({"facts": [{"content": "api key is sk-proj-abcdefghijklmnopqrstuvwx"}]}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out.success);
        assert!(out.content.contains("1 rejected"), "{}", out.content);
    }

    #[tokio::test]
    async fn absorb_dry_run_writes_nothing() {
        let ctx = ctx_with_memory().await;
        let out = MemoryAbsorbTool
            .execute(
                serde_json::json!({
                    "facts": [{"content": "planned fact about future roadmap items"}],
                    "dry_run": true
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out.content.contains("dry run"));
        let out = MemorySearchTool
            .execute(serde_json::json!({"query": "future roadmap items"}), &ctx)
            .await
            .unwrap();
        assert!(out.content.contains("No matching memories"));
    }

    #[tokio::test]
    async fn digest_and_boost_and_link() {
        let ctx = ctx_with_memory().await;
        let _ = MemoryAbsorbTool
            .execute(
                serde_json::json!({
                    "facts": [
                        {"content": "billing rewrite project uses the new Rust workspace"},
                        {"content": "verify emails before pushing to CRM system", "tags": ["todo"]}
                    ]
                }),
                &ctx,
            )
            .await
            .unwrap();

        let digest = MemoryDigestTool
            .execute(serde_json::json!({"topic": "billing rewrite"}), &ctx)
            .await
            .unwrap();
        assert!(digest.success);
        assert!(digest.content.contains("digest"), "{}", digest.content);
        assert!(digest.content.contains("Open TODOs"), "{}", digest.content);

        // Extract an id from the search output to boost/link.
        let search_out = MemorySearchTool
            .execute(serde_json::json!({"query": "billing rewrite rust workspace"}), &ctx)
            .await
            .unwrap();
        let id = search_out
            .content
            .lines()
            .next()
            .unwrap()
            .trim_start_matches('[')
            .split(']')
            .next()
            .unwrap()
            .to_string();

        let boost = MemoryBoostTool
            .execute(serde_json::json!({"id": id, "amount": 0.5}), &ctx)
            .await
            .unwrap();
        assert!(boost.success, "{}", boost.content);

        let search_out2 = MemorySearchTool
            .execute(serde_json::json!({"query": "verify emails crm"}), &ctx)
            .await
            .unwrap();
        let id2 = search_out2
            .content
            .lines()
            .next()
            .unwrap()
            .trim_start_matches('[')
            .split(']')
            .next()
            .unwrap()
            .to_string();

        let link = MemoryLinkTool
            .execute(
                serde_json::json!({"from": id, "to": id2, "edge_type": "related_to", "reason": "same project"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(link.success, "{}", link.content);

        // Invalid edge type is rejected.
        let bad = MemoryLinkTool
            .execute(serde_json::json!({"from": id, "to": id2, "edge_type": "bogus"}), &ctx)
            .await
            .unwrap();
        assert!(!bad.success);
    }

    #[tokio::test]
    async fn search_by_id_with_follow() {
        let ctx = ctx_with_memory().await;
        let out = MemoryAbsorbTool
            .execute(serde_json::json!({"facts": [{"content": "versioned fact about the pricing model v1"}]}), &ctx)
            .await
            .unwrap();
        // Pull the created id out of the detail line "[xxxxxxxx] new -> ...".
        let id = out
            .content
            .lines()
            .find(|l| l.starts_with("- ["))
            .unwrap()
            .trim_start_matches("- [")
            .split(']')
            .next()
            .unwrap()
            .to_string();

        let by_id = MemorySearchTool
            .execute(serde_json::json!({"query": "", "id": id, "follow": "latest"}), &ctx)
            .await
            .unwrap();
        assert!(by_id.success, "{}", by_id.content);
        assert!(by_id.content.contains("pricing model"));

        let missing = MemorySearchTool
            .execute(serde_json::json!({"query": "", "id": "ffffffff"}), &ctx)
            .await
            .unwrap();
        assert!(!missing.success);
    }

    #[tokio::test]
    async fn tools_error_without_memory_store() {
        let ctx = ToolContext::new(PathBuf::from("/tmp"), SearchConfig::default());
        let out = MemorySearchTool
            .execute(serde_json::json!({"query": "anything"}), &ctx)
            .await
            .unwrap();
        assert!(!out.success);
        assert!(out.content.contains("not configured"));
    }

    #[test]
    fn schemas_are_valid_json() {
        for tool in [
            Arc::new(MemoryAbsorbTool) as Arc<dyn Tool>,
            Arc::new(MemorySearchTool) as Arc<dyn Tool>,
            Arc::new(MemoryDigestTool) as Arc<dyn Tool>,
            Arc::new(MemoryBoostTool) as Arc<dyn Tool>,
            Arc::new(MemoryLinkTool) as Arc<dyn Tool>,
            Arc::new(MemoryGraphTool) as Arc<dyn Tool>,
        ] {
            let schema = tool.schema();
            assert_eq!(schema.name, tool.name());
            assert!(schema.parameters.is_object());
        }
    }

    #[tokio::test]
    async fn graph_add_query_list_roundtrip() {
        let ctx = ctx_with_memory().await;
        let graph = MemoryGraphTool;

        let out = graph
            .execute(
                serde_json::json!({
                    "action": "add",
                    "entities": [
                        {"name": "Pavel Durov", "type": "person"},
                        {"name": "Telegram", "type": "company"}
                    ],
                    "relations": [
                        {"from": "Pavel Durov", "to": "Telegram", "relation": "founded", "confidence": 0.99}
                    ],
                    "source": "unit-test"
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out.success, "{}", out.content);
        assert!(out.content.contains("2 entit"));
        assert!(out.content.contains("1 relation"));

        // Duplicate entity add reuses the node (no error, still one node).
        let out = graph
            .execute(
                serde_json::json!({
                    "action": "add",
                    "entities": [{"name": "pavel durov", "type": "person"}]
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out.success);

        let out = graph
            .execute(serde_json::json!({"action": "query", "name": "Telegram"}), &ctx)
            .await
            .unwrap();
        assert!(out.success, "{}", out.content);
        assert!(out.content.contains("Pavel Durov"), "{}", out.content);
        assert!(out.content.contains("founded"), "{}", out.content);

        let out = graph
            .execute(serde_json::json!({"action": "list", "entity_type": "company"}), &ctx)
            .await
            .unwrap();
        assert!(out.success);
        assert!(out.content.contains("Telegram"));

        let missing = graph
            .execute(serde_json::json!({"action": "query", "name": "Nobody Here"}), &ctx)
            .await
            .unwrap();
        assert!(!missing.success);
    }
}
