//! REST + SSE API mirroring the OpenMausBot harness surface. All routes are
//! local-first: the harness binds 127.0.0.1:8799 only.

use crate::sessions::{self, SendOpts, SendOutcome};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use fathom_core::types::*;

use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

pub const PORT: u16 = 8799;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/config", get(get_config).patch(patch_config))
        .route("/api/events", get(events))
        .route("/api/search", get(search))
        .route("/api/bots", get(list_bots).post(create_bot))
        .route(
            "/api/bots/{id}",
            get(get_bot).patch(patch_bot).delete(delete_bot),
        )
        .route("/api/bots/{id}/read", post(mark_read))
        .route("/api/bots/{id}/soul", put(put_soul))
        .route("/api/bots/{id}/thread", get(direct_thread))
        .route(
            "/api/bots/{id}/threads",
            get(list_threads).post(create_thread),
        )
        .route(
            "/api/threads/{id}",
            get(get_thread).patch(patch_thread).delete(delete_thread),
        )
        .route(
            "/api/threads/{id}/messages",
            get(list_messages).post(send_message),
        )
        .route("/api/threads/{id}/stop", post(stop_thread))
        .route("/api/threads/{id}/export", get(export_thread))
        .route(
            "/api/messages/{id}",
            axum::routing::patch(patch_message).delete(delete_message),
        )
        .route("/api/messages/{id}/reactions", post(react_message))
        .route("/api/attachments", post(upload_attachment))
        .route("/api/attachments/{id}", get(get_attachment))
        .route("/api/rooms", get(list_rooms).post(create_room))
        .route(
            "/api/rooms/{id}",
            get(get_room).patch(patch_room).delete(delete_room),
        )
        .route(
            "/api/rooms/{id}/threads",
            get(list_room_threads).post(create_room_thread),
        )
        .route("/api/rooms/{id}/read", post(mark_room_read))
        // Wire name for rooms is "groups" — alias the same handlers.
        .route("/api/groups", get(list_rooms).post(create_room))
        .route(
            "/api/groups/{id}",
            get(get_room).patch(patch_room).delete(delete_room),
        )
        .route(
            "/api/groups/{id}/threads",
            get(list_room_threads).post(create_room_thread),
        )
        .route("/api/groups/{id}/read", post(mark_room_read))
        .route("/api/decisions", get(list_decisions))
        .route("/api/sidebar-sections", get(sidebar_sections))
        .route("/api/cli-test", post(cli_test))
        .route("/api/usage", get(usage))
        .route("/api/usage.csv", get(usage_csv))
        .route("/api/files", get(list_files))
        .route("/api/cli-candidates", get(cli_candidates))
        .route(
            "/api/mcp/servers",
            get(list_mcp_servers).post(add_mcp_server),
        )
        .route(
            "/api/mcp/servers/{name}",
            axum::routing::delete(del_mcp_server),
        )
        // Peer comms — the bot↔bot internal tools (engines call these).
        .route("/api/internal/ask-bot", post(internal_ask_bot))
        .route("/api/internal/delegate-bot", post(internal_delegate_bot))
        .route("/api/internal/post-to-room", post(internal_post_to_room))
        .route(
            "/api/internal/coordinate-bots",
            post(internal_coordinate_bots),
        )
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
        profile_name: s
            .store
            .get_kv("profile_name")
            .ok()
            .flatten()
            .unwrap_or_default(),
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

#[derive(Deserialize)]
struct PatchConfig {
    profile_name: Option<String>,
    analytics_enabled: Option<bool>,
}

async fn patch_config(
    State(s): State<Arc<AppState>>,
    Json(body): Json<PatchConfig>,
) -> impl IntoResponse {
    if let Some(n) = body.profile_name {
        let _ = s.store.set_kv("profile_name", n.trim());
    }
    if let Some(a) = body.analytics_enabled {
        let _ = s
            .store
            .set_kv("analytics_enabled", if a { "true" } else { "false" });
    }
    StatusCode::NO_CONTENT
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
    title: Option<String>,
    #[serde(default)]
    soul: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    approval_mode: Option<ApprovalMode>,
    /// Legacy flag — folds into approval_mode.
    #[serde(default)]
    auto_approve: Option<bool>,
}

#[derive(Deserialize)]
struct ListBotsQ {
    /// `?archived` / `?archived=1` / `?archived=true` all mean "list the archive".
    #[serde(default, deserialize_with = "opt_flag")]
    archived: bool,
}

