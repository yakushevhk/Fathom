//! REST + SSE API mirroring the OpenMausBot harness surface (subset that the
//! GPUI client drives). All routes are local-first: the harness binds
//! 127.0.0.1:8799 only.

use crate::sessions;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use fathom_core::types::*;

use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

pub const PORT: u16 = 8799;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/config", get(get_config))
        .route("/api/events", get(events))
        .route("/api/bots", get(list_bots).post(create_bot))
        .route(
            "/api/bots/{id}",
            get(get_bot).patch(patch_bot).delete(delete_bot),
        )
        .route("/api/bots/{id}/read", post(mark_read))
        .route("/api/bots/{id}/soul", put(put_soul))
        .route("/api/bots/{id}/thread", get(direct_thread))
        .route(
            "/api/threads/{id}/messages",
            get(list_messages).post(send_message),
        )
        .route("/api/threads/{id}/stop", post(stop_thread))
        .route("/api/approvals", get(list_approvals))
        .route("/api/approvals/{id}", post(resolve_approval))
        .route("/api/engines", get(list_engines))
        .route("/api/engines/{kind}", put(put_engine))
        .route("/api/computers", get(list_computers))
        .route("/api/models", get(list_models))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "ok": true, "name": "fathom", "port": PORT }))
}

async fn get_config(State(s): State<Arc<AppState>>) -> impl IntoResponse {
    let mut engines = std::collections::BTreeMap::new();
    for kind in EngineKind::ALL {
        engines.insert(kind.to_string(), s.engine_config(kind));
    }
    Json(HarnessConfig {
        data_dir: s.store.data_dir.display().to_string(),
        engines,
        analytics_enabled: s
            .store
            .get_kv("analytics_enabled")
            .ok()
            .flatten()
            .map(|v| v == "true")
            .unwrap_or(false),
    })
}

// ---- events (SSE) ----------------------------------------------------------

async fn events(State(s): State<Arc<AppState>>) -> impl IntoResponse {
    let rx = s.bus.subscribe();
    let stream = futures::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    let data = serde_json::to_string(&ev).unwrap_or_default();
                    return Some((Ok::<_, Infallible>(Event::default().data(data)), rx));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(15)))
}

// ---- bots ------------------------------------------------------------------

#[derive(Deserialize)]
struct CreateBot {
    name: String,
    engine: EngineKind,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    soul: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    auto_approve: Option<bool>,
}

async fn list_bots(State(s): State<Arc<AppState>>) -> impl IntoResponse {
    Json(s.store.list_bots().unwrap_or_default())
}

