//! Computer-service relay routes.
//!
//! The configured service is an intentionally small loopback HTTP service.  Its
//! screenshot endpoint returns `{ mimeType, data }`, where `data` is base64
//! image data.  `/screen` upgrades to a websocket and emits binary image frames
//! at a bounded polling interval (currently 500ms); a client may close the
//! websocket at any time.  Upstream calls and websocket writes have explicit
//! timeouts so a disconnected or wedged computer service cannot retain a
//! request task indefinitely.

use axum::{
    body::{Body, Bytes},
    extract::{Extension, Path, Query, State, WebSocketUpgrade},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use axum::extract::ws::{Message, WebSocket};
use base64::Engine;
use futures::{SinkExt, StreamExt};
use crate::{auth::AuthPrincipal, AppState};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{timeout, Interval};

const DEFAULT_SERVICE_URL: &str = "http://127.0.0.1:8765";
const SERVICE_URL_ENV: &str = "FATHOM_COMPUTER_SERVICE_URL";
const LEGACY_SERVICE_URL_ENV: &str = "COMPUTER_SERVICE_URL";
const SERVICE_TOKEN_ENV: &str = "COMPUTER_TOKEN";
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(5);
const WEBSOCKET_SEND_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_SCREENSHOT_BYTES: usize = 20 * 1024 * 1024;
const MAX_JSON_BYTES: usize = 2 * 1024 * 1024;
const MAX_WORKSPACE_BODY_BYTES: usize = 1_100_000;

#[derive(Debug, Deserialize)]
pub struct FilePathQuery {
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScreenshotEnvelope {
    mime_type: String,
    data: String,
}

fn service_root() -> String {
    std::env::var(SERVICE_URL_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| std::env::var(LEGACY_SERVICE_URL_ENV).ok().filter(|v| !v.trim().is_empty()))
        .unwrap_or_else(|| DEFAULT_SERVICE_URL.to_string())
        .trim_end_matches('/')
        .to_string()
}

fn service_url_at(root: &str, path: &str) -> String {
    format!("{}{}", root.trim_end_matches('/'), path)
}

fn percent_encode(value: &str) -> String {
    value.bytes().fold(String::new(), |mut encoded, byte| {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
        encoded
    })
}

async fn agent_service_root(state: &Arc<AppState>, agent_id: &str) -> anyhow::Result<String> {
    let Some(supervisor) = state.supervisor.as_ref() else {
        anyhow::bail!("per-agent computer supervisor is not configured");
    };
    let container = supervisor.ensure(agent_id).await.map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(format!("http://127.0.0.1:{}", container.port))
}

fn service_request(request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    match std::env::var(SERVICE_TOKEN_ENV) {
        Ok(token) if !token.trim().is_empty() => request.bearer_auth(token),
        _ => request,
    }
}

fn operator_request(request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    service_request(request).header("x-fathom-operator", "true")
}

async fn bounded_bytes(response: reqwest::Response, limit: usize) -> anyhow::Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        anyhow::bail!("computer service response exceeds {limit} bytes");
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if bytes.len().saturating_add(chunk.len()) > limit {
            anyhow::bail!("computer service response exceeds {limit} bytes");
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn upstream_status(status: reqwest::StatusCode) -> StatusCode {
    StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY)
}

fn proxy_body(status: reqwest::StatusCode, content_type: Option<&str>, body: Vec<u8>) -> Response {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = upstream_status(status);
    if let Some(content_type) = content_type.and_then(|value| HeaderValue::from_str(value).ok()) {
        response.headers_mut().insert(header::CONTENT_TYPE, content_type);
    }
    response
}

async fn upstream_json_get_at(root: &str, path: &str) -> Response {
    let client = pr_core::http_client();
    let request = service_request(client.get(service_url_at(root, path)));
    let response = match timeout(UPSTREAM_TIMEOUT, request.send()).await {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => return (StatusCode::BAD_GATEWAY, format!("computer service unavailable: {error}")).into_response(),
        Err(_) => return (StatusCode::GATEWAY_TIMEOUT, "computer service request timed out").into_response(),
    };
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    match timeout(UPSTREAM_TIMEOUT, bounded_bytes(response, MAX_JSON_BYTES)).await {
        Ok(Ok(body)) => proxy_body(status, content_type.as_deref(), body),
        Ok(Err(error)) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, "computer service response timed out").into_response(),
    }
}

async fn upstream_json_get(path: &str) -> Response {
    upstream_json_get_at(&service_root(), path).await
}

async fn upstream_json_post_at(root: &str, path: &str, body: Value) -> Response {
    upstream_json_post_with(root, path, body, false).await
}

async fn upstream_json_post_operator(root: &str, path: &str, body: Value) -> Response {
    upstream_json_post_with(root, path, body, true).await
}

async fn upstream_json_post_with(root: &str, path: &str, body: Value, operator: bool) -> Response {
    let client = pr_core::http_client();
    let request = if operator { operator_request(client.post(service_url_at(root, path)).json(&body)) } else { service_request(client.post(service_url_at(root, path)).json(&body)) };
    let response = match timeout(UPSTREAM_TIMEOUT, request.send()).await {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => return (StatusCode::BAD_GATEWAY, format!("computer service unavailable: {error}")).into_response(),
        Err(_) => return (StatusCode::GATEWAY_TIMEOUT, "computer service request timed out").into_response(),
    };
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    match timeout(UPSTREAM_TIMEOUT, bounded_bytes(response, MAX_JSON_BYTES)).await {
        Ok(Ok(bytes)) => proxy_body(status, content_type.as_deref(), bytes),
        Ok(Err(error)) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, "computer service response timed out").into_response(),
    }
}