fn opt_flag<'de, D>(d: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = Option::<String>::deserialize(d)?;
    Ok(matches!(s.as_deref(), Some("1") | Some("true")))
}

async fn list_bots(
    State(s): State<Arc<AppState>>,
    Query(q): Query<ListBotsQ>,
) -> impl IntoResponse {
    if q.archived {
        Json(s.store.list_bots_filtered(true).unwrap_or_default())
    } else {
        Json(s.store.list_bots().unwrap_or_default())
    }
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
        title: body.title.unwrap_or_default(),
        description: String::new(),
        engine: body.engine,
        model: body.model,
        effort: None,
        soul: soul.clone(),
        avatar_seed: seed,
        avatar: None,
        cwd: body.cwd,
        approval_mode: body.approval_mode.unwrap_or(match body.auto_approve {
            Some(true) => ApprovalMode::Auto,
            _ => ApprovalMode::Ask,
        }),
        always_allow: vec![],
        notifications: true,
        pinned: false,
        hidden: false,
        section: None,
        park_dms: true,
        archived: false,
        computer_id: None,
        created_at: now,
        updated_at: now,
        last_message: None,
        last_activity_at: Some(now),
        unread: 0,
        working: false,
        waiting_on_you: false,
        projects: vec![],
        model_variant: None,
        soul_hash: None,
        soul_drift: false,
        mascot_expression: None,
        mascot_body: None,
        avatar_crop: None,
        computer: None,
        cloud_backend: None,
        auto_start_vps: false,
        speak_replies: false,
        voice: None,
        rewound: false,
        chief_of_staff: false,
        managed_sections: vec![],
        approve_peer_comms: false,
        peers: vec![],
        composio: false,
        browser: false,
        mcp_servers: vec![],
        browser_profile: None,
        playbooks: vec![],
        pinned_message_id: None,
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
    title: Option<String>,
    description: Option<String>,
    engine: Option<EngineKind>,
    model: Option<Option<String>>,
    effort: Option<Option<String>>,
    soul: Option<String>,
    cwd: Option<Option<String>>,
    approval_mode: Option<ApprovalMode>,
    /// Legacy flag.
    auto_approve: Option<bool>,
    always_allow: Option<Vec<String>>,
    notifications: Option<bool>,
    pinned: Option<bool>,
    hidden: Option<bool>,
    section: Option<Option<String>>,
    park_dms: Option<bool>,
    avatar: Option<Option<String>>,
    archived: Option<bool>,
    computer_id: Option<Option<String>>,
    // --- WireBot parity fields ---
    projects: Option<Vec<BotProject>>,
    model_variant: Option<Option<String>>,
    mascot_expression: Option<Option<String>>,
    mascot_body: Option<Option<String>>,
    avatar_crop: Option<Option<String>>,
    computer: Option<Option<String>>,
    cloud_backend: Option<Option<String>>,
    auto_start_vps: Option<bool>,
    speak_replies: Option<bool>,
    voice: Option<Option<String>>,
    chief_of_staff: Option<bool>,
    managed_sections: Option<Vec<String>>,
    approve_peer_comms: Option<bool>,
    peers: Option<Vec<String>>,
    composio: Option<bool>,
    browser: Option<bool>,
    mcp_servers: Option<Vec<String>>,
    browser_profile: Option<Option<String>>,
    playbooks: Option<Vec<InstalledPlaybook>>,
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
    if let Some(v) = body.title {
        bot.title = v;
    }
    if let Some(v) = body.description {
        bot.description = v;
    }
    if let Some(e) = body.engine {
        bot.engine = e;
    }
    if let Some(m) = body.model {
        bot.model = m;
    }
    if let Some(e) = body.effort {
        bot.effort = e;
    }
    if let Some(soul) = &body.soul {
        bot.soul = soul.clone();
        let _ = s.store.write_soul(&id, soul);
    }
    if let Some(c) = body.cwd {
        bot.cwd = c;
    }
    if let Some(m) = body.approval_mode {
        bot.approval_mode = m;
    }
    if let Some(a) = body.auto_approve {
        bot.approval_mode = if a {
            ApprovalMode::Auto
        } else {
            ApprovalMode::Ask
        };
    }
    if let Some(v) = body.always_allow {
        bot.always_allow = v;
    }
    if let Some(v) = body.notifications {
        bot.notifications = v;
    }
    if let Some(v) = body.pinned {
        bot.pinned = v;
    }
    if let Some(v) = body.hidden {
        bot.hidden = v;
    }
    if let Some(v) = body.section {
        bot.section = v;
    }
    if let Some(v) = body.park_dms {
        bot.park_dms = v;
    }
    if let Some(v) = body.avatar {
        bot.avatar = v;
    }
    if let Some(v) = body.computer_id {
        bot.computer_id = v;
    }
    if let Some(v) = body.projects {
        bot.projects = v;
    }
    if let Some(v) = body.model_variant {
        bot.model_variant = v;
    }
    if let Some(v) = body.mascot_expression {
        bot.mascot_expression = v;
    }
    if let Some(v) = body.mascot_body {
        bot.mascot_body = v;
    }
    if let Some(v) = body.avatar_crop {
        bot.avatar_crop = v;
    }
    if let Some(v) = body.computer {
        bot.computer = v;
    }
    if let Some(v) = body.cloud_backend {
        bot.cloud_backend = v;
    }
    if let Some(v) = body.auto_start_vps {
        bot.auto_start_vps = v;
    }
    if let Some(v) = body.speak_replies {
        bot.speak_replies = v;
    }
    if let Some(v) = body.voice {
        bot.voice = v;
    }
    if let Some(v) = body.chief_of_staff {
        bot.chief_of_staff = v;
    }
    if let Some(v) = body.managed_sections {
        bot.managed_sections = v;
    }
    if let Some(v) = body.approve_peer_comms {
        bot.approve_peer_comms = v;
    }
    if let Some(v) = body.peers {
        bot.peers = v;
    }
    if let Some(v) = body.composio {
        bot.composio = v;
    }
    if let Some(v) = body.browser {
        bot.browser = v;
    }
    if let Some(v) = body.mcp_servers {
        bot.mcp_servers = v;
    }
    if let Some(v) = body.browser_profile {
        bot.browser_profile = v;
    }
    if let Some(v) = body.playbooks {
        bot.playbooks = v;
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
    let _ = s.bus.send(ServerEvent::BotDeleted { bot_id: id });
    StatusCode::NO_CONTENT.into_response()
}

async fn mark_read(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> impl IntoResponse {
    let _ = s.store.mark_read(&id);
    if let Ok(Some(b)) = s.store.get_bot(&id) {
        let _ = s.bus.send(ServerEvent::BotUpsert { bot: b });
    }
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

async fn list_threads(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> impl IntoResponse {
    Json(s.store.bot_threads(&id).unwrap_or_default())
}

#[derive(Deserialize)]
struct CreateThread {
    #[serde(default)]
    title: Option<String>,
}

async fn create_thread(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<CreateThread>,
) -> impl IntoResponse {
    if s.store.get_bot(&id).ok().flatten().is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    match s.store.new_task_thread(&id, body.title) {
        Ok(t) => {
            let _ = s.bus.send(ServerEvent::ThreadUpsert { thread: t.clone() });
            (StatusCode::CREATED, Json(t)).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")).into_response(),
    }
}

async fn get_thread(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> impl IntoResponse {
    match s.store.get_thread(&id) {
        Ok(Some(t)) => Json(t).into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Deserialize)]
struct PatchThread {
    #[serde(default)]
    title: Option<Option<String>>,
    #[serde(default)]
    pinned_message_id: Option<Option<String>>,
    /// Archive timestamp: set to ms epoch, or null to unarchive.
    #[serde(default)]
    archived_at: Option<Option<i64>>,
    /// Per-task pinned working directory.
    #[serde(default)]
    cwd: Option<Option<String>>,
}

async fn patch_thread(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<PatchThread>,
) -> impl IntoResponse {
    let Some(mut t) = s.store.get_thread(&id).ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if let Some(v) = body.title {
        t.title = v;
    }
    if let Some(v) = body.pinned_message_id {
        t.pinned_message_id = v;
    }
    if let Some(v) = body.archived_at {
        t.archived_at = v;
    }
    if let Some(v) = body.cwd {
        t.cwd = v;
    }
    t.updated_at = now_ms();
    let _ = s.store.upsert_thread(&t);
    let _ = s.bus.send(ServerEvent::ThreadUpsert { thread: t.clone() });
    Json(t).into_response()
}

async fn delete_thread(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    sessions::abort_thread(&s, &id).await;
    let _ = s.store.delete_thread(&id);
    StatusCode::NO_CONTENT.into_response()
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
    if body.text.trim().is_empty() && body.attachments.is_empty() {
        return (StatusCode::BAD_REQUEST, "empty message").into_response();
    }
    let opts = SendOpts {
        text: body.text.clone(),
        model: body.model.clone(),
        reply_to: body.reply_to.clone(),
        attachments: body.attachments.clone(),
        sender: body.sender.clone(),
        via_api: true,
        send_id: body.send_id.clone(),
        channel_mode: body.channel_mode.clone(),
    };
    // Room threads fan out to member bots.
    if thread.kind == "room" {
        let Some(room) = s.store.get_room_by_thread(&id).ok().flatten() else {
            return (StatusCode::NOT_FOUND, "room not found").into_response();
        };
        return match sessions::start_room_turn(&s, &room.id, opts).await {
            Ok(user) => Json(serde_json::json!({ "user": user })).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")).into_response(),
        };
    }
    let Some(bot_id) = thread.bot_id.clone() else {
        return (StatusCode::BAD_REQUEST, "thread has no bot").into_response();
    };
    match sessions::start_turn(&s, &bot_id, &id, opts).await {
        Ok(SendOutcome::Started { user, pending }) => {
            Json(serde_json::json!({ "user": user, "pending": pending })).into_response()
        }
        Ok(SendOutcome::Queued { user }) => {
            Json(serde_json::json!({ "user": user, "queued": true })).into_response()
        }
        Ok(SendOutcome::Deduped { user }) => {
            Json(serde_json::json!({ "user": user, "deduped": true })).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")).into_response(),
    }
}

async fn stop_thread(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> impl IntoResponse {
    sessions::abort_thread(&s, &id).await;
    StatusCode::NO_CONTENT
}

async fn export_thread(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(thread) = s.store.get_thread(&id).ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let msgs = s.store.list_messages(&id, 10000, None).unwrap_or_default();
    let title = thread.title.clone().unwrap_or_else(|| "thread".into());
    let mut md = format!("# {title}\n\n");
    for m in &msgs {
        let who = match m.role {
            Role::User => m.sender.clone().unwrap_or_else(|| "You".into()),
            Role::Bot => m
                .from_bot
                .as_ref()
                .map(|f| f.name.clone())
                .or_else(|| m.engine.map(|e| e.label().to_string()))
                .unwrap_or_else(|| "Bot".into()),
            _ => "system".into(),
        };
        if !m.text.trim().is_empty() {
            md.push_str(&format!("**{who}:** {}\n\n", m.text));
        }
        for seg in &m.segments {
            if let Segment::ToolCall { name, .. } = seg {
                md.push_str(&format!("- _tool: {name}_\n"));
            }
        }
        if !m.attachments.is_empty() {
            for a in &m.attachments {
                md.push_str(&format!("- attachment: {} ({})\n", a.name, a.mime));
            }
        }
    }
    (
        [
            (header::CONTENT_TYPE, "text/markdown; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"transcript.md\"",
            ),
        ],
        md,
    )
        .into_response()
}

#[derive(Deserialize)]
struct PatchMessage {
    text: Option<String>,
}

async fn patch_message(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<PatchMessage>,
) -> impl IntoResponse {
    let Some(mut m) = s.store.get_message(&id).ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if let Some(t) = body.text {
        m.text = t.clone();
        m.segments = vec![Segment::Text { text: t }];
    }
    let _ = s.store.upsert_message(&m);
    let _ = s
        .bus
        .send(ServerEvent::MessageUpsert { message: m.clone() });
    Json(m).into_response()
}

async fn delete_message(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(m) = s.store.get_message(&id).ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let _ = s.store.delete_message(&id);
    let _ = s.bus.send(ServerEvent::MessageDeleted {
        id: id.clone(),
        thread_id: m.thread_id.clone(),
    });
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
struct ReactBody {
    emoji: String,
    #[serde(default = "default_reactor_by")]
    by: String,
}
fn default_reactor_by() -> String {
    "user".into()
}

async fn react_message(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ReactBody>,
) -> impl IntoResponse {
    match s.store.toggle_reaction(&id, &body.emoji, &body.by) {
        Ok(Some(m)) => {
            let _ = s
                .bus
                .send(ServerEvent::MessageUpsert { message: m.clone() });
            Json(m).into_response()
        }
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

// ---- attachments --------------------------------------------------------------

async fn upload_attachment(
    State(s): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    if body.is_empty() || body.len() > 50 * 1024 * 1024 {
        return (StatusCode::BAD_REQUEST, "empty or too large").into_response();
    }
    let name = headers
        .get("x-file-name")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("file")
        .to_string();
    let mime = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    match s.store.save_attachment(&name, &mime, &body) {
        Ok(a) => (
            StatusCode::CREATED,
            Json(serde_json::json!({
                "id": a.id, "name": a.name, "mime": a.mime, "kind": a.kind, "size": a.size,
                "url": format!("/api/attachments/{}", a.id),
            })),
        )
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")).into_response(),
    }
}

async fn get_attachment(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(a) = s.store.get_attachment(&id).ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match std::fs::read(&a.path) {
        Ok(bytes) => ([(header::CONTENT_TYPE, a.mime.clone())], bytes).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

// ---- rooms --------------------------------------------------------------------

#[derive(Deserialize)]
struct CreateRoom {
    name: String,
    member_ids: Vec<String>,
    #[serde(default)]
    responder: Option<Responder>,
    /// Shared instructions prepended to every member's persona.
    #[serde(default)]
    bulletin: Option<String>,
    #[serde(default)]
    section: Option<String>,
    /// Shared working directory overriding each member's cwd.
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    dm: Option<bool>,
}

async fn list_rooms(State(s): State<Arc<AppState>>) -> impl IntoResponse {
    Json(s.store.list_rooms().unwrap_or_default())
}

async fn get_room(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> impl IntoResponse {
    match s.store.get_room(&id) {
        Ok(Some(r)) => Json(r).into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn create_room(
    State(s): State<Arc<AppState>>,
    Json(body): Json<CreateRoom>,
) -> impl IntoResponse {
    let now = now_ms();
    let id = new_id("room");
    let thread = Thread {
        id: new_id("th"),
        kind: "room".into(),
        bot_id: None,
        room_id: Some(id.clone()),
        title: Some(body.name.clone()),
        pinned_message_id: None,
        title_from_first_message: false,
        archived_at: None,
        cwd: None,
        rewound: false,
        turn_started_at: None,
        created_at: now,
        updated_at: now,
    };
    let _ = s.store.upsert_thread(&thread);
    let room = Room {
        id: id.clone(),
        name: body.name.trim().to_string(),
        member_ids: body.member_ids,
        thread_id: thread.id.clone(),
        responder: body.responder.unwrap_or_default(),
        bulletin: body.bulletin.unwrap_or_default(),
        dm: body.dm.unwrap_or(false),
        section: body.section,
        cwd: body.cwd,
        working: false,
        busy_bot_id: None,
        turn_started_at: None,
        setup_completed_at: None,
        setup_skipped_at: None,
        unread: 0,
        last_message: None,
        last_activity_at: Some(now),
        created_at: now,
        updated_at: now,
    };
    let _ = s.store.upsert_room(&room);
    let _ = s.bus.send(ServerEvent::RoomUpsert { room: room.clone() });
    (StatusCode::CREATED, Json(room)).into_response()
}

#[derive(Deserialize)]
struct PatchRoom {
    name: Option<String>,
    member_ids: Option<Vec<String>>,
    responder: Option<Responder>,
    /// Switch the active task channel.
    thread_id: Option<String>,
    bulletin: Option<String>,
    section: Option<Option<String>>,
    cwd: Option<Option<String>>,
    setup_completed_at: Option<Option<i64>>,
    setup_skipped_at: Option<Option<i64>>,
}

async fn patch_room(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<PatchRoom>,
) -> impl IntoResponse {
    let Some(mut room) = s.store.get_room(&id).ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if let Some(v) = body.name {
        room.name = v;
    }
    if let Some(v) = body.member_ids {
        room.member_ids = v;
    }
    if let Some(v) = body.responder {
        room.responder = v;
    }
    if let Some(v) = body.thread_id {
        // Only task threads owned by this room may become active.
        let owned = s
            .store
            .get_thread(&v)
            .ok()
            .flatten()
            .map(|t| t.room_id.as_deref() == Some(id.as_str()))
            .unwrap_or(false);
        if !owned {
            return (StatusCode::BAD_REQUEST, "thread not in room").into_response();
        }
        room.thread_id = v;
    }
    if let Some(v) = body.bulletin {
        room.bulletin = v;
    }
    if let Some(v) = body.section {
        room.section = v;
    }
    if let Some(v) = body.cwd {
        room.cwd = v;
    }
    if let Some(v) = body.setup_completed_at {
        room.setup_completed_at = v;
    }
    if let Some(v) = body.setup_skipped_at {
        room.setup_skipped_at = v;
    }
    room.updated_at = now_ms();
    let _ = s.store.upsert_room(&room);
    let _ = s.bus.send(ServerEvent::RoomUpsert { room: room.clone() });
    Json(room).into_response()
}

async fn delete_room(State(s): State<Arc<AppState>>, Path(id): Path<String>) -> impl IntoResponse {
    if let Ok(Some(room)) = s.store.get_room(&id) {
        sessions::abort_thread(&s, &room.thread_id).await;
    }
    let _ = s.store.delete_room(&id);
    let _ = s.bus.send(ServerEvent::RoomDeleted { room_id: id });
    StatusCode::NO_CONTENT.into_response()
}

/// A room's task channels (GroupTask list).
async fn list_room_threads(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if s.store.get_room(&id).ok().flatten().is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    Json(s.store.room_threads(&id).unwrap_or_default()).into_response()
}

/// Open a new task channel inside a room and make it active.
async fn create_room_thread(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<CreateThread>,
) -> impl IntoResponse {
    let Some(mut room) = s.store.get_room(&id).ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match s.store.new_room_thread(&id, body.title) {
        Ok(t) => {
            room.thread_id = t.id.clone();
            room.updated_at = now_ms();
            let _ = s.store.upsert_room(&room);
            let _ = s.bus.send(ServerEvent::ThreadUpsert { thread: t.clone() });
            let _ = s.bus.send(ServerEvent::RoomUpsert { room });
            (StatusCode::CREATED, Json(t)).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")).into_response(),
    }
}

async fn mark_room_read(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let _ = s.store.mark_room_read(&id);
    if let Ok(Some(r)) = s.store.get_room(&id) {
        let _ = s.bus.send(ServerEvent::RoomUpsert { room: r });
    }
    StatusCode::NO_CONTENT
}

// ---- search ------------------------------------------------------------------

#[derive(Deserialize)]
struct SearchQ {
    q: String,
}

async fn search(State(s): State<Arc<AppState>>, Query(q): Query<SearchQ>) -> impl IntoResponse {
    let needle = q.q.trim().to_lowercase();
    if needle.is_empty() {
        return Json(serde_json::json!({ "bots": [], "messages": [], "rooms": [] }));
    }
    let bots: Vec<Bot> = s
        .store
        .list_bots_filtered(false)
        .unwrap_or_default()
        .into_iter()
        .chain(s.store.list_bots_filtered(true).unwrap_or_default())
        .filter(|b| {
            b.name.to_lowercase().contains(&needle)
                || b.title.to_lowercase().contains(&needle)
                || b.description.to_lowercase().contains(&needle)
        })
        .collect();
    let rooms: Vec<Room> = s
        .store
        .list_rooms()
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.name.to_lowercase().contains(&needle))
        .collect();
    let messages = s.store.search_messages(&needle, 50).unwrap_or_default();
    Json(serde_json::json!({ "bots": bots, "messages": messages, "rooms": rooms }))
}

// ---- decisions / sections / cli-test / usage -------------------------------

/// Decision log — every approval card (resolved included), newest first.
async fn list_decisions(State(s): State<Arc<AppState>>) -> impl IntoResponse {
    Json(s.store.recent_approvals(500).unwrap_or_default())
}

async fn sidebar_sections(State(s): State<Arc<AppState>>) -> impl IntoResponse {
    Json(s.store.sidebar_sections().unwrap_or_default())
}

#[derive(Deserialize)]
struct CliTest {
    engine: EngineKind,
}

/// Probe an engine — same check the engines list runs, on demand.
async fn cli_test(State(s): State<Arc<AppState>>, Json(body): Json<CliTest>) -> impl IntoResponse {
    let cfg = s.engine_config(body.engine);
    let status = s.driver_for(body.engine).probe(&cfg).await;
    Json(status)
}

/// Aggregate usage: message/turn counts per bot (tokens when engines report).
async fn usage(State(s): State<Arc<AppState>>) -> impl IntoResponse {
    Json(usage_rows(&s))
}

async fn usage_csv(State(s): State<Arc<AppState>>) -> impl IntoResponse {
    let rows = usage_rows(&s);
    let mut csv = String::from("bot_id,name,engine,messages,bot_replies\n");
    for r in &rows {
        csv.push_str(&format!(
            "{},{},{},{},{}\n",
            r["bot_id"].as_str().unwrap_or_default(),
            r["name"].as_str().unwrap_or_default().replace(',', ";"),
            r["engine"].as_str().unwrap_or_default(),
            r["messages"],
            r["bot_replies"],
        ));
    }
    ([(header::CONTENT_TYPE, "text/csv; charset=utf-8")], csv)
}

fn usage_rows(s: &AppState) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    let mut bots = s.store.list_bots_filtered(false).unwrap_or_default();
    bots.extend(s.store.list_bots_filtered(true).unwrap_or_default());
    for b in bots {
        let mut messages = 0i64;
        let mut bot_replies = 0i64;
        if let Ok(threads) = s.store.bot_threads(&b.id) {
            for t in threads {
                for m in s
                    .store
                    .list_messages(&t.id, 100000, None)
                    .unwrap_or_default()
                {
                    messages += 1;
                    if m.role == Role::Bot {
                        bot_replies += 1;
                    }
                }
            }
        }
        out.push(serde_json::json!({
            "bot_id": b.id,
            "name": b.name,
            "engine": b.engine.to_string(),
            "messages": messages,
            "bot_replies": bot_replies,
        }));
    }
    out
}

// ---- files / cli-candidates / mcp registry ------------------------------------

#[derive(Deserialize)]
struct FilesQ {
    /// Directory to list; defaults to the data dir.
    path: Option<String>,
}

/// Browse the filesystem for the attach picker (local files only).
async fn list_files(State(s): State<Arc<AppState>>, Query(q): Query<FilesQ>) -> impl IntoResponse {
    let dir = q
        .path
        .filter(|p| !p.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| s.store.data_dir.clone());
    let mut out = Vec::new();
    match std::fs::read_dir(&dir) {
        Ok(rd) => {
            for e in rd.flatten().take(500) {
                let md = e.metadata().ok();
                out.push(serde_json::json!({
                    "name": e.file_name().to_string_lossy(),
                    "path": e.path().display().to_string(),
                    "dir": md.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                    "size": md.map(|m| m.len()).unwrap_or(0),
                }));
            }
            out.sort_by(|a, b| {
                b["dir"].as_bool().cmp(&a["dir"].as_bool()).then(
                    a["name"]
                        .as_str()
                        .unwrap_or("")
                        .cmp(b["name"].as_str().unwrap_or("")),
                )
            });
            Json(serde_json::json!({ "path": dir.display().to_string(), "entries": out }))
                .into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, format!("{e}")).into_response(),
    }
}

/// CLI binaries discovered on PATH for each engine kind.
async fn cli_candidates(State(s): State<Arc<AppState>>) -> impl IntoResponse {
    let mut out = Vec::new();
    for kind in EngineKind::ALL {
        let cfg = s.engine_config(kind);
        let bin = match kind {
            EngineKind::Claude => "claude",
            EngineKind::Codex => "codex",
            EngineKind::Grok | EngineKind::OpenAiCompat => continue,
            _ => "fathom",
        };
        out.push(serde_json::json!({
            "engine": kind.to_string(),
            "cli": crate::engine::resolve_cli(&cfg, bin),
        }));
    }
    Json(out)
}

#[derive(Clone, Serialize, Deserialize)]
struct McpServer {
    name: String,
    /// stdio command (e.g. "npx -y some-mcp") or an http url.
    command: String,
    #[serde(default)]
    args: Vec<String>,
}

fn mcp_registry(s: &AppState) -> Vec<McpServer> {
    s.store
        .get_kv("mcp_servers")
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default()
}

async fn list_mcp_servers(State(s): State<Arc<AppState>>) -> impl IntoResponse {
    Json(mcp_registry(&s))
}

async fn add_mcp_server(
    State(s): State<Arc<AppState>>,
    Json(body): Json<McpServer>,
) -> impl IntoResponse {
    if body.name.trim().is_empty() || body.command.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "name and command required").into_response();
    }
    let mut reg = mcp_registry(&s);
    reg.retain(|m| m.name != body.name);
    reg.push(body);
    let _ = s.store.set_kv(
        "mcp_servers",
        &serde_json::to_string(&reg).unwrap_or_default(),
    );
    StatusCode::NO_CONTENT.into_response()
}

async fn del_mcp_server(
    State(s): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let mut reg = mcp_registry(&s);
    reg.retain(|m| m.name != name);
    let _ = s.store.set_kv(
        "mcp_servers",
        &serde_json::to_string(&reg).unwrap_or_default(),
    );
    StatusCode::NO_CONTENT.into_response()
}

// ---- peer comms (internal, called by engines/tools) ---------------------------

#[derive(Deserialize)]
struct PeerCall {
    /// Target bot id or name.
    bot: String,
    text: String,
    /// Caller identity — enforces peers list + approvePeerComms.
    #[serde(default)]
    from_bot: Option<String>,
    #[serde(default)]
    unattended: bool,
    /// delegate-bot only: task title.
    #[serde(default)]
    title: Option<String>,
}

/// Synchronous bot→bot question — returns the reply once the turn settles.
async fn internal_ask_bot(
    State(s): State<Arc<AppState>>,
    Json(body): Json<PeerCall>,
) -> impl IntoResponse {
    let (cid, cname) = caller(&s, &body.from_bot);
    match sessions::ask_bot(
        &s,
        &body.bot,
        &body.text,
        cid.as_deref(),
        cname.as_deref(),
        body.unattended,
    )
    .await
    {
        Ok(m) => Json(serde_json::json!({
            "bot_id": m.from_bot.as_ref().map(|f| f.bot_id.clone()),
            "thread_id": m.thread_id,
            "text": m.text,
            "message_id": m.id,
        }))
        .into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, format!("{e:#}")).into_response(),
    }
}

/// Async delegation — opens a task thread on the target and returns it.
async fn internal_delegate_bot(
    State(s): State<Arc<AppState>>,
    Json(body): Json<PeerCall>,
) -> impl IntoResponse {
    let (cid, cname) = caller(&s, &body.from_bot);
    match sessions::delegate_bot(
        &s,
        &body.bot,
        &body.text,
        body.title,
        cid.as_deref(),
        cname.as_deref(),
    )
    .await
    {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, format!("{e:#}")).into_response(),
    }
}

#[derive(Deserialize)]
struct PostToRoom {
    /// Caller bot id or name.
    bot: String,
    /// Room id or name.
    room: String,
    text: String,
    #[serde(default)]
    unattended: bool,
}

/// Bot pushes a line into a room thread (peer_post chip).
async fn internal_post_to_room(
    State(s): State<Arc<AppState>>,
    Json(body): Json<PostToRoom>,
) -> impl IntoResponse {
    let Some(bot) = sessions::find_bot(&s, &body.bot) else {
        return (StatusCode::NOT_FOUND, "bot not found").into_response();
    };
    match sessions::post_to_room(&s, &bot, &body.room, &body.text, body.unattended).await {
        Ok(m) => Json(m).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, format!("{e:#}")).into_response(),
    }
}

#[derive(Deserialize)]
struct Coordinate {
    /// Caller bot id or name.
    bot: String,
    /// Member bot ids or names (caller is added automatically).
    members: Vec<String>,
    /// The directive — becomes the room bulletin and first message.
    directive: String,
    #[serde(default)]
    name: Option<String>,
}

/// Bot spins up a coordination room with the named members.
async fn internal_coordinate_bots(
    State(s): State<Arc<AppState>>,
    Json(body): Json<Coordinate>,
) -> impl IntoResponse {
    let Some(caller) = sessions::find_bot(&s, &body.bot) else {
        return (StatusCode::NOT_FOUND, "bot not found").into_response();
    };
    match sessions::coordinate_bots(
        &s,
        &caller,
        &body.members,
        &body.directive,
        body.name.as_deref(),
    )
    .await
    {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, format!("{e:#}")).into_response(),
    }
}

/// Resolve a `from_bot` field (id or name) to (id, display name).
fn caller(s: &AppState, from: &Option<String>) -> (Option<String>, Option<String>) {
    from.as_deref()
        .and_then(|f| sessions::find_bot(s, f))
        .map(|b| (Some(b.id), Some(b.name)))
        .unwrap_or((None, None))
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
    /// "allow" | "deny" | "always" | free-text answer
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
