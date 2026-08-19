//! HTTP proxy — thin relay between the Tauri frontend and the fathom
//! engine API. The frontend cannot directly call localhost:port (CSP,
//! mixed content in a Tauri webview), so all engine API calls go through
//! Tauri commands.

use crate::types::DaemonStatus;
use crate::AppState;
use reqwest::Client;
use serde_json::Value;
use tauri::State;
use tokio::sync::Mutex;

/// Helper: send a GET to the engine and return JSON body.
async fn engine_get(state: &Mutex<AppState>, path: &str) -> Result<Value, String> {
    let app = state.lock().await;
    let url = app
        .daemon
        .base_url()
        .await
        .ok_or_else(|| "engine not running".to_string())?;
    let client = Client::new();
    let resp = client
        .get(format!("{url}{path}"))
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    resp.json::<Value>()
        .await
        .map_err(|e| format!("parse failed: {e}"))
}

/// Helper: send a POST to the engine with JSON body.
async fn engine_post(
    state: &Mutex<AppState>,
    path: &str,
    body: Value,
) -> Result<Value, String> {
    let app = state.lock().await;
    let url = app
        .daemon
        .base_url()
        .await
        .ok_or_else(|| "engine not running".to_string())?;
    let client = Client::new();
    let resp = client
        .post(format!("{url}{path}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    resp.json::<Value>()
        .await
        .map_err(|e| format!("parse failed: {e}"))
}

/// Helper: send a DELETE to the engine.
async fn engine_delete(state: &Mutex<AppState>, path: &str) -> Result<Value, String> {
    let app = state.lock().await;
    let url = app
        .daemon
        .base_url()
        .await
        .ok_or_else(|| "engine not running".to_string())?;
    let client = Client::new();
    let resp = client
        .delete(format!("{url}{path}"))
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    resp.json::<Value>()
        .await
        .map_err(|e| format!("parse failed: {e}"))
}

// ── Tauri commands ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn daemon_status(state: State<'_, Mutex<AppState>>) -> Result<DaemonStatus, String> {
    state.lock().await.daemon.health().await
}

#[tauri::command]
pub async fn daemon_start(
    state: State<'_, Mutex<AppState>>,
    port: Option<u16>,
    force: bool,
) -> Result<DaemonStatus, String> {
    state
        .lock()
        .await
        .daemon
        .start(crate::types::StartOptions { port, force })
        .await
}

#[tauri::command]
pub async fn daemon_stop(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    state.lock().await.daemon.kill().await;
    Ok(())
}

// ── Session API ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_sessions(state: State<'_, Mutex<AppState>>) -> Result<Value, String> {
    engine_get(&state, "/api/v1/sessions").await
}

#[tauri::command]
pub async fn create_session(
    state: State<'_, Mutex<AppState>>,
    query: String,
) -> Result<Value, String> {
    engine_post(&state, "/api/v1/sessions", serde_json::json!({ "query": query })).await
}

#[tauri::command]
pub async fn get_session(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<Value, String> {
    engine_get(&state, &format!("/api/v1/sessions/{id}")).await
}

#[tauri::command]
pub async fn cancel_session(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<Value, String> {
    engine_delete(&state, &format!("/api/v1/sessions/{id}")).await
}

#[tauri::command]
pub async fn steer_session(
    state: State<'_, Mutex<AppState>>,
    id: String,
    message: String,
) -> Result<Value, String> {
    engine_post(
        &state,
        &format!("/api/v1/sessions/{id}/steer"),
        serde_json::json!({ "message": message }),
    )
    .await
}

#[tauri::command]
pub async fn get_session_results(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<Value, String> {
    engine_get(&state, &format!("/api/v1/sessions/{id}/results")).await
}

// ── Agent API ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_agents(state: State<'_, Mutex<AppState>>) -> Result<Value, String> {
    engine_get(&state, "/api/v1/agents").await
}

#[tauri::command]
pub async fn get_agent(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<Value, String> {
    engine_get(&state, &format!("/api/v1/agents/{id}")).await
}

// ── Job API ───────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_jobs(state: State<'_, Mutex<AppState>>) -> Result<Value, String> {
    engine_get(&state, "/api/v1/jobs").await
}

#[tauri::command]
pub async fn create_job(
    state: State<'_, Mutex<AppState>>,
    task: String,
    attempts: Option<i64>,
) -> Result<Value, String> {
    engine_post(
        &state,
        "/api/v1/jobs",
        serde_json::json!({
            "task": task,
            "attempts": attempts.unwrap_or(3),
        }),
    )
    .await
}

#[tauri::command]
pub async fn cancel_job(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<Value, String> {
    engine_delete(&state, &format!("/api/v1/jobs/{id}")).await
}

// ── Memory API ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_memories(state: State<'_, Mutex<AppState>>) -> Result<Value, String> {
    engine_get(&state, "/api/v1/memories").await
}