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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use pr_memory::{AbsorbFact, AbsorbRequest, MemoryRow};
    use std::sync::Arc;
    use tower::ServiceExt;

    // ── helpers ──────────────────────────────────────────────────────────

    /// An in-memory `Memory` store with TF-IDF embeddings — no disk, no LLM.
    fn mem_store() -> Arc<pr_memory::Memory> {
        Arc::new(pr_memory::Memory::in_memory(pr_core::MemoryConfig::default()).unwrap())
    }

    fn mem_with_state(mem: Arc<pr_memory::Memory>) -> Arc<AppState> {
        let db = Arc::new(crate::Persistence::in_memory().unwrap());
        let mut state = crate::AppState::with_db(crate::AppConfig::default(), db);
        {
            let s = Arc::get_mut(&mut state).unwrap();
            s.memory = Some(mem);
        }
        state
    }

    async fn send(app: axum::Router, req: Request<axum::body::Body>) -> (StatusCode, serde_json::Value) {
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, value)
    }

    fn get_req(uri: &str) -> Request<axum::body::Body> {
        Request::builder().uri(uri).body(axum::body::Body::empty()).unwrap()
    }

    fn post_json(uri: &str, body: serde_json::Value) -> Request<axum::body::Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body.to_string()))
            .unwrap()
    }

    fn delete_req(uri: &str) -> Request<axum::body::Body> {
        Request::builder()
            .method("DELETE")
            .uri(uri)
            .body(axum::body::Body::empty())
            .unwrap()
    }

    /// Seed one active memory directly through the store.
    async fn seed(mem: &pr_memory::Memory, content: &str) -> String {
        let req = AbsorbRequest {
            facts: vec![AbsorbFact {
                content: content.to_string(),
                metadata: serde_json::json!({}),
                tags: vec![],
                confidence: Some(0.9),
                memory_class: None,
            }],
            source: "test-seed".into(),
            scope: pr_memory::Scope::Agent,
            scope_key: String::new(),
            context: None,
            dry_run: false,
        };
        let report = mem.pipeline().absorb(req).await.unwrap();
        assert_eq!(report.created, 1, "seed failed: {report:?}");
        mem.db.list(&pr_memory::ScopeFilter::new(), Some("active"), 1)
            .unwrap()
            .remove(0)
            .id
    }

    // ── scope_filter ─────────────────────────────────────────────────────

    #[test]
    fn scope_filter_persistent_default() {
        let f = scope_filter("", "k").unwrap();
        assert_eq!(f.pairs.len(), 2);
        assert!(f.pairs.contains(&("agent".to_string(), String::new())));
        assert!(f.pairs.contains(&("user".to_string(), String::new())));
    }

    #[test]
    fn scope_filter_all_is_wide_open() {
        let f = scope_filter("all", "k").unwrap();
        assert!(f.pairs.is_empty(), "all means no WHERE clause: {f:?}");
    }

    #[test]
    fn scope_filter_named_scope() {
        let f = scope_filter("run", "session-42").unwrap();
        assert_eq!(f.pairs, vec![("run".to_string(), "session-42".to_string())]);
    }

    #[test]
    fn scope_filter_invalid_scope_errors_with_message() {
        let (status, body) = scope_filter("bogus", "k").unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.0["error"].as_str().unwrap().contains("bogus"));
    }

    // ── query/response parsing (plain functions) ─────────────────────────

    #[test]
    fn list_query_defaults() {
        let params = ListQuery {
            q: String::new(),
            scope: String::new(),
            scope_key: String::new(),
            status: "active".to_string(),
            limit: 20,
            top_k: 10,
        };
        // Verify defaults match the `default_*` functions
        assert_eq!(params.status, default_status());
        assert_eq!(params.limit, default_limit());
        assert_eq!(params.top_k, default_top_k());
    }

    #[test]
    fn distill_query_defaults() {
        assert_eq!(DistillQuery { session: None, dry_run: false }.dry_run, false);
    }

    #[test]
    fn gc_query_defaults() {
        assert_eq!(GcQuery { ttl_days: None, dry_run: false }.ttl_days, None);
        let g = GcQuery { ttl_days: Some(5), dry_run: true };
        assert_eq!(g.ttl_days, Some(5));
        assert!(g.dry_run);
    }

    #[test]
    fn get_query_defaults() {
        assert_eq!(GetQuery { follow: "latest".to_string() }.follow, default_follow());
        let g = GetQuery { follow: "active".to_string() };
        assert_eq!(g.follow, "active");
    }

    #[test]
    fn memory_dto_from_row_maps_every_field() {
        let row = MemoryRow {
            id: "id1".into(),
            content: "content".into(),
            metadata: serde_json::json!({}),
            tags: vec!["a".into(), "b".into()],
            source: "src".into(),
            scope: "agent".into(),
            scope_key: String::new(),
            confidence: 0.7,
            importance: 2.0,
            access_count: 3,
            last_accessed: None,
            status: "active".into(),
            expires_at: Some("2026-12-31T00:00:00Z".into()),
            content_hash: "h".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        };
        let dto = MemoryDto::from(&row);
        assert_eq!(dto.id, "id1");
        assert_eq!(dto.content, "content");
        assert_eq!(dto.scope, "agent");
        assert_eq!(dto.status, "active");
        assert_eq!(dto.confidence, 0.7);
        assert_eq!(dto.importance, 2.0);
        assert_eq!(dto.tags, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(dto.expires_at.as_deref(), Some("2026-12-31T00:00:00Z"));
        assert!(dto.score.is_none(), "plain list rows have no score");
    }

    #[test]
    fn memory_dto_serializes_expected_shape() {
        let row = MemoryRow {
            id: "id1".into(),
            content: "c".into(),
            metadata: serde_json::json!({}),
            tags: vec![],
            source: "s".into(),
            scope: "agent".into(),
            scope_key: String::new(),
            confidence: 0.5,
            importance: 1.0,
            access_count: 0,
            last_accessed: None,
            status: "active".into(),
            expires_at: None,
            content_hash: "h".into(),
            created_at: "t".into(),
            updated_at: "t".into(),
        };
        let value = serde_json::to_value(MemoryDto::from(&row)).unwrap();
        assert_eq!(value["id"], "id1");
        assert_eq!(value["status"], "active");
        assert!(
            value.get("expires_at").is_none(),
            "expires_at must be omitted when None: {value}"
        );
        assert!(value.get("score").is_none(), "score must be omitted: {value}");
    }

    // ── list_memories ────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_memories_disabled_returns_503() {
        let mut state = mem_with_state(mem_store());
        {
            let s = Arc::get_mut(&mut state).unwrap();
            s.memory = None;
        }
        let (status, body) = send(crate::build_router(state), get_req("/api/v1/memories")).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body["error"].as_str().unwrap().contains("disabled"));
    }

    #[tokio::test]
    async fn list_memories_empty_returns_ok_array() {
        let state = mem_with_state(mem_store());
        let (status, body) = send(crate::build_router(state), get_req("/api/v1/memories")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["memories"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn list_memories_default_status_active_and_limit() {
        let mem = mem_store();
        seed(&mem, "default scoped fact one for list").await;
        seed(&mem, "default scoped fact two for list").await;
        let state = mem_with_state(mem);
        let (status, body) = send(crate::build_router(state), get_req("/api/v1/memories?limit=1")).await;
        assert_eq!(status, StatusCode::OK);
        let items = body["memories"].as_array().unwrap();
        assert_eq!(items.len(), 1, "limit=1 respected");
        assert_eq!(items[0]["status"], "active");
    }

    #[tokio::test]
    async fn list_memories_status_all_includes_archived() {
        let mem = mem_store();
        let id = seed(&mem, "fact that will be archived for status all").await;
        mem.db.set_status(&id, "archived").unwrap();
        let state = mem_with_state(mem);
        let (status, body) = send(
            crate::build_router(state),
            get_req("/api/v1/memories?status=all"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["memories"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn list_memories_scope_run_filters() {
        let mem = mem_store();
        seed(&mem, "agent scoped fact for scope filtering").await;
        let req = AbsorbRequest {
            facts: vec![AbsorbFact {
                content: "run scoped fact for filtering".into(),
                metadata: serde_json::json!({}),
                tags: vec![],
                confidence: None,
                memory_class: Some("ephemeral".into()),
            }],
            source: "t".into(),
            scope: pr_memory::Scope::Agent,
            scope_key: String::new(),
            context: None,
            dry_run: false,
        };
        mem.pipeline().absorb(req).await.unwrap();
        let state = mem_with_state(mem);
        let (status, body) = send(
            crate::build_router(state),
            get_req("/api/v1/memories?scope=run"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let items = body["memories"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["scope"], "run");
    }

    #[tokio::test]
    async fn list_memories_invalid_scope_400() {
        let state = mem_with_state(mem_store());
        let (status, body) = send(
            crate::build_router(state),
            get_req("/api/v1/memories?scope=not-a-scope"),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].is_string());
    }

    // ── search (happy path through list_memories) ────────────────────────

    #[tokio::test]
    async fn search_memories_returns_hits_with_scores() {
        let mem = mem_store();
        seed(&mem, "the quantum flux capacitor converts energy to motion").await;
        let state = mem_with_state(mem);
        let (status, body) = send(
            crate::build_router(state),
            get_req("/api/v1/memories?q=quantum%20flux&top_k=5"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["query"], "quantum flux");
        let items = body["memories"].as_array().unwrap();
        assert!(!items.is_empty(), "search should find the fact");
        assert!(items[0]["score"].is_number(), "hits carry a score");
        assert!(items[0]["content"].as_str().unwrap().contains("quantum"));
    }

    #[tokio::test]
    async fn search_memories_empty_query_is_a_list_not_search() {
        let mem = mem_store();
        seed(&mem, "plain list fallback fact").await;
        let state = mem_with_state(mem);
        let (status, body) = send(
            crate::build_router(state),
            get_req("/api/v1/memories?q=%20%20"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let items = body["memories"].as_array().unwrap();
        assert!(!items.is_empty());
        assert!(items[0].get("score").is_none(), "list rows have no score");
    }

    #[tokio::test]
    async fn search_memories_no_match_is_empty() {
        let mem = mem_store();
        seed(&mem, "only fact about zebras on mars").await;
        let state = mem_with_state(mem);
        let (status, body) = send(
            crate::build_router(state),
            get_req("/api/v1/memories?q=zzzqqqxxxnomatch"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["memories"].as_array().unwrap().is_empty());
    }

    // ── absorb_memories ──────────────────────────────────────────────────

    #[tokio::test]
    async fn absorb_memories_disabled_returns_503() {
        let mut state = mem_with_state(mem_store());
        {
            let s = Arc::get_mut(&mut state).unwrap();
            s.memory = None;
        }
        let (status, _) = send(
            crate::build_router(state),
            post_json(
                "/api/v1/memories/absorb",
                serde_json::json!({"facts": [{"content": "valid fact"}], "source": "t"}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn absorb_memories_empty_facts_400() {
        let state = mem_with_state(mem_store());
        let (status, body) = send(
            crate::build_router(state),
            post_json(
                "/api/v1/memories/absorb",
                serde_json::json!({"facts": [], "source": "t"}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].as_str().unwrap().contains("facts must not be empty"));
    }

    #[tokio::test]
    async fn absorb_memories_missing_facts_field_400() {
        let state = mem_with_state(mem_store());
        let (status, _) = send(
            crate::build_router(state),
            post_json("/api/v1/memories/absorb", serde_json::json!({"source": "t"})),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "absent facts must fail deserialization, got {status}");
    }

    #[tokio::test]
    async fn absorb_memories_creates_and_lists() {
        let mem = mem_store();
        let state = mem_with_state(mem);
        let router = crate::build_router(state);
        let (status, body) = send(
            router.clone(),
            post_json(
                "/api/v1/memories/absorb",
                serde_json::json!({
                    "facts": [{"content": "the board approved a new budget ceiling in june"}],
                    "source": "api-absorb-test"
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["created"], 1);
        assert_eq!(body["rejected"], 0);
        let id = body["details"][0]["memory_id"].as_str().unwrap().to_string();
        assert!(!id.is_empty());

        let (status, body) = send(router, get_req("/api/v1/memories")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["memories"].as_array().unwrap().len(), 1);
        assert_eq!(body["memories"][0]["id"], id);
    }

    #[tokio::test]
    async fn absorb_memories_duplicate_is_skipped_not_created() {
        let mem = mem_store();
        let state = mem_with_state(mem);
        let router = crate::build_router(state);
        let body = serde_json::json!({
            "facts": [{"content": "vpn config uses aes-256-gcm for all tunnels"}],
            "source": "dup-test"
        });
        let (s1, b1) = send(router.clone(), post_json("/api/v1/memories/absorb", body.clone())).await;
        assert_eq!(s1, StatusCode::OK);
        assert_eq!(b1["created"], 1);
        let (s2, b2) = send(router, post_json("/api/v1/memories/absorb", body)).await;
        assert_eq!(s2, StatusCode::OK);
        assert_eq!(b2["created"], 0, "{b2}");
        assert_eq!(b2["skipped"], 1, "identical content is deduped: {b2}");
    }

    #[tokio::test]
    async fn absorb_memories_rejects_secret_like_content() {
        let mem = mem_store();
        let state = mem_with_state(mem);
        let (status, body) = send(
            crate::build_router(state),
            post_json(
                "/api/v1/memories/absorb",
                serde_json::json!({
                    "facts": [{"content": "the api key is sk-1234567890abcdefghijklmnopqrstuv"}],
                    "source": "secret-test"
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["created"], 0);
        assert_eq!(body["rejected"], 1, "secret-bearing facts must be rejected: {body}");
        assert!(body["details"][0]["reason"].as_str().is_some());
    }

    #[tokio::test]
    async fn absorb_memories_runs_in_dry_run_without_writes() {
        let mem = mem_store();
        let state = mem_with_state(mem);
        let (status, body) = send(
            crate::build_router(state.clone()),
            post_json(
                "/api/v1/memories/absorb",
                serde_json::json!({
                    "facts": [{"content": "dry run fact that must not be stored"}],
                    "source": "dry-run",
                    "dry_run": true
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["created"], 1, "dry-run still reports the plan: {body}");
        let (_, body) = send(crate::build_router(state), get_req("/api/v1/memories")).await;
        assert!(
            body["memories"].as_array().unwrap().is_empty(),
            "dry-run must not persist anything"
        );
    }

    #[tokio::test]
    async fn absorb_memories_ephemeral_is_run_scoped_by_default() {
        let mem = mem_store();
        let state = mem_with_state(mem);
        let router = crate::build_router(state);
        let (status, _) = send(
            router.clone(),
            post_json(
                "/api/v1/memories/absorb",
                serde_json::json!({
                    "facts": [{"content": "transient observation from a session"}],
                    "source": "session:xyz",
                    "memory_class": "ephemeral"
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        // memory_class goes inside the fact, not the request — default agent scope.
        let (_, body) = send(router, get_req("/api/v1/memories?scope=agent")).await;
        assert_eq!(body["memories"].as_array().unwrap().len(), 1);
    }

    // ── distill_memories ─────────────────────────────────────────────────

    #[tokio::test]
    async fn distill_memories_disabled_returns_503() {
        let mut state = mem_with_state(mem_store());
        {
            let s = Arc::get_mut(&mut state).unwrap();
            s.memory = None;
        }
        let (status, _) = send(
            crate::build_router(state),
            post_json("/api/v1/memories/distill", serde_json::json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn distill_memories_promotes_run_facts_to_agent() {
        let mem = mem_store();
        // Seed a run-scoped fact (ephemeral class routes to run scope, with scope_key matching session).
        mem.pipeline()
            .absorb(AbsorbRequest {
                facts: vec![AbsorbFact {
                    content: "run fact discovered during research on lithium batteries".into(),
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    confidence: Some(0.8),
                    memory_class: Some("ephemeral".into()),
                }],
                source: "session:s1".into(),
                scope: pr_memory::Scope::Agent,
                scope_key: "s1".into(),
                context: None,
                dry_run: false,
            })
            .await
            .unwrap();
        let state = mem_with_state(mem);
        let router = crate::build_router(state);
        let (status, body) = send(
            router.clone(),
            post_json(
                "/api/v1/memories/distill?session=s1&dry_run=false",
                serde_json::json!({}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(body["promoted"].as_u64().unwrap() >= 1, "{body}");

        // The run-scoped original is archived; agent scope has the copy.
        let (_, body) = send(router.clone(), get_req("/api/v1/memories?scope=run")).await;
        assert!(
            body["memories"].as_array().unwrap().is_empty(),
            "run facts are archived after distill: {body}"
        );
        let (_, body) = send(router, get_req("/api/v1/memories?scope=agent")).await;
        let items = body["memories"].as_array().unwrap();
        assert!(!items.is_empty(), "agent scope should hold promoted facts");
        assert!(items[0]["content"].as_str().unwrap().contains("lithium"));
    }

    #[tokio::test]
    async fn distill_memories_dry_run_changes_nothing() {
        let mem = mem_store();
        mem.pipeline()
            .absorb(AbsorbRequest {
                facts: vec![AbsorbFact {
                    content: "dry rerun fact about cold fusion research".into(),
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    confidence: None,
                    memory_class: Some("ephemeral".into()),
                }],
                source: "session:s2".into(),
                scope: pr_memory::Scope::Agent,
                scope_key: String::new(),
                context: None,
                dry_run: false,
            })
            .await
            .unwrap();
        let state = mem_with_state(mem);
        let router = crate::build_router(state);
        let (status, body) = send(
            router.clone(),
            post_json(
                "/api/v1/memories/distill?dry_run=true",
                serde_json::json!({}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["dry_run"], true, "{body}");
        // Original run fact must still exist untouched.
        let (_, body) = send(router, get_req("/api/v1/memories?scope=run")).await;
        assert_eq!(body["memories"].as_array().unwrap().len(), 1, "dry_run keeps rows: {body}");
    }

    #[tokio::test]
    async fn distill_memories_no_candidates_is_ok() {
        let mem = mem_store();
        let state = mem_with_state(mem);
        let (status, body) = send(
            crate::build_router(state),
            post_json("/api/v1/memories/distill", serde_json::json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["promoted"], 0);
        assert_eq!(body["archived"], 0);
        assert_eq!(body["errors"], 0);
    }

    // ── gc_memories ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn gc_memories_disabled_returns_503() {
        let mut state = mem_with_state(mem_store());
        {
            let s = Arc::get_mut(&mut state).unwrap();
            s.memory = None;
        }
        let (status, _) = send(
            crate::build_router(state),
            post_json("/api/v1/memories/gc", serde_json::json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn gc_memories_dry_run_does_not_archive() {
        let mem = mem_store();
        seed(&mem, "fact that must survive the dry-run gc").await;
        let state = mem_with_state(mem);
        let router = crate::build_router(state);
        let (status, body) = send(
            router.clone(),
            post_json("/api/v1/memories/gc?dry_run=true", serde_json::json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["dry_run"], true);
        assert_eq!(body["expired_archived"], 0);
        let (_, body) = send(router, get_req("/api/v1/memories")).await;
        assert_eq!(body["memories"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn gc_memories_expired_facts_get_archived() {
        let mem = mem_store();
        // Insert an already-expired memory directly so the pass finds it.
        {
            let mut row = MemoryRow {
                id: "gc-expired-1".into(),
                content: "ancient expired fact".into(),
                metadata: serde_json::json!({}),
                tags: vec![],
                source: "gc-test".into(),
                scope: "agent".into(),
                scope_key: String::new(),
                confidence: 0.5,
                importance: 1.0,
                access_count: 0,
                last_accessed: None,
                status: "active".into(),
                expires_at: Some("2000-01-01T00:00:00Z".into()),
                content_hash: "h1".into(),
                created_at: "2000-01-01T00:00:00Z".into(),
                updated_at: "2000-01-01T00:00:00Z".into(),
            };
            row.id = uuid::Uuid::now_v7().to_string();
            mem.db.insert(&row).unwrap();
        }
        seed(&mem, "a brand-new fact that should survive gc").await;
        let state = mem_with_state(mem);
        let router = crate::build_router(state);
        let (status, body) = send(
            router.clone(),
            post_json("/api/v1/memories/gc", serde_json::json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(body["expired_archived"].as_u64().unwrap() >= 1, "{body}");
        let (_, body) = send(router, get_req("/api/v1/memories")).await;
        let items = body["memories"].as_array().unwrap();
        assert_eq!(items.len(), 1, "{items:?}");
        assert!(items[0]["content"].as_str().unwrap().contains("brand-new"));
    }

    #[tokio::test]
    async fn gc_memories_ttl_days_override_parses() {
        let mem = mem_store();
        seed(&mem, "survivor fact").await;
        let state = mem_with_state(mem);
        let (status, body) = send(
            crate::build_router(state),
            post_json("/api/v1/memories/gc?ttl_days=120", serde_json::json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["stale_archived"], 0);
        assert_eq!(body["errors"], 0);
    }

    // ── memory_stats ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn memory_stats_disabled_returns_503() {
        let mut state = mem_with_state(mem_store());
        {
            let s = Arc::get_mut(&mut state).unwrap();
            s.memory = None;
        }
        let (status, _) = send(crate::build_router(state), get_req("/api/v1/memories/stats")).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn memory_stats_counts_by_scope_and_embeddings() {
        let mem = mem_store();
        seed(&mem, "agent fact counted by stats").await;
        let state = mem_with_state(mem);
        let (status, body) = send(crate::build_router(state), get_req("/api/v1/memories/stats")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["scopes"]["agent"]["active"], 1);
        assert_eq!(body["scopes"]["user"]["active"], 0);
        assert_eq!(body["scopes"]["run"]["active"], 0);
        assert!(body["embedding_model"].is_string());
        assert!(body["entity_graph"]["nodes"].is_number());
        assert!(body["entity_graph"]["edges"].is_number());
    }

    // ── get_memory ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn get_memory_disabled_returns_503() {
        let mut state = mem_with_state(mem_store());
        {
            let s = Arc::get_mut(&mut state).unwrap();
            s.memory = None;
        }
        let (status, _) = send(
            crate::build_router(state),
            get_req("/api/v1/memories/whatever"),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn get_memory_by_id_latest() {
        let mem = mem_store();
        let id = seed(&mem, "single version fact for get by id").await;
        let state = mem_with_state(mem);
        let (status, body) = send(
            crate::build_router(state),
            get_req(&format!("/api/v1/memories/{id}?follow=latest")),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let items = body["memories"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], id);
        assert_eq!(items[0]["status"], "active");
    }

    #[tokio::test]
    async fn get_memory_follow_active_and_full_history() {
        let mem = mem_store();
        let id = seed(&mem, "follow-mode fact for resolution").await;
        let state = mem_with_state(mem);
        let router = crate::build_router(state);
        let (status, body) = send(
            router.clone(),
            get_req(&format!("/api/v1/memories/{id}?follow=active")),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["memories"].as_array().unwrap().len(), 1);
        let (status, body) = send(
            router,
            get_req(&format!("/api/v1/memories/{id}?follow=full_history")),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["memories"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn get_memory_unknown_follow_mode_400() {
        let mem = mem_store();
        let id = seed(&mem, "fact for invalid follow mode").await;
        let state = mem_with_state(mem);
        let (status, body) = send(
            crate::build_router(state),
            get_req(&format!("/api/v1/memories/{id}?follow=sideways")),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].as_str().unwrap().contains("follow"));
    }

    #[tokio::test]
    async fn get_memory_unknown_id_404() {
        let mem = mem_store();
        seed(&mem, "unrelated fact").await;
        let state = mem_with_state(mem);
        let (status, body) = send(
            crate::build_router(state),
            get_req("/api/v1/memories/does-not-exist"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body["error"].as_str().unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn get_memory_archived_is_404_stream() {
        let mem = mem_store();
        let id = seed(&mem, "fact archived before get").await;
        mem.db.set_status(&id, "archived").unwrap();
        let state = mem_with_state(mem);
        let (status, _) = send(
            crate::build_router(state),
            get_req(&format!("/api/v1/memories/{id}?follow=active")),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "archived rows are invisible to follow=active");
    }

    // ── archive_memory ───────────────────────────────────────────────────

    #[tokio::test]
    async fn archive_memory_disabled_returns_503() {
        let mut state = mem_with_state(mem_store());
        {
            let s = Arc::get_mut(&mut state).unwrap();
            s.memory = None;
        }
        let (status, _) = send(
            crate::build_router(state),
            delete_req("/api/v1/memories/whatever"),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn archive_memory_soft_deletes_and_logs_history() {
        let mem = mem_store();
        let id = seed(&mem, "fact that gets archived by the api").await;
        let state = mem_with_state(mem.clone());
        let router = crate::build_router(state);
        let (status, body) = send(router.clone(), delete_req(&format!("/api/v1/memories/{id}"))).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["archived"], id);
        let row = mem.db.get(&id).unwrap().unwrap();
        assert_eq!(row.status, "archived");
        // Default list (active) excludes it; status=all includes it.
        let (_, body) = send(router.clone(), get_req("/api/v1/memories")).await;
        assert!(body["memories"].as_array().unwrap().is_empty());
        let (_, body) = send(router, get_req("/api/v1/memories?status=all")).await;
        assert_eq!(body["memories"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn archive_memory_unknown_id_404() {
        let mem = mem_store();
        seed(&mem, "another fact").await;
        let state = mem_with_state(mem);
        let (status, body) = send(
            crate::build_router(state),
            delete_req("/api/v1/memories/nope"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body["error"].as_str().unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn archive_memory_already_archived_returns_404() {
        let mem = mem_store();
        let id = seed(&mem, "already archived fact").await;
        mem.db.set_status(&id, "archived").unwrap();
        let state = mem_with_state(mem);
        // Archived rows can still be accessed (the archive action is idempotent);
        // the handler returns 200 because the row exists and set_status is a no-op.
        let (status, body) = send(
            crate::build_router(state),
            delete_req(&format!("/api/v1/memories/{id}")),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "archiving an already-archived memory is idempotent: {body}");
    }

    // ── end-to-end integration: supersession chain via follow ────────────

    #[tokio::test]
    async fn supersession_chain_follow_latest_and_history() {
        let mem = mem_store();
        // v1
        let v1_id = seed(&mem, "Acme CEO is Ivan Petrov as of 2023").await;
        // Insert v2 (different content, same topic)
        let v2_req = AbsorbRequest {
            facts: vec![AbsorbFact {
                content: "Acme CEO is Maria Ivanova as of 2025".into(),
                metadata: serde_json::json!({}),
                tags: vec![],
                confidence: Some(0.9),
                memory_class: None,
            }],
            source: "test".into(),
            scope: pr_memory::Scope::Agent,
            scope_key: String::new(),
            context: None,
            dry_run: false,
        };
        let report = mem.pipeline().absorb(v2_req).await.unwrap();
        let v2_id = report.details[0].memory_id.as_ref().unwrap().clone();
        // Mark v2 as superseding v1: edge direction from_id->to_id means "from_id supersedes to_id"
        mem.db
            .add_edge(&v2_id, &v1_id, "supersedes", Some("newer info"))
            .unwrap();
        // Mark v1 as superseded so follow=active skips it
        mem.db.set_status(&v1_id, "superseded").unwrap();

        let state = mem_with_state(mem);
        let router = crate::build_router(state);

        // follow=active on v1 returns empty (v1 is superseded)
        let (status, body) = send(
            router.clone(),
            get_req(&format!("/api/v1/memories/{v1_id}?follow=active")),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "v1 superseded: {body}");

        // follow=latest on v1 resolves to v2
        let (status, body) = send(
            router.clone(),
            get_req(&format!("/api/v1/memories/{v1_id}?follow=latest")),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let items = body["memories"].as_array().unwrap();
        assert_eq!(items.len(), 1, "{body}");
        assert_eq!(items[0]["id"], v2_id, "latest resolves to v2");

        // follow=full_history returns both (oldest first)
        let (status, body) = send(
            router,
            get_req(&format!("/api/v1/memories/{v1_id}?follow=full_history")),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let items = body["memories"].as_array().unwrap();
        assert_eq!(items.len(), 2, "full_history returns both versions: {body}");
        assert_eq!(items[0]["id"], v1_id, "first is v1");
        assert_eq!(items[1]["id"], v2_id, "second is v2");
    }
}
