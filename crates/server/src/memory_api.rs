//! HTTP API for the long-term semantic memory store.
//!
//! Routes (nested under `/api/v1`, behind auth + rate limiting):
//!
//! - `GET    /memories`            — list (scope/status/limit) or search (?q=)
//! - `POST   /memories/absorb`     — absorb facts through the full pipeline
//! - `GET    /memories/stats`      — store statistics
//! - `POST   /memories/distill`    — promote run facts into agent knowledge
//! - `POST   /memories/gc`         — archive expired/stale facts, compact groups
//! - `GET    /memories/:id`        — one memory, ?follow=active|latest|full_history
//! - `DELETE /memories/:id`        — archive (soft delete)

use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use pr_memory::{AbsorbRequest, ScopeFilter};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Memory was not configured on this server.
fn memory_disabled() -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({
            "error": "memory subsystem disabled ([memory] enabled = false)"
        })),
    )
}

fn scope_filter(scope: &str, scope_key: &str) -> Result<ScopeFilter, (StatusCode, Json<serde_json::Value>)> {
    match scope {
        "" | "persistent" => Ok(ScopeFilter::persistent()),
        "all" => Ok(ScopeFilter::new()),
        s => {
            let parsed: pr_memory::Scope = s.parse().map_err(|e: anyhow::Error| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
            })?;
            Ok(ScopeFilter::new().add(parsed, scope_key))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    /// Free-text hybrid search; when present this becomes a search request.
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub scope_key: String,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
}

fn default_status() -> String {
    "active".to_string()
}
fn default_limit() -> usize {
    20
}
fn default_top_k() -> usize {
    10
}

#[derive(Debug, Serialize)]
struct MemoryDto {
    id: String,
    content: String,
    scope: String,
    scope_key: String,
    status: String,
    source: String,
    confidence: f64,
    importance: f64,
    tags: Vec<String>,
    created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    score: Option<f32>,
}

impl From<&pr_memory::MemoryRow> for MemoryDto {
    fn from(r: &pr_memory::MemoryRow) -> Self {
        Self {
            id: r.id.clone(),
            content: r.content.clone(),
            scope: r.scope.clone(),
            scope_key: r.scope_key.clone(),
            status: r.status.clone(),
            source: r.source.clone(),
            confidence: r.confidence,
            importance: r.importance,
            tags: r.tags.clone(),
            created_at: r.created_at.clone(),
            expires_at: r.expires_at.clone(),
            score: None,
        }
    }
}

/// `GET /memories?q=...` (search) or `GET /memories` (list).
pub async fn list_memories(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let Some(mem) = &state.memory else {
        return memory_disabled().into_response();
    };
    let filter = match scope_filter(&params.scope, &params.scope_key) {
        Ok(f) => f,
        Err(e) => return e.into_response(),
    };

    if !params.q.trim().is_empty() {
        match mem.search(&params.q, &filter, Some(params.top_k)).await {
            Ok(hits) => {
                let items: Vec<MemoryDto> = hits
                    .iter()
                    .map(|h| {
                        let mut dto = MemoryDto::from(&h.memory);
                        dto.score = Some(h.score);
                        dto
                    })
                    .collect();
                Json(serde_json::json!({ "query": params.q, "memories": items })).into_response()
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response(),
        }
    } else {
        let status = match params.status.as_str() {
            "all" => None,
            s => Some(s.to_string()),
        };
        match mem.db.list(&filter, status.as_deref(), params.limit) {
            Ok(rows) => {
                let items: Vec<MemoryDto> = rows.iter().map(MemoryDto::from).collect();
                Json(serde_json::json!({ "memories": items })).into_response()
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response(),
        }
    }
}

/// `POST /memories/absorb`
pub async fn absorb_memories(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AbsorbRequest>,
) -> impl IntoResponse {
    let Some(mem) = &state.memory else {
        return memory_disabled().into_response();
    };
    if req.facts.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "facts must not be empty"})),
        )
            .into_response();
    }
    // Absorb classification is a high-volume auxiliary call: prefer the
    // cheap fast model when one is configured.
    let aux = pr_llm::build_fast_provider(&state.config.llm)
        .ok()
        .flatten()
        .unwrap_or_else(|| state.llm.clone());
    let pipeline = mem.pipeline_with_llm(aux);
    match pipeline.absorb(req).await {
        Ok(report) => (StatusCode::OK, Json(serde_json::to_value(report).unwrap())).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct DistillQuery {
    #[serde(default)]
    pub session: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
}

/// `POST /memories/distill?session=&dry_run=`
pub async fn distill_memories(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DistillQuery>,
) -> impl IntoResponse {
    let Some(mem) = &state.memory else {
        return memory_disabled().into_response();
    };
    match mem.distill(params.session.as_deref(), params.dry_run).await {
        Ok(report) => Json(serde_json::to_value(report).unwrap()).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct GcQuery {
    /// Override `[memory].gc_ttl_days` for this pass.
    #[serde(default)]
    pub ttl_days: Option<u32>,
    #[serde(default)]
    pub dry_run: bool,
}

/// `POST /memories/gc?ttl_days=&dry_run=`
pub async fn gc_memories(
    State(state): State<Arc<AppState>>,
    Query(params): Query<GcQuery>,
) -> impl IntoResponse {
    let Some(mem) = &state.memory else {
        return memory_disabled().into_response();
    };
    let opts = pr_memory::GcOptions {
        ttl_days: params.ttl_days.unwrap_or(mem.config.gc_ttl_days),
        compact_above: mem.config.gc_compact_above as usize,
        dry_run: params.dry_run,
        ..Default::default()
    };
    match mem.gc(&opts).await {
        Ok(report) => Json(serde_json::to_value(report).unwrap()).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// `GET /memories/stats`
pub async fn memory_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let Some(mem) = &state.memory else {
        return memory_disabled().into_response();
    };
    let mut by_scope = serde_json::Map::new();
    for (label, scope) in [
        ("agent", pr_memory::Scope::Agent),
        ("user", pr_memory::Scope::User),
        ("run", pr_memory::Scope::Run),
    ] {
        let filter = ScopeFilter::new().add(scope, "");
        let count = |status: &str| mem.db.list(&filter, Some(status), usize::MAX).map(|v| v.len()).unwrap_or(0);
        by_scope.insert(
            label.to_string(),
            serde_json::json!({
                "active": count("active"),
                "superseded": count("superseded"),
                "archived": count("archived"),
            }),
        );
    }
    let (nodes, edges) = mem.db.count_entities().unwrap_or((0, 0));
    Json(serde_json::json!({
        "embedding_model": mem.embedder.model_name(),
        "scopes": by_scope,
        "entity_graph": {"nodes": nodes, "edges": edges},
    }))
    .into_response()
}

#[derive(Debug, Deserialize)]
pub struct GetQuery {
    #[serde(default = "default_follow")]
    pub follow: String,
}

fn default_follow() -> String {
    "latest".to_string()
}

/// `GET /memories/:id?follow=latest`
pub async fn get_memory(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<GetQuery>,
) -> impl IntoResponse {
    let Some(mem) = &state.memory else {
        return memory_disabled().into_response();
    };
    let follow: pr_memory::Follow = match params.follow.parse() {
        Ok(f) => f,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()})))
                .into_response()
        }
    };
    match pr_memory::resolve_follow(&mem.db, &id, follow) {
        Ok(rows) if rows.is_empty() => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("memory '{id}' not found")})),
        )
            .into_response(),
        Ok(rows) => {
            let items: Vec<MemoryDto> = rows.iter().map(MemoryDto::from).collect();
            Json(serde_json::json!({"memories": items})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// `DELETE /memories/:id` — archive (soft delete; history stays intact).
pub async fn archive_memory(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(mem) = &state.memory else {
        return memory_disabled().into_response();
    };
    let Ok(Some(row)) = mem.db.get(&id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("memory '{id}' not found")})),
        )
            .into_response();
    };
    if let Err(e) = mem.db.set_status(&row.id, "archived") {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }
    mem.db.log_history(&row.id, "archive", Some(&row.status), Some("archived"));
    Json(serde_json::json!({"archived": row.id})).into_response()
}