async fn upstream_json_post(path: &str, body: Value) -> Response {
    upstream_json_post_at(&service_root(), path, body).await
}

async fn upstream_json_put_at(root: &str, path: &str, body: Bytes) -> Response {
    if body.len() > MAX_WORKSPACE_BODY_BYTES {
        return (StatusCode::PAYLOAD_TOO_LARGE, "request body too large").into_response();
    }
    let client = pr_core::http_client();
    let request = service_request(client.put(service_url_at(root, path)).header(header::CONTENT_TYPE, "application/json").body(body));
    let response = match timeout(UPSTREAM_TIMEOUT, request.send()).await {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => return (StatusCode::BAD_GATEWAY, format!("computer service unavailable: {error}")).into_response(),
        Err(_) => return (StatusCode::GATEWAY_TIMEOUT, "computer service request timed out").into_response(),
    };
    let status = response.status();
    let content_type = response.headers().get(header::CONTENT_TYPE).and_then(|value| value.to_str().ok()).map(str::to_owned);
    match timeout(UPSTREAM_TIMEOUT, bounded_bytes(response, MAX_JSON_BYTES)).await {
        Ok(Ok(bytes)) => proxy_body(status, content_type.as_deref(), bytes),
        Ok(Err(error)) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, "computer service response timed out").into_response(),
    }
}

async fn upstream_delete_at(root: &str, path: &str) -> Response {
    let client = pr_core::http_client();
    let request = service_request(client.delete(service_url_at(root, path)));
    let response = match timeout(UPSTREAM_TIMEOUT, request.send()).await {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => return (StatusCode::BAD_GATEWAY, format!("computer service unavailable: {error}")).into_response(),
        Err(_) => return (StatusCode::GATEWAY_TIMEOUT, "computer service request timed out").into_response(),
    };
    let status = response.status();
    let content_type = response.headers().get(header::CONTENT_TYPE).and_then(|value| value.to_str().ok()).map(str::to_owned);
    match timeout(UPSTREAM_TIMEOUT, bounded_bytes(response, MAX_JSON_BYTES)).await {
        Ok(Ok(bytes)) => proxy_body(status, content_type.as_deref(), bytes),
        Ok(Err(error)) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, "computer service response timed out").into_response(),
    }
}

async fn screenshot_bytes_at(root: &str) -> anyhow::Result<(String, Vec<u8>)> {
    let client = pr_core::http_client();
    let request = service_request(client.get(service_url_at(root, "/screenshot")));
    let response = timeout(UPSTREAM_TIMEOUT, request.send()).await??;
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let body = timeout(UPSTREAM_TIMEOUT, bounded_bytes(response, MAX_SCREENSHOT_BYTES)).await??;
    if !status.is_success() {
        anyhow::bail!("computer service returned HTTP {status}");
    }
    if content_type.starts_with("image/") {
        return Ok((content_type, body));
    }
    let envelope: ScreenshotEnvelope = serde_json::from_slice(&body)?;
    let mime = if envelope.mime_type.trim().is_empty() {
        "image/png".to_string()
    } else {
        envelope.mime_type
    };
    let encoded = envelope
        .data
        .split_once(',')
        .map(|(_, value)| value)
        .unwrap_or(&envelope.data);
    let image = base64::engine::general_purpose::STANDARD.decode(encoded.trim())?;
    if image.len() > MAX_SCREENSHOT_BYTES {
        anyhow::bail!("computer screenshot exceeds {MAX_SCREENSHOT_BYTES} bytes");
    }
    Ok((mime, image))
}