async fn get_bot(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> impl IntoResponse {
    match s.store.get_bot(&id) {
        Ok(Some(b)) => Json(b).into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn create_bot(
    State(s): State<Arc<AppState>>,
    Json(body): Json<CreateBot>,
) -> impl IntoResponse {
    let now = now_ms();
    let id = new_id("bot");
    let mut seed = 0u8;
    for b in id.bytes() {
        seed = seed.wrapping_add(b);
    }
    let soul = body.soul.unwrap_or_default();
    let bot = Bot {
        id: id.clone(),
        name: body.name.trim().to_string(),
        engine: body.engine,
        model: body.model,
        soul: soul.clone(),
        avatar_seed: seed,
        cwd: body.cwd,
        auto_approve: body.auto_approve.unwrap_or(false),
        archived: false,
        computer_id: None,
        created_at: now,
        updated_at: now,
        last_message: None,
        last_activity_at: Some(now),
        unread: 0,
        working: false,
    };
    let _ = s.store.upsert_bot(&bot);
    let _ = s.store.write_soul(&id, &soul);
    // Create the DM thread eagerly so the client can navigate straight in.
    let _ = s.store.direct_thread(&id);
    let _ = s.bus.send(ServerEvent::BotUpsert { bot: bot.clone() });
    (StatusCode::CREATED, Json(bot)).into_response()
}

#[derive(Deserialize)]
struct PatchBot {
    name: Option<String>,
    engine: Option<EngineKind>,
    model: Option<Option<String>>,
    soul: Option<String>,
    cwd: Option<Option<String>>,
    auto_approve: Option<bool>,
    archived: Option<bool>,
}

async fn patch_bot(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<PatchBot>,
) -> impl IntoResponse {
    let Some(mut bot) = s.store.get_bot(&id).ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let engine_changed = body.engine.map(|e| e != bot.engine).unwrap_or(false);
    if let Some(n) = body.name {
        bot.name = n;
    }
    if let Some(e) = body.engine {
        bot.engine = e;
    }
    if let Some(m) = body.model {
        bot.model = m;
    }
    if let Some(soul) = &body.soul {
        bot.soul = soul.clone();
        let _ = s.store.write_soul(&id, soul);
    }
    if let Some(c) = body.cwd {
        bot.cwd = c;
    }
    if let Some(a) = body.auto_approve {
        bot.auto_approve = a;
    }
    if let Some(ar) = body.archived {
        bot.archived = ar;
    }
    bot.updated_at = now_ms();
    let _ = s.store.upsert_bot(&bot);
    if engine_changed {
        // Engine swap invalidates the live session for its DM thread.
        if let Ok(t) = s.store.direct_thread(&id) {
            sessions::abort_thread(&s, &t.id).await;
        }
    }
    let _ = s.bus.send(ServerEvent::BotUpsert { bot: bot.clone() });
    Json(bot).into_response()
}

async fn delete_bot(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> impl IntoResponse {
    if let Ok(t) = s.store.direct_thread(&id) {
        sessions::abort_thread(&s, &t.id).await;
    }
    let _ = s.store.delete_bot(&id);
    StatusCode::NO_CONTENT.into_response()
}

async fn mark_read(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> impl IntoResponse {
    let _ = s.store.mark_read(&id);
    StatusCode::NO_CONTENT
}

async fn put_soul(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let soul = body
        .get("soul")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if let Some(mut bot) = s.store.get_bot(&id).ok().flatten() {
        bot.soul = soul.to_string();
        let _ = s.store.upsert_bot(&bot);
        let _ = s.store.write_soul(&id, soul);
        return StatusCode::NO_CONTENT.into_response();
    }
    StatusCode::NOT_FOUND.into_response()
}

// ---- threads / messages ----------------------------------------------------

async fn direct_thread(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match s.store.direct_thread(&id) {
        Ok(t) => Json(t).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Deserialize)]
struct ListQ {
    #[serde(default = "default_limit")]
    limit: i64,
    before: Option<i64>,
}
fn default_limit() -> i64 {
    200
}

async fn list_messages(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<ListQ>,
) -> impl IntoResponse {
    Json(
        s.store
            .list_messages(&id, q.limit, q.before)
            .unwrap_or_default(),
    )
}

async fn send_message(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<SendMessage>,
) -> impl IntoResponse {
    let Some(thread) = s.store.get_thread(&id).ok().flatten() else {
        return (StatusCode::NOT_FOUND, "thread not found").into_response();
    };
    let Some(bot_id) = thread.bot_id.clone() else {
        return (StatusCode::BAD_REQUEST, "room threads not yet supported").into_response();
    };
    if body.text.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "empty message").into_response();
    }
    match sessions::start_turn(&s, &bot_id, &id, &body.text, body.model).await {
        Ok((user, pending)) => {
            Json(serde_json::json!({ "user": user, "pending": pending })).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")).into_response(),
    }
}

async fn stop_thread(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> impl IntoResponse {
    sessions::abort_thread(&s, &id).await;
    StatusCode::NO_CONTENT
}

// ---- approvals --------------------------------------------------------------

#[derive(Deserialize)]
struct ApprovalQ {
    bot_id: Option<String>,
}

async fn list_approvals(
    State(s): State<Arc<AppState>>,
    Query(q): Query<ApprovalQ>,
) -> impl IntoResponse {
    Json(
        s.store
            .pending_approvals(q.bot_id.as_deref())
            .unwrap_or_default(),
    )
}

#[derive(Deserialize)]
struct ResolveApproval {
    /// "allow" | "deny" | free-text answer
    decision: String,
    #[serde(default)]
    reason: Option<String>,
}

async fn resolve_approval(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ResolveApproval>,
) -> impl IntoResponse {
    if sessions::resolve_approval(&s, &id, &body.decision, body.reason).await {
        StatusCode::NO_CONTENT.into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

// ---- engines ----------------------------------------------------------------

async fn list_engines(State(s): State<Arc<AppState>>) -> impl IntoResponse {
    Json(s.engine_statuses().await)
}

async fn put_engine(
    State(s): State<Arc<AppState>>,
    Path(kind): Path<String>,
    Json(cfg): Json<EngineConfig>,
) -> impl IntoResponse {
    let Ok(kind) = kind.parse::<EngineKind>() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let _ = s.set_engine_config(kind, &cfg);
    let statuses = s.engine_statuses().await;
    let _ = s.bus.send(ServerEvent::Engines { engines: statuses });
    StatusCode::NO_CONTENT.into_response()
}

async fn list_models(
    State(s): State<Arc<AppState>>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let engine: EngineKind = q
        .get("engine")
        .and_then(|e| e.parse().ok())
        .unwrap_or(EngineKind::Claude);
    Json(s.driver_for(engine).model_catalog())
}

// ---- computers --------------------------------------------------------------

/// Cloud/local computers ride the Fathom supervisor in the full product; the
/// harness exposes the same shape and reports `off` until a supervisor is wired.
async fn list_computers(State(s): State<Arc<AppState>>) -> impl IntoResponse {
    let _ = s;
    Json(Vec::<Computer>::new())
}
