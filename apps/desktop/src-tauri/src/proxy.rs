//! HTTP proxy — thin relay between the Tauri frontend and the fathom
//! engine API. The frontend cannot directly call localhost:port (CSP,
//! mixed content in a Tauri webview), so all engine API calls go through
//! Tauri commands.

use crate::types::DaemonStatus;
use crate::AppState;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex as StdMutex};
use tauri::{AppHandle, Emitter, State};
use tokio_util::sync::CancellationToken;

static SSE_TASKS: LazyLock<StdMutex<HashMap<String, CancellationToken>>> = LazyLock::new(|| StdMutex::new(HashMap::new()));

fn api_key() -> Option<String> {
    std::env::var("FATHOM_API_KEYS")
        .ok()
        .or_else(|| std::env::var("FATHOM_API_KEY").ok())
        .and_then(|keys| keys.split(',').map(str::trim).find(|key| !key.is_empty()).map(str::to_owned))
}

async fn send_json(request: reqwest::RequestBuilder) -> Result<Value, String> {
    let response = request.send().await.map_err(|e| format!("request failed: {e}"))?;
    let status = response.status();
    let body = response.text().await.map_err(|e| format!("read failed: {e}"))?;
    let value = serde_json::from_str::<Value>(&body).unwrap_or_else(|_| Value::String(body.clone()));
    if !status.is_success() {
        let detail = value.get("error").and_then(Value::as_str).unwrap_or(body.trim());
        return Err(format!("HTTP {status}: {detail}"));
    }
    Ok(value)
}

fn authorized(request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    match api_key() {
        Some(key) => request.header("X-Api-Key", key),
        None => request,
    }
}
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
    send_json(authorized(client.get(format!("{url}{path}")))).await
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
    send_json(authorized(client.post(format!("{url}{path}")).json(&body))).await
}

/// Helper: send a DELETE to the engine.
#[tauri::command]
pub async fn engine_screenshot(
    state: State<'_, Mutex<AppState>>,
    path: String,
) -> Result<Value, String> {
    let app = state.lock().await;
    let url = app.daemon.base_url().await.ok_or_else(|| "engine not running".to_string())?;
    let response = authorized(Client::new().get(format!("{url}{path}")))
        .send().await.map_err(|e| format!("request failed: {e}"))?;
    let status = response.status();
    let content_type = response.headers().get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok()).unwrap_or("image/png").to_string();
    let bytes = response.bytes().await.map_err(|e| format!("read failed: {e}"))?;
    if !status.is_success() { return Err(format!("HTTP {status}: screenshot request failed")); }
    Ok(serde_json::json!({ "bytes": bytes.to_vec(), "content_type": content_type }))
}

#[tauri::command]
pub async fn engine_request(
    state: State<'_, Mutex<AppState>>,
    method: String,
    path: String,
    body: Option<Value>,
) -> Result<Value, String> {
    let app = state.lock().await;
    let url = app.daemon.base_url().await.ok_or_else(|| "engine not running".to_string())?;
    let client = Client::new();
    let request = match method.to_uppercase().as_str() {
        "GET" => client.get(format!("{url}{path}")),
        "POST" => client.post(format!("{url}{path}")),
        "PUT" => client.put(format!("{url}{path}")),
        "DELETE" => client.delete(format!("{url}{path}")),
        other => return Err(format!("unsupported HTTP method: {other}")),
    };
    let request = if let Some(body) = body { request.json(&body) } else { request };
    send_json(authorized(request)).await
}

#[tauri::command]
pub async fn engine_sse_start(
    state: State<'_, Mutex<AppState>>,
    app: AppHandle,
    stream_id: String,
    path: String,
) -> Result<(), String> {
    let base_url = {
        let app_state = state.lock().await;
        app_state.daemon.base_url().await.ok_or_else(|| "engine not running".to_string())?
    };
    let response = authorized(Client::new().get(format!("{base_url}{path}")))
        .header(reqwest::header::ACCEPT, "text/event-stream")
        .send().await.map_err(|e| format!("SSE request failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("SSE request failed: HTTP {}", response.status()));
    }
    let cancel = CancellationToken::new();
    SSE_TASKS.lock().map_err(|_| "SSE registry unavailable".to_string())?.insert(stream_id.clone(), cancel.clone());
    tokio::spawn(async move {
        let mut buffer = String::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = tokio::select! {
            _ = cancel.cancelled() => None,
            value = stream.next() => value,
        } {
            let Ok(chunk) = chunk else { break };
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            let lines: Vec<String> = buffer.split('\n').map(str::to_owned).collect();
            buffer = lines.last().cloned().unwrap_or_default();
            for line in lines.into_iter().take_while(|_| true).filter(|line| line.starts_with("data: ")) {
                if let Ok(value) = serde_json::from_str::<Value>(&line[6..]) {
                    let _ = app.emit(&format!("engine:sse:{stream_id}"), value);
                }
            }
        }
        if let Ok(mut tasks) = SSE_TASKS.lock() { tasks.remove(&stream_id); }
        let _ = app.emit(&format!("engine:sse-end:{stream_id}"), ());
    });
    Ok(())
}

#[tauri::command]
pub fn engine_sse_stop(stream_id: String) {
    if let Ok(mut tasks) = SSE_TASKS.lock() {
        if let Some(cancel) = tasks.remove(&stream_id) { cancel.cancel(); }
    }
}

async fn engine_delete(state: &Mutex<AppState>, path: &str) -> Result<Value, String> {
    let app = state.lock().await;
    let url = app
        .daemon
        .base_url()
        .await
        .ok_or_else(|| "engine not running".to_string())?;
    let client = Client::new();
    send_json(authorized(client.delete(format!("{url}{path}")))).await
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

#[tauri::command]
pub async fn answer_session(
    state: State<'_, Mutex<AppState>>,
    id: String,
    request_id: String,
    text: String,
) -> Result<Value, String> {
    engine_post(&state, &format!("/api/v1/sessions/{id}/answer"), serde_json::json!({ "request_id": request_id, "text": text })).await
}

#[tauri::command]
pub async fn approve_session(
    state: State<'_, Mutex<AppState>>,
    id: String,
    request_id: String,
    approved: bool,
) -> Result<Value, String> {
    engine_post(&state, &format!("/api/v1/sessions/{id}/approve"), serde_json::json!({ "request_id": request_id, "approved": approved })).await
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