async fn screenshot_response_at(root: &str) -> Response {
    match screenshot_bytes_at(root).await {
        Ok((mime, bytes)) => proxy_body(
            reqwest::StatusCode::OK,
            Some(&mime),
            bytes,
        ),
        Err(error) => (StatusCode::BAD_GATEWAY, format!("computer screenshot unavailable: {error}")).into_response(),
    }
}

async fn screenshot_response() -> Response {
    screenshot_response_at(&service_root()).await
}

/// Proxy screenshot for an explicitly named agent, resolving its supervisor port.
pub async fn screenshot(State(state): State<Arc<AppState>>, Path(agent_id): Path<String>) -> Response {
    match agent_service_root(&state, &agent_id).await {
        Ok(root) => screenshot_response_at(&root).await,
        Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
    }
}

pub async fn screenshot_default() -> Response {
    screenshot_response().await
}

pub async fn health() -> Response {
    upstream_json_get("/health").await
}

pub async fn health_default() -> Response {
    health().await
}

/// Start or refresh the upstream browser session.
pub async fn start_session(Json(body): Json<Value>) -> Response {
    upstream_json_post("/session", body).await
}

pub async fn snapshot() -> Response {
    upstream_json_get("/snapshot").await
}

pub async fn snapshot_default() -> Response {
    snapshot().await
}

pub async fn tabs() -> Response { upstream_json_get("/tabs").await }
pub async fn tabs_open(Json(body): Json<Value>) -> Response { upstream_json_post("/tabs/open", body).await }
pub async fn tab_activate(Path(tab_id): Path<String>) -> Response { upstream_json_post(&format!("/tabs/{}/activate", percent_encode(&tab_id)), Value::Object(Default::default())).await }
pub async fn tab_close(Path(tab_id): Path<String>) -> Response { upstream_json_post(&format!("/tabs/{}/close", percent_encode(&tab_id)), Value::Object(Default::default())).await }

pub async fn files() -> Response {
    upstream_json_get("/files").await
}

pub async fn files_read(Query(query): Query<FilePathQuery>) -> Response {
    upstream_json_get(&format!("/files/read?path={}", percent_encode(&query.path))).await
}

pub async fn files_write(body: Bytes) -> Response {
    upstream_json_put_at(&service_root(), "/files/write", body).await
}

pub async fn files_delete(Query(query): Query<FilePathQuery>) -> Response {
    upstream_delete_at(&service_root(), &format!("/files?path={}", percent_encode(&query.path))).await
}

pub async fn files_for_agent(State(state): State<Arc<AppState>>, Path(agent_id): Path<String>) -> Response {
    match agent_service_root(&state, &agent_id).await {
        Ok(root) => upstream_json_get_at(&root, "/files").await,
        Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
    }
}

pub async fn files_read_for_agent(State(state): State<Arc<AppState>>, Path(agent_id): Path<String>, Query(query): Query<FilePathQuery>) -> Response {
    match agent_service_root(&state, &agent_id).await {
        Ok(root) => upstream_json_get_at(&root, &format!("/files/read?path={}", percent_encode(&query.path))).await,
        Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
    }
}

pub async fn files_write_for_agent(State(state): State<Arc<AppState>>, Path(agent_id): Path<String>, body: Bytes) -> Response {
    match agent_service_root(&state, &agent_id).await {
        Ok(root) => upstream_json_put_at(&root, "/files/write", body).await,
        Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
    }
}

pub async fn files_delete_for_agent(State(state): State<Arc<AppState>>, Path(agent_id): Path<String>, Query(query): Query<FilePathQuery>) -> Response {
    match agent_service_root(&state, &agent_id).await {
        Ok(root) => upstream_delete_at(&root, &format!("/files?path={}", percent_encode(&query.path))).await,
        Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
    }
}

pub async fn health_for_agent(State(state): State<Arc<AppState>>, Path(agent_id): Path<String>) -> Response {
    match agent_service_root(&state, &agent_id).await {
        Ok(root) => upstream_json_get_at(&root, "/health").await,
        Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
    }
}

pub async fn snapshot_for_agent(State(state): State<Arc<AppState>>, Path(agent_id): Path<String>) -> Response {
    match agent_service_root(&state, &agent_id).await {
        Ok(root) => upstream_json_get_at(&root, "/snapshot").await,
        Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
    }
}

async fn control(path: &str) -> Response {
    upstream_json_post(path, Value::Object(Default::default())).await
}

