use std::sync::Arc;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct InboundWebhookPayload {
    pub source: String,
    pub event_type: String,
    pub payload: serde_json::Value,
}

/// POST /api/v1/webhooks/inbound
/// Inbound webhook reactor for GitHub, Sentry, Stripe, and CRM event triggers.
pub async fn handle_inbound_webhook(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    raw_body: axum::body::Bytes,
) -> impl IntoResponse {
    // Verify source-specific signatures if secret is configured
    if let Ok(secret) = std::env::var("FATHOM_WEBHOOK_SECRET") {
        if !secret.is_empty() {
            let sig = headers.get("x-fathom-signature")
                .or_else(|| headers.get("x-hub-signature-256"))
                .and_then(|v| v.to_str().ok());
            
            let Some(sig_str) = sig else {
                return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({
                    "error": "Missing webhook signature header"
                }))).into_response();
            };

            // Clean sha256= prefix if present
            let hex_sig = sig_str.strip_prefix("sha256=").unwrap_or(sig_str);

            use ring::hmac;
            let s_key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
            let Ok(provided_bytes) = hex::decode(hex_sig.trim()) else {
                return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({
                    "error": "Invalid webhook signature format"
                }))).into_response();
            };

            // Verify against raw canonical payload bytes to avoid JSON re-serialization differences
            if hmac::verify(&s_key, &raw_body, &provided_bytes).is_err() {
                return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({
                    "error": "Invalid webhook signature"
                }))).into_response();
            }
        }
    }

    let body: InboundWebhookPayload = match serde_json::from_slice(&raw_body) {
        Ok(p) => p,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": format!("Invalid JSON payload: {e}")
            }))).into_response();
        }
    };

    // Auto-dispatch session to autonomous worker
    let query = format!("[Webhook Trigger: {}/{}] Payload: {}", body.source, body.event_type, body.payload);
    
    // Persist and spawn session execution
    let session_id = pr_core::SessionId::new();
    let base_dir = std::path::PathBuf::from(&state.config.output.dir);
    let session_dir = base_dir.join(session_id.0.clone());
    let _ = std::fs::create_dir_all(&session_dir);
    let _ = state.db.create_session(&session_id, &query);
    let _ = state.db.set_session_output_dir(&session_id, &session_dir.display().to_string());

    let finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (steer_tx, steer_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let cancel = tokio_util::sync::CancellationToken::new();
    let handle = crate::spawn_session(
        state.clone(),
        session_id.clone(),
        query.clone(),
        session_dir.clone(),
        finished.clone(),
        steer_rx,
        cancel.clone(),
    );

    if let Ok(mut map) = state.active_sessions.lock() {
        map.insert(
            session_id.0.clone(),
            crate::RunningSession {
                handle,
                finished,
                steer_tx,
                cancel,
            },
        );
    }

    state.metrics.sessions_total.inc();
    state.metrics.sessions_active.inc();

    (StatusCode::ACCEPTED, Json(serde_json::json!({
        "status": "accepted",
        "session_id": session_id.0,
        "source": body.source,
        "event_type": body.event_type,
        "dispatched": true
    }))).into_response()
}