pub async fn take_control() -> Response {
    control("/control/take").await
}

pub async fn release_control() -> Response {
    control("/control/release").await
}

pub async fn take_control_for_agent(State(state): State<Arc<AppState>>, Path(agent_id): Path<String>) -> Response {
    match agent_service_root(&state, &agent_id).await {
        Ok(root) => upstream_json_post_at(&root, "/control/take", Value::Object(Default::default())).await,
        Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
    }
}

pub async fn release_control_for_agent(State(state): State<Arc<AppState>>, Path(agent_id): Path<String>) -> Response {
    match agent_service_root(&state, &agent_id).await {
        Ok(root) => upstream_json_post_at(&root, "/control/release", Value::Object(Default::default())).await,
        Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
    }
}

pub async fn navigate(Json(body): Json<Value>) -> Response {
    upstream_json_post("/navigate", body).await
}

pub async fn navigate_for_agent(State(state): State<Arc<AppState>>, Path(agent_id): Path<String>, Json(body): Json<Value>) -> Response {
    match agent_service_root(&state, &agent_id).await {
        Ok(root) => upstream_json_post_at(&root, "/navigate", body).await,
        Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
    }
}

pub async fn click(Json(body): Json<Value>) -> Response {
    upstream_json_post("/click", body).await
}

pub async fn type_text(Json(body): Json<Value>) -> Response {
    upstream_json_post("/type", body).await
}

/// Enter a secret directly into the computer without returning or logging the value.
/// The upstream service returns only the refreshed page metadata/snapshot.
pub async fn secret(Extension(principal): Extension<AuthPrincipal>, Json(body): Json<Value>) -> Response {
    if principal.0 == "anonymous" {
        return (StatusCode::FORBIDDEN, "operator authentication required").into_response();
    }
    if let Value::Object(map) = &body {
        if !map.contains_key("ref") || !map.contains_key("secret") {
            return (StatusCode::BAD_REQUEST, "secret requires ref and secret").into_response();
        }
    } else {
        return (StatusCode::BAD_REQUEST, "secret body must be an object").into_response();
    }
    upstream_json_post_operator(&service_root(), "/operator/secret", body).await
}

pub async fn key(Json(body): Json<Value>) -> Response {
    upstream_json_post("/key", body).await
}

async fn scoped_post(state: &Arc<AppState>, agent_id: &str, path: &str, body: Value) -> Response {
    match agent_service_root(state, agent_id).await {
        Ok(root) => upstream_json_post_at(&root, path, body).await,
        Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
    }
}

pub async fn click_for_agent(State(state): State<Arc<AppState>>, Path(agent_id): Path<String>, Json(body): Json<Value>) -> Response {
    scoped_post(&state, &agent_id, "/click", body).await
}

pub async fn type_for_agent(State(state): State<Arc<AppState>>, Path(agent_id): Path<String>, Json(body): Json<Value>) -> Response {
    scoped_post(&state, &agent_id, "/type", body).await
}

pub async fn key_for_agent(State(state): State<Arc<AppState>>, Path(agent_id): Path<String>, Json(body): Json<Value>) -> Response {
    scoped_post(&state, &agent_id, "/key", body).await
}

/// Poll screenshots and relay each successful frame as a websocket binary
/// message.  Polling is deliberately bounded and stops on close, read error,
/// timeout, or a client that does not accept frames promptly.
pub async fn screen(
    Path(agent_id): Path<String>,
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match agent_service_root(&state, &agent_id).await {
        Ok(root) => ws.on_upgrade(move |socket| relay_screen(socket, root)),
        Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
    }
}

async fn relay_screen(socket: WebSocket, root: String) {
    let (mut sender, mut receiver) = socket.split();
    let mut interval: Interval = tokio::time::interval(POLL_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(Message::Ping(payload))) => {
                        if timeout(WEBSOCKET_SEND_TIMEOUT, sender.send(Message::Pong(payload))).await.is_err() { break; }
                    }
                    Some(Ok(_)) => {}
                }
            }
            _ = interval.tick() => {
                let bytes = match timeout(UPSTREAM_TIMEOUT, screenshot_bytes_at(&root)).await {
                    Ok(Ok((_mime, bytes))) => bytes,
                    _ => continue,
                };
                if timeout(WEBSOCKET_SEND_TIMEOUT, sender.send(Message::Binary(bytes.into()))).await.is_err() {
                    break;
                }
            }
        }
    }
    let _ = timeout(WEBSOCKET_SEND_TIMEOUT, sender.send(Message::Close(None))).await;
}